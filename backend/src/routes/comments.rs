use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{AssetComment, CreateComment, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/assets/:asset_id/comments", get(list).post(create))
        .route("/comments/:id", delete(delete_one))
}

/// The project that owns an asset, or 404 — the authorization anchor for comments.
async fn asset_project(state: &AppState, asset_id: Uuid) -> Result<Uuid, AppError> {
    sqlx::query_scalar("SELECT project_id FROM assets WHERE id = $1")
        .bind(asset_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)
}

const COMMENT_COLS: &str = "c.id, c.asset_id, c.author_id, u.email AS author_email, c.body, c.created_at";

/// An asset's comment thread, oldest first.
async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(asset_id): Path<Uuid>,
) -> Result<Json<Vec<AssetComment>>, AppError> {
    let project_id = asset_project(&state, asset_id).await?;
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;
    let rows = sqlx::query_as::<_, AssetComment>(&format!(
        "SELECT {COMMENT_COLS} FROM asset_comments c
         LEFT JOIN users u ON u.id = c.author_id
         WHERE c.asset_id = $1 ORDER BY c.created_at ASC"
    ))
    .bind(asset_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

/// Post a comment. Any project member who can edit can comment.
async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Path(asset_id): Path<Uuid>,
    Json(body): Json<CreateComment>,
) -> Result<(StatusCode, Json<AssetComment>), AppError> {
    let project_id = asset_project(&state, asset_id).await?;
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    let text = body.body.trim();
    if text.is_empty() {
        return Err(AppError::BadRequest("comment body required".into()));
    }
    let comment = sqlx::query_as::<_, AssetComment>(&format!(
        "WITH inserted AS (
           INSERT INTO asset_comments (asset_id, author_id, body)
           VALUES ($1, $2, $3) RETURNING id, asset_id, author_id, body, created_at
         )
         SELECT {COMMENT_COLS} FROM inserted c LEFT JOIN users u ON u.id = c.author_id"
    ))
    .bind(asset_id)
    .bind(user.id)
    .bind(text)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(comment)))
}

/// Delete a comment. The author may always remove their own; otherwise a project
/// Owner can moderate.
async fn delete_one(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let row: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT a.project_id, c.author_id FROM asset_comments c
         JOIN assets a ON a.id = c.asset_id WHERE c.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    let (project_id, author_id) = row.ok_or(AppError::NotFound)?;

    let min = if author_id == Some(user.id) {
        WorkspaceRole::Viewer // own comment — membership is enough
    } else {
        WorkspaceRole::Owner // moderating someone else's
    };
    auth::require_project_access(&state.pool, project_id, user.id, min).await?;

    sqlx::query("DELETE FROM asset_comments WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
