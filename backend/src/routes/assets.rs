use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::ai;
use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{Asset, AssetDetail, DeriveAssets, GenerateAssets, UpdateAsset, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/assets", get(list).post(generate))
        .route("/projects/:project_id/assets/upload", post(upload))
        .route("/projects/:project_id/assets/:base_id/derive", post(derive))
        .route("/assets/:id", get(get_one).patch(update_asset).delete(delete_one))
        .route("/assets/:id/file", get(file))
}

pub(crate) const ASSET_COLS: &str =
    "id, project_id, name, kind, s3_key, mime_type, prompt, role, status, tags, source_kind, derivation, canon_version_id, exemplar, created_at";

/// 10 MB cap on a single uploaded asset.
const MAX_UPLOAD: usize = 10 * 1024 * 1024;

/// Fill in the browser-usable `url` for an asset. Object-stored assets are
/// served through our authed proxy; inline assets carry the URL directly.
pub(crate) fn with_url(mut a: Asset) -> Asset {
    a.url = if is_inline(&a.s3_key) {
        a.s3_key.clone()
    } else {
        format!("/api/assets/{}/file", a.id)
    };
    a
}

/// An inline reference is something the browser can load directly (a data URL
/// or an absolute http(s) URL) rather than an object-storage key.
fn is_inline(s3_key: &str) -> bool {
    s3_key.starts_with("data:") || s3_key.starts_with("http://") || s3_key.starts_with("https://")
}

/// Generate one or more images for a project and persist them as assets.
async fn generate(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<GenerateAssets>,
) -> Result<(StatusCode, Json<Vec<Asset>>), AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    let count = body.count.unwrap_or(1).clamp(1, 4);

    // Seed against the current canon so generated bases follow the project's
    // style from the start. The model gets the compiled prompt; the asset keeps
    // the raw text (for a clean caption) + the canon version it was made under.
    let canon: Option<(Uuid, Value)> = sqlx::query_as(
        "SELECT id, data FROM canon WHERE project_id = $1 ORDER BY version DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?;
    let vertical: String = sqlx::query_scalar("SELECT vertical FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_one(&state.pool)
        .await?;
    let canon_id = canon.as_ref().map(|(id, _)| *id);
    let prompt = compile_prompt(&body.prompt, canon.as_ref().map(|(_, d)| d), &vertical);

    // The moat loop: if the project has an approved style exemplar, condition
    // generation on it (reference img2img) so new assets inherit the approved
    // art direction. Falls back to text-only when there's none (or it can't be
    // referenced, e.g. an inline/remote-URL asset).
    let exemplar: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT id, s3_key, mime_type FROM assets
         WHERE project_id = $1 AND exemplar AND status = 'approved'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?;
    let exemplar_ref = match &exemplar {
        Some((eid, key, mime)) => match asset_bytes(&state, key, mime.as_deref()).await {
            Ok((bytes, m)) => {
                tracing::info!(exemplar = %eid, "conditioning generation on approved exemplar");
                Some((*eid, bytes, m))
            }
            Err(_) => None,
        },
        None => None,
    };
    let exemplar_meta = match &exemplar_ref {
        Some((eid, _, _)) => serde_json::json!({ "exemplar_id": eid }),
        None => serde_json::json!({}),
    };

    let mut assets = Vec::with_capacity(count as usize);
    for n in 0..count as usize {
        let img = match &exemplar_ref {
            Some((_, bytes, mime)) => ai::images::derive_image(bytes, mime, &prompt, n).await?,
            None => ai::images::generate_image(&prompt, n).await?,
        };

        // Object storage when configured; otherwise store the image inline as a
        // data URL so the app still works with no object store.
        let s3_key = if state.storage.configured() {
            let key = format!("projects/{project_id}/assets/{}", Uuid::new_v4());
            state.storage.put(&key, &img.bytes, &img.mime).await?;
            key
        } else {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&img.bytes);
            format!("data:{};base64,{b64}", img.mime)
        };

        let asset = sqlx::query_as::<_, Asset>(&format!(
            "INSERT INTO assets (project_id, kind, s3_key, mime_type, prompt, source_kind, canon_version_id, metadata)
             VALUES ($1, 'image', $2, $3, $4, 'seeded', $5, $6) RETURNING {ASSET_COLS}"
        ))
        .bind(project_id)
        .bind(&s3_key)
        .bind(&img.mime)
        .bind(&body.prompt)
        .bind(canon_id)
        .bind(&exemplar_meta)
        .fetch_one(&state.pool)
        .await?;
        ai::embeddings::index_asset_soft(&state.pool, &asset).await;
        assets.push(with_url(asset));
    }
    Ok((StatusCode::CREATED, Json(assets)))
}

#[derive(Debug, Deserialize)]
pub struct UploadParams {
    /// Optional free-text role, e.g. "base", "reference".
    #[serde(default)]
    role: Option<String>,
}

/// Bring a base/reference asset in by uploading raw image bytes (body = the
/// file, `Content-Type` = its mime). Stored in object storage when configured,
/// else inline as a data URL. `source_kind='uploaded'`.
async fn upload(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Query(params): Query<UploadParams>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<Asset>), AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    if body.is_empty() {
        return Err(AppError::BadRequest("empty upload".into()));
    }
    if body.len() > MAX_UPLOAD {
        return Err(AppError::BadRequest("file too large (max 10MB)".into()));
    }
    let mime = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .filter(|m| m.starts_with("image/"))
        .unwrap_or("image/png")
        .to_string();

    let s3_key = if state.storage.configured() {
        let key = format!("projects/{project_id}/assets/{}", Uuid::new_v4());
        state.storage.put(&key, &body, &mime).await?;
        key
    } else {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&body);
        format!("data:{mime};base64,{b64}")
    };

    let asset = sqlx::query_as::<_, Asset>(&format!(
        "INSERT INTO assets (project_id, kind, s3_key, mime_type, role, source_kind)
         VALUES ($1, 'image', $2, $3, $4, 'uploaded') RETURNING {ASSET_COLS}"
    ))
    .bind(project_id)
    .bind(&s3_key)
    .bind(&mime)
    .bind(&params.role)
    .fetch_one(&state.pool)
    .await?;
    ai::embeddings::index_asset_soft(&state.pool, &asset).await;
    Ok((StatusCode::CREATED, Json(with_url(asset))))
}

async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<Asset>>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;
    let rows = sqlx::query_as::<_, Asset>(&format!(
        "SELECT {ASSET_COLS} FROM assets WHERE project_id = $1 ORDER BY created_at DESC"
    ))
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows.into_iter().map(with_url).collect()))
}

/// Derive N images from a base asset, conditioned on the base + current canon.
/// Each derivative records `source_kind='derived'`, a `derived_from` edge to the
/// base, and the canon version it was made under.
async fn derive(
    State(state): State<AppState>,
    user: AuthUser,
    Path((project_id, base_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<DeriveAssets>,
) -> Result<(StatusCode, Json<Vec<Asset>>), AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    let count = body.count.unwrap_or(1).clamp(1, 4);
    let instruction = body.instruction.trim();
    if instruction.is_empty() {
        return Err(AppError::BadRequest("instruction required".into()));
    }

    // Load the base (must belong to this project) and its bytes.
    let base: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT s3_key, mime_type FROM assets WHERE id = $1 AND project_id = $2",
    )
    .bind(base_id)
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?;
    let (base_key, base_mime) = base.ok_or(AppError::NotFound)?;
    let (base_bytes, base_mime) = asset_bytes(&state, &base_key, base_mime.as_deref()).await?;

    // Compile the prompt from the instruction + current canon (style + negatives).
    let canon: Option<(Uuid, Value)> = sqlx::query_as(
        "SELECT id, data FROM canon WHERE project_id = $1 ORDER BY version DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?;
    let vertical: String = sqlx::query_scalar("SELECT vertical FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_one(&state.pool)
        .await?;
    let canon_id = canon.as_ref().map(|(id, _)| *id);
    let prompt = compile_prompt(instruction, canon.as_ref().map(|(_, d)| d), &vertical);

    let mut out = Vec::with_capacity(count as usize);
    for n in 0..count as usize {
        let img = ai::images::derive_image(&base_bytes, &base_mime, &prompt, n).await?;
        let s3_key = if state.storage.configured() {
            let key = format!("projects/{project_id}/assets/{}", Uuid::new_v4());
            state.storage.put(&key, &img.bytes, &img.mime).await?;
            key
        } else {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&img.bytes);
            format!("data:{};base64,{b64}", img.mime)
        };
        let asset = sqlx::query_as::<_, Asset>(&format!(
            "INSERT INTO assets
               (project_id, kind, s3_key, mime_type, prompt, source_kind, derivation, canon_version_id)
             VALUES ($1, 'image', $2, $3, $4, 'derived', $5, $6) RETURNING {ASSET_COLS}"
        ))
        .bind(project_id)
        .bind(&s3_key)
        .bind(&img.mime)
        .bind(&prompt)
        .bind(instruction)
        .bind(canon_id)
        .fetch_one(&state.pool)
        .await?;
        // Provenance edge: derivative -> base.
        sqlx::query(
            "INSERT INTO asset_links (from_asset, to_asset, relation) VALUES ($1, $2, 'derived_from')",
        )
        .bind(asset.id)
        .bind(base_id)
        .execute(&state.pool)
        .await?;
        ai::embeddings::index_asset_soft(&state.pool, &asset).await;
        out.push(with_url(asset));
    }
    Ok((StatusCode::CREATED, Json(out)))
}

/// One asset with its lineage — the base it was derived from + its derivatives.
async fn get_one(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<AssetDetail>, AppError> {
    let asset = sqlx::query_as::<_, Asset>(&format!("SELECT {ASSET_COLS} FROM assets WHERE id = $1"))
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;
    auth::require_project_access(&state.pool, asset.project_id, user.id, WorkspaceRole::Viewer).await?;

    // The base this was derived from (if any).
    let base_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT to_asset FROM asset_links WHERE from_asset = $1 AND relation = 'derived_from' LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    let base = match base_id {
        Some(bid) => sqlx::query_as::<_, Asset>(&format!("SELECT {ASSET_COLS} FROM assets WHERE id = $1"))
            .bind(bid)
            .fetch_optional(&state.pool)
            .await?
            .map(with_url),
        None => None,
    };

    // Everything derived from this asset.
    let derivatives = sqlx::query_as::<_, Asset>(&format!(
        "SELECT {ASSET_COLS} FROM assets
         WHERE id IN (SELECT from_asset FROM asset_links WHERE to_asset = $1 AND relation = 'derived_from')
         ORDER BY created_at DESC"
    ))
    .bind(id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(with_url)
    .collect();

    Ok(Json(AssetDetail { asset: with_url(asset), base, derivatives }))
}

/// Update editable metadata (status / role / tags). Only provided fields change.
/// Reused by the review buttons (status only) and the inspector (role/tags).
async fn update_asset(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateAsset>,
) -> Result<Json<Asset>, AppError> {
    let project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM assets WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
    let project_id = project_id.ok_or(AppError::NotFound)?;
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;

    let asset = sqlx::query_as::<_, Asset>(&format!(
        "UPDATE assets SET
           status   = COALESCE($1::asset_status, status),
           role     = COALESCE($2::text, role),
           tags     = COALESCE($3::text[], tags),
           name     = COALESCE($5::text, name),
           exemplar = COALESCE($6::boolean, exemplar)
         WHERE id = $4 RETURNING {ASSET_COLS}"
    ))
    .bind(body.status)
    .bind(body.role)
    .bind(body.tags)
    .bind(id)
    .bind(body.name)
    .bind(body.exemplar)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(with_url(asset)))
}

/// Delete an asset. `asset_links` rows CASCADE; the stored object is cleaned up
/// best-effort (a leftover object is harmless, a failed delete shouldn't 500).
async fn delete_one(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let row: Option<(Uuid, String)> =
        sqlx::query_as("SELECT project_id, s3_key FROM assets WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
    let (project_id, s3_key) = row.ok_or(AppError::NotFound)?;
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;

    sqlx::query("DELETE FROM assets WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if !is_inline(&s3_key) {
        if let Err(e) = state.storage.delete(&s3_key).await {
            tracing::warn!(error = %e, key = %s3_key, "asset deleted but object cleanup failed");
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Load an asset's raw bytes to use as a derivation reference: object storage by
/// key, or an inline `data:` URL decoded in place.
pub(crate) async fn asset_bytes(
    state: &AppState,
    s3_key: &str,
    mime: Option<&str>,
) -> Result<(Vec<u8>, String), AppError> {
    if let Some(rest) = s3_key.strip_prefix("data:") {
        let (meta, payload) = rest
            .split_once(',')
            .ok_or_else(|| AppError::Internal("malformed stored data URL".into()))?;
        let m = meta
            .split(';')
            .next()
            .filter(|x| !x.is_empty())
            .map(str::to_string)
            .or_else(|| mime.map(str::to_string))
            .unwrap_or_else(|| "image/png".to_string());
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .map_err(|e| AppError::Internal(format!("base decode failed: {e}")))?;
        Ok((bytes, m))
    } else if is_inline(s3_key) {
        Err(AppError::ServiceUnavailable(
            "base is a remote URL; cannot derive from it".into(),
        ))
    } else {
        let bytes = state.storage.get(s3_key).await?;
        Ok((bytes, mime.unwrap_or("image/png").to_string()))
    }
}

/// Build a derivation/generation prompt: instruction + the canon's style rules
/// + the vertical's framing hint + negatives. `canon` is optional so the
/// vertical framing still applies before a project has defined its canon.
fn compile_prompt(instruction: &str, canon: Option<&Value>, vertical: &str) -> String {
    let style = canon
        .and_then(|c| c.get("style"))
        .and_then(Value::as_object)
        .map(|o| {
            o.values()
                .filter_map(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let negative = canon
        .and_then(|c| c.get("negative"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("; "))
        .unwrap_or_default();

    let mut p = instruction.trim().to_string();
    if !style.is_empty() {
        p.push_str(&format!(" Maintain this exact art style: {style}."));
    }
    p.push_str(&format!(" {}", crate::verticals::get(vertical).render_hint));
    if !negative.is_empty() {
        p.push_str(&format!(" Must NOT include: {negative}."));
    }
    p
}

#[cfg(test)]
mod tests {
    use super::compile_prompt;

    #[test]
    fn vertical_render_hint_is_applied() {
        let game = compile_prompt("draw a sword", None, "game_2d");
        assert!(game.contains("One centered isolated asset, transparent background."));

        let manhwa = compile_prompt("draw a sword", None, "manhwa");
        assert!(manhwa.contains("webtoon panel-ready"));
        assert!(!manhwa.contains("One centered isolated asset"));

        // Unknown vertical falls back to the default (game_2d) hint.
        let unknown = compile_prompt("draw a sword", None, "bogus");
        assert!(unknown.contains("One centered isolated asset, transparent background."));
    }
}

/// Stream an asset's image bytes. Stable, same-origin URL safe to embed in a
/// screen's DSL (`props.src`). Authorized via the asset's owning project.
async fn file(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let row: Option<(Uuid, String, Option<String>)> =
        sqlx::query_as("SELECT project_id, s3_key, mime_type FROM assets WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
    let (project_id, s3_key, mime_type) = row.ok_or(AppError::NotFound)?;
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;

    let content_type = mime_type.unwrap_or_else(|| "application/octet-stream".to_string());

    // Inline references (legacy / no object store): decode-and-serve a data
    // URL, or redirect for an http(s) URL.
    if is_inline(&s3_key) {
        if let Some(rest) = s3_key.strip_prefix("data:") {
            let (meta, payload) = rest
                .split_once(',')
                .ok_or_else(|| AppError::Internal("malformed stored data URL".into()))?;
            let mime = meta.split(';').next().filter(|m| !m.is_empty()).unwrap_or(&content_type);
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(payload)
                .map_err(|e| AppError::Internal(format!("stored image decode failed: {e}")))?;
            return Ok(serve(bytes, mime));
        }
        return Ok(Response::builder()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, s3_key)
            .body(Body::empty())
            .map_err(|e| AppError::Internal(e.to_string()))?);
    }

    let bytes = state.storage.get(&s3_key).await?;
    Ok(serve(bytes, &content_type))
}

fn serve(bytes: Vec<u8>, content_type: &str) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            // Immutable: an asset's bytes never change once generated.
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable".to_string(),
            ),
        ],
        bytes,
    )
        .into_response()
}
