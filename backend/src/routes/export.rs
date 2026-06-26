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

/// Decode + run deterministic checks for one asset, returning its verdict and
/// raw bytes (so export can write the same bytes it checked). The filename is
/// index-prefixed for stable uniqueness within the pack.
fn check_one(index: usize, asset: &Asset, bytes: &[u8], mime: &str) -> (AssetCheck, Vec<u8>) {
    let ext = ext_for(mime);
    let base = slug(asset.role.as_deref().unwrap_or("asset"));
    let base = if base.is_empty() { "asset".to_string() } else { base };
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

    let project_name: String =
        sqlx::query_scalar("SELECT name FROM projects WHERE id = $1")
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

    let checked = gather(&state, project_id, &body.asset_ids).await?;

    let included: Vec<&(AssetCheck, Vec<u8>)> = checked.iter().filter(|(c, _)| c.ok).collect();
    let skipped: Vec<&AssetCheck> = checked.iter().filter(|(c, _)| !c.ok).map(|(c, _)| c).collect();

    let manifest = serde_json::json!({
        "project": project_name,
        "canon_version": canon_version,
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "asset_count": included.len(),
        "assets": included.iter().map(|(c, _)| serde_json::json!({
            "id": c.id,
            "filename": c.filename,
            "role": c.role,
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
            zip.start_file(format!("assets/{}", c.filename), opts)
                .map_err(|e| AppError::Internal(format!("zip: {e}")))?;
            zip.write_all(bytes)
                .map_err(|e| AppError::Internal(format!("zip: {e}")))?;
        }
        zip.finish()
            .map_err(|e| AppError::Internal(format!("zip: {e}")))?
            .into_inner()
    };

    let fname = format!("{}-pack.zip", slug(&project_name));
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{fname}\"")),
        ],
        buf,
    ))
}
