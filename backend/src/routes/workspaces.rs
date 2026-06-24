use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};

use crate::auth::AuthUser;
use crate::error::AppError;
use crate::models::{CreateWorkspace, Workspace};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/workspaces", get(list).post(create))
}

/// Create a workspace and make the caller its owner, atomically.
async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<CreateWorkspace>,
) -> Result<(StatusCode, Json<Workspace>), AppError> {
    let mut tx = state.pool.begin().await?;

    let ws = sqlx::query_as::<_, Workspace>(
        "INSERT INTO workspaces (name) VALUES ($1) RETURNING id, name, created_at",
    )
    .bind(body.name)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(ws.id)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(ws)))
}

/// List only the workspaces the caller is a member of.
async fn list(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Vec<Workspace>>, AppError> {
    let rows = sqlx::query_as::<_, Workspace>(
        "SELECT w.id, w.name, w.created_at FROM workspaces w
         JOIN workspace_members m ON m.workspace_id = w.id
         WHERE m.user_id = $1 ORDER BY w.created_at DESC",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}
