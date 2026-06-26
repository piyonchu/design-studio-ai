use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{CreateProject, Project, WorkspaceRole};
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
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
    Json(body): Json<CreateProject>,
) -> Result<(StatusCode, Json<Project>), AppError> {
    // Must be at least an editor in the target workspace (404 if not a member).
    auth::require_member(&state.pool, workspace_id, user.id, WorkspaceRole::Editor).await?;

    // The vertical (if given) must be a registered pack — the registry is the
    // authority. Omitted → defaults to game_2d via COALESCE below.
    if let Some(v) = &body.vertical {
        if !crate::verticals::is_known(v) {
            return Err(AppError::BadRequest(format!("unknown vertical '{v}'")));
        }
    }

    let project = sqlx::query_as::<_, Project>(
        "INSERT INTO projects (workspace_id, name, brief, vertical)
         VALUES ($1, $2, $3, COALESCE($4, 'game_2d'))
         RETURNING id, workspace_id, name, brief, vertical, created_at",
    )
    .bind(workspace_id)
    .bind(body.name)
    .bind(body.brief)
    .bind(body.vertical)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(project)))
}

async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Vec<Project>>, AppError> {
    auth::require_member(&state.pool, workspace_id, user.id, WorkspaceRole::Viewer).await?;

    let rows = sqlx::query_as::<_, Project>(
        "SELECT id, workspace_id, name, brief, vertical, created_at
         FROM projects WHERE workspace_id = $1 ORDER BY created_at DESC",
    )
    .bind(workspace_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn get_one(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Project>, AppError> {
    auth::require_project_access(&state.pool, id, user.id, WorkspaceRole::Viewer).await?;

    let project = sqlx::query_as::<_, Project>(
        "SELECT id, workspace_id, name, brief, vertical, created_at FROM projects WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(project))
}
