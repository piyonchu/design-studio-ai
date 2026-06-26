use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{Canon, CreateCanon, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    // GET = current canon, POST = append a new version.
    Router::new().route("/projects/:project_id/canon", get(latest).post(create))
}

const CANON_COLS: &str = "id, project_id, parent_id, version, data, created_at";

/// The current (highest-version) canon for a project; 404 if none defined yet.
async fn latest(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Canon>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;

    let canon = sqlx::query_as::<_, Canon>(&format!(
        "SELECT {CANON_COLS} FROM canon WHERE project_id = $1 ORDER BY version DESC LIMIT 1"
    ))
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(canon))
}

/// Append a new canon version: parent = current head, version auto-incremented.
/// Immutable lineage so a style change is "v2, keep or regenerate?" not a destroy.
async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateCanon>,
) -> Result<(StatusCode, Json<Canon>), AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;

    let head: Option<(Uuid, i32)> = sqlx::query_as(
        "SELECT id, version FROM canon WHERE project_id = $1 ORDER BY version DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?;
    let (parent_id, version) = match head {
        Some((id, v)) => (Some(id), v + 1),
        None => (None, 1),
    };

    let canon = sqlx::query_as::<_, Canon>(&format!(
        "INSERT INTO canon (project_id, parent_id, version, data)
         VALUES ($1, $2, $3, $4) RETURNING {CANON_COLS}"
    ))
    .bind(project_id)
    .bind(parent_id)
    .bind(version)
    .bind(&body.data)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(canon)))
}
