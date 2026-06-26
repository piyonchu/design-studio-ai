use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{CreateRecipe, Recipe, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/recipes", get(list).post(create))
        .route("/recipes/:id", delete(delete_one))
}

/// A project's saved derivation templates, newest first.
async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<Recipe>>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;
    let rows = sqlx::query_as::<_, Recipe>(
        "SELECT id, project_id, name, instruction, created_at
         FROM generation_recipes WHERE project_id = $1 ORDER BY created_at DESC",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateRecipe>,
) -> Result<(StatusCode, Json<Recipe>), AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    let name = body.name.trim();
    let instruction = body.instruction.trim();
    if name.is_empty() || instruction.is_empty() {
        return Err(AppError::BadRequest("name and instruction required".into()));
    }
    let recipe = sqlx::query_as::<_, Recipe>(
        "INSERT INTO generation_recipes (project_id, name, instruction)
         VALUES ($1, $2, $3) RETURNING id, project_id, name, instruction, created_at",
    )
    .bind(project_id)
    .bind(name)
    .bind(instruction)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(recipe)))
}

async fn delete_one(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM generation_recipes WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
    let project_id = project_id.ok_or(AppError::NotFound)?;
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    sqlx::query("DELETE FROM generation_recipes WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
