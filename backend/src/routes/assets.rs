use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::ai;
use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{Asset, AttachAsset, GenerateAssets, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/assets", get(list).post(generate))
        .route("/assets/:id/attach", post(attach))
}

const ASSET_COLS: &str =
    "id, project_id, screen_id, kind, s3_key, mime_type, prompt, created_at";

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
        let url = ai::images::generate_image(&body.prompt, n).await?;
        let asset = sqlx::query_as::<_, Asset>(&format!(
            "INSERT INTO assets (project_id, kind, s3_key, mime_type, prompt)
             VALUES ($1, 'image', $2, 'image/png', $3) RETURNING {ASSET_COLS}"
        ))
        .bind(project_id)
        .bind(url)
        .bind(&body.prompt)
        .fetch_one(&state.pool)
        .await?;
        assets.push(asset);
    }
    Ok((StatusCode::CREATED, Json(assets)))
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
    Ok(Json(rows))
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
    Ok(Json(asset))
}
