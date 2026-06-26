use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use serde::Deserialize;
use uuid::Uuid;

use crate::ai;
use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{Asset, AttachAsset, GenerateAssets, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/assets", get(list).post(generate))
        .route("/projects/:project_id/assets/upload", post(upload))
        .route("/assets/:id/attach", post(attach))
        .route("/assets/:id/file", get(file))
}

const ASSET_COLS: &str =
    "id, project_id, screen_id, kind, s3_key, mime_type, prompt, role, status, tags, source_kind, created_at";

/// 10 MB cap on a single uploaded asset.
const MAX_UPLOAD: usize = 10 * 1024 * 1024;

/// Fill in the browser-usable `url` for an asset. Object-stored assets are
/// served through our authed proxy; inline assets carry the URL directly.
fn with_url(mut a: Asset) -> Asset {
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

    let mut assets = Vec::with_capacity(count as usize);
    for n in 0..count as usize {
        let img = ai::images::generate_image(&body.prompt, n).await?;

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
            "INSERT INTO assets (project_id, kind, s3_key, mime_type, prompt, source_kind)
             VALUES ($1, 'image', $2, $3, $4, 'seeded') RETURNING {ASSET_COLS}"
        ))
        .bind(project_id)
        .bind(&s3_key)
        .bind(&img.mime)
        .bind(&body.prompt)
        .fetch_one(&state.pool)
        .await?;
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

/// Record the asset↔screen relationship (Design Memory).
async fn attach(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AttachAsset>,
) -> Result<Json<Asset>, AppError> {
    // Authorize via the asset's owning project.
    let project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM assets WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
    let project_id = project_id.ok_or(AppError::NotFound)?;
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;

    let asset = sqlx::query_as::<_, Asset>(&format!(
        "UPDATE assets SET screen_id = $1 WHERE id = $2 RETURNING {ASSET_COLS}"
    ))
    .bind(body.screen_artifact_id)
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(with_url(asset)))
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
