use std::io::{Cursor, Write};

use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use uuid::Uuid;
use zip::write::SimpleFileOptions;

use super::assets::{asset_bytes, ASSET_COLS};
use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{Asset, AssetCheck, AssetStatus, ExportReport, ExportRequest, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/export/check", post(check))
        .route("/projects/:project_id/export", post(export))
}

/// Fetch the requested assets that actually belong to the project, newest first.
async fn load_assets(
    state: &AppState,
    project_id: Uuid,
    ids: &[Uuid],
) -> Result<Vec<Asset>, AppError> {
    let assets = sqlx::query_as::<_, Asset>(&format!(
        "SELECT {ASSET_COLS} FROM assets
         WHERE project_id = $1 AND id = ANY($2::uuid[])
         ORDER BY created_at ASC"
    ))
    .bind(project_id)
    .bind(ids)
    .fetch_all(&state.pool)
    .await?;
    Ok(assets)
}

fn ext_for(mime: &str) -> &'static str {
    match mime {
        m if m.contains("png") => "png",
        m if m.contains("jpeg") || m.contains("jpg") => "jpg",
        m if m.contains("svg") => "svg",
        m if m.contains("webp") => "webp",
        _ => "bin",
    }
}

/// A filesystem-safe slug: lowercase alnum, runs of anything else collapse to `-`.
fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::{ext_for, slug};

    #[test]
    fn slug_lowercases_and_collapses_separators() {
        assert_eq!(slug("Hero Idle Frame"), "hero-idle-frame");
        assert_eq!(slug("  A/B  c! "), "a-b-c");
        assert_eq!(slug("já_vu??"), "j-vu"); // non-ascii dropped
        assert_eq!(slug(""), "");
    }

    #[test]
    fn ext_for_maps_known_mimes() {
        assert_eq!(ext_for("image/png"), "png");
        assert_eq!(ext_for("image/jpeg"), "jpg");
        assert_eq!(ext_for("image/svg+xml"), "svg");
        assert_eq!(ext_for("image/webp"), "webp");
        assert_eq!(ext_for("application/octet-stream"), "bin");
    }
}

/// Decode + run deterministic checks for one asset, returning its verdict and
/// raw bytes (so export can write the same bytes it checked). The filename is
/// index-prefixed for stable uniqueness within the pack.
fn check_one(index: usize, asset: &Asset, bytes: &[u8], mime: &str) -> (AssetCheck, Vec<u8>) {
    let ext = ext_for(mime);
    // Group by role (folder); name the file by the explicit name, else the role.
    let role_slug = slug(asset.role.as_deref().unwrap_or(""));
    let group = if role_slug.is_empty() { "ungrouped".to_string() } else { role_slug };
    let name_slug = slug(asset.name.as_deref().unwrap_or(""));
    let base = if !name_slug.is_empty() {
        name_slug
    } else if group != "ungrouped" {
        group.clone()
    } else {
        "asset".to_string()
    };
    let filename = format!("{index:02}-{base}.{ext}");

    let mut issues: Vec<String> = Vec::new();
    let (mut width, mut height, mut has_alpha, format) = (None, None, None, Some(ext.to_string()));

    if asset.status == AssetStatus::Rejected {
        issues.push("asset is rejected — exclude or re-review before export".into());
    }

    if ext == "svg" {
        // Vector source: nothing raster to verify, but flag it for a sprite pack.
        issues.push("vector (SVG) source — no fixed raster dimensions".into());
    } else if ext == "bin" {
        issues.push(format!("unrecognized image format ({mime})"));
    } else {
        match image::load_from_memory(bytes) {
            Ok(img) => {
                width = Some(img.width());
                height = Some(img.height());
                has_alpha = Some(img.color().has_alpha());
            }
            Err(e) => issues.push(format!("could not decode image: {e}")),
        }
    }

    // Blocking = rejected or undecodable. SVG note is a warning, not a blocker.
    let blocking = asset.status == AssetStatus::Rejected
        || issues.iter().any(|i| i.starts_with("could not decode") || i.starts_with("unrecognized"));

    (
        AssetCheck {
            id: asset.id,
            filename,
            role: asset.role.clone(),
            group,
            tags: asset.tags.clone(),
            status: asset.status,
            format,
            width,
            height,
            has_alpha,
            issues,
            ok: !blocking,
        },
        bytes.to_vec(),
    )
}

/// Fetch bytes + check every requested asset. Shared by `check` and `export`.
async fn gather(
    state: &AppState,
    project_id: Uuid,
    ids: &[Uuid],
) -> Result<Vec<(AssetCheck, Vec<u8>)>, AppError> {
    let assets = load_assets(state, project_id, ids).await?;
    if assets.is_empty() {
        return Err(AppError::BadRequest("no exportable assets in this project".into()));
    }
    let mut out = Vec::with_capacity(assets.len());
    for (i, a) in assets.iter().enumerate() {
        let (bytes, mime) = asset_bytes(state, &a.s3_key, a.mime_type.as_deref()).await?;
        out.push(check_one(i + 1, a, &bytes, &mime));
    }
    Ok(out)
}

/// Pre-export report: per-asset pass/fail without producing the pack.
async fn check(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<ExportRequest>,
) -> Result<Json<ExportReport>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;
    let checked = gather(&state, project_id, &body.asset_ids).await?;
    let assets: Vec<AssetCheck> = checked.into_iter().map(|(c, _)| c).collect();
    let ok_count = assets.iter().filter(|a| a.ok).count();
    let issue_count = assets.iter().filter(|a| !a.issues.is_empty()).count();
    Ok(Json(ExportReport { assets, ok_count, issue_count }))
}

/// Build the pack: a zip of the asset images + a `manifest.json` describing
/// them. Blocking assets (rejected / undecodable) are skipped, with the skip
/// recorded in the manifest so the download is always a clean, usable pack.
async fn export(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<ExportRequest>,
) -> Result<impl IntoResponse, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;

    let (project_name, vertical): (String, String) =
        sqlx::query_as("SELECT name, vertical FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or(AppError::NotFound)?;
    let canon_version: Option<i32> = sqlx::query_scalar(
        "SELECT version FROM canon WHERE project_id = $1 ORDER BY version DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?;

    // Resolve the export target against the project's vertical. Absent/"generic"
    // → the vertical-neutral pack; a known engine tag is only allowed if the
    // vertical declares that engine (the per-vertical export-adapter hook).
    let engine = match body.target.as_deref() {
        None | Some("") | Some("generic") => None,
        Some(tag) => {
            let e = crate::verticals::Engine::from_tag(tag)
                .ok_or_else(|| AppError::BadRequest(format!("unknown export target '{tag}'")))?;
            if crate::verticals::get(&vertical).engine != Some(e) {
                return Err(AppError::BadRequest(format!(
                    "vertical '{vertical}' has no '{tag}' export target"
                )));
            }
            Some(e)
        }
    };

    let checked = gather(&state, project_id, &body.asset_ids).await?;

    let included: Vec<&(AssetCheck, Vec<u8>)> = checked.iter().filter(|(c, _)| c.ok).collect();
    let skipped: Vec<&AssetCheck> = checked.iter().filter(|(c, _)| !c.ok).map(|(c, _)| c).collect();

    // The pack path for an included asset: assets/<group>/<filename>.
    let path_of = |c: &AssetCheck| format!("assets/{}/{}", c.group, c.filename);

    // Groups in first-seen order, each listing its in-pack file paths. A future
    // engine adapter (Godot/Unity) maps a group → an animation / atlas.
    let mut group_order: Vec<String> = Vec::new();
    for (c, _) in &included {
        if !group_order.contains(&c.group) {
            group_order.push(c.group.clone());
        }
    }
    let groups = group_order.iter().map(|g| {
        let files: Vec<String> = included
            .iter()
            .filter(|(c, _)| &c.group == g)
            .map(|(c, _)| path_of(c))
            .collect();
        serde_json::json!({ "name": g, "count": files.len(), "files": files })
    }).collect::<Vec<_>>();

    let manifest = serde_json::json!({
        "project": project_name,
        "canon_version": canon_version,
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "target": engine.map(|e| e.tag()).unwrap_or("generic"),
        "asset_count": included.len(),
        "groups": groups,
        "assets": included.iter().map(|(c, _)| serde_json::json!({
            "id": c.id,
            "path": path_of(c),
            "filename": c.filename,
            "group": c.group,
            "role": c.role,
            "tags": c.tags,
            "status": c.status,
            "format": c.format,
            "width": c.width,
            "height": c.height,
            "has_alpha": c.has_alpha,
        })).collect::<Vec<_>>(),
        "skipped": skipped.iter().map(|c| serde_json::json!({
            "id": c.id, "filename": c.filename, "issues": c.issues,
        })).collect::<Vec<_>>(),
    });

    let buf = {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        zip.start_file("manifest.json", opts)
            .map_err(|e| AppError::Internal(format!("zip: {e}")))?;
        zip.write_all(serde_json::to_string_pretty(&manifest).unwrap_or_default().as_bytes())
            .map_err(|e| AppError::Internal(format!("zip: {e}")))?;

        for (c, bytes) in &included {
            zip.start_file(path_of(c), opts)
                .map_err(|e| AppError::Internal(format!("zip: {e}")))?;
            zip.write_all(bytes)
                .map_err(|e| AppError::Internal(format!("zip: {e}")))?;
        }

        // Engine adapter: emit the import-ready scaffolding alongside the assets.
        if engine == Some(crate::verticals::Engine::Godot) {
            use crate::export::godot;
            let mut extra: Vec<godot::TextFile> = Vec::new();
            // A `<asset>.import` next to each texture so Godot imports it with
            // our 2D-sprite settings (it fills the local cache pointer itself).
            for (c, _) in &included {
                let asset_path = path_of(c);
                extra.push(godot::TextFile {
                    path: format!("{asset_path}.import"),
                    contents: godot::texture_import(&asset_path),
                });
            }
            let group_counts: Vec<(String, usize)> = group_order
                .iter()
                .map(|g| (g.clone(), included.iter().filter(|(c, _)| &c.group == g).count()))
                .collect();
            extra.push(godot::TextFile {
                path: "project.godot".into(),
                contents: godot::project_godot(&project_name),
            });
            extra.push(godot::TextFile {
                path: "README.md".into(),
                contents: godot::readme(&project_name, canon_version, &group_counts),
            });
            for f in &extra {
                zip.start_file(&f.path, opts)
                    .map_err(|e| AppError::Internal(format!("zip: {e}")))?;
                zip.write_all(f.contents.as_bytes())
                    .map_err(|e| AppError::Internal(format!("zip: {e}")))?;
            }
        }

        zip.finish()
            .map_err(|e| AppError::Internal(format!("zip: {e}")))?
            .into_inner()
    };

    let suffix = engine.map(|e| format!("-{}", e.tag())).unwrap_or_default();
    let fname = format!("{}{suffix}-pack.zip", slug(&project_name));
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{fname}\"")),
        ],
        buf,
    ))
}
