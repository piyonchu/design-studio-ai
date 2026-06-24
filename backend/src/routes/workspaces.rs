use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};

use crate::error::AppError;
use crate::models::{CreateWorkspace, Workspace};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/workspaces", get(list).post(create))
}

async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateWorkspace>,
) -> Result<(StatusCode, Json<Workspace>), AppError> {
    let ws = sqlx::query_as::<_, Workspace>(
        "INSERT INTO workspaces (name) VALUES ($1) RETURNING id, name, created_at",
    )
    .bind(body.name)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(ws)))
}

async fn list(State(state): State<AppState>) -> Result<Json<Vec<Workspace>>, AppError> {
    let rows = sqlx::query_as::<_, Workspace>(
        "SELECT id, name, created_at FROM workspaces ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}
