use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{CreateProject, Project};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/workspaces/:workspace_id/projects",
            post(create).get(list),
        )
        .route("/projects/:id", get(get_one))
}

async fn create(
    State(state): State<AppState>,
    Path(workspace_id): Path<Uuid>,
    Json(body): Json<CreateProject>,
) -> Result<(StatusCode, Json<Project>), AppError> {
    // Explicit parent check gives a clean 404 instead of a generic FK error.
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = $1)")
            .bind(workspace_id)
            .fetch_one(&state.pool)
            .await?;
    if !exists {
        return Err(AppError::NotFound);
    }

    let project = sqlx::query_as::<_, Project>(
        "INSERT INTO projects (workspace_id, name, brief) VALUES ($1, $2, $3)
         RETURNING id, workspace_id, name, brief, created_at",
    )
    .bind(workspace_id)
    .bind(body.name)
    .bind(body.brief)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(project)))
}

async fn list(
    State(state): State<AppState>,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Vec<Project>>, AppError> {
    let rows = sqlx::query_as::<_, Project>(
        "SELECT id, workspace_id, name, brief, created_at
         FROM projects WHERE workspace_id = $1 ORDER BY created_at DESC",
    )
    .bind(workspace_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Project>, AppError> {
    let project = sqlx::query_as::<_, Project>(
        "SELECT id, workspace_id, name, brief, created_at FROM projects WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(project))
}
