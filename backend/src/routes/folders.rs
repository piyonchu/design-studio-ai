use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{CreateFolder, Folder, FolderNode, UpdateFolder, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/folders", get(list).post(create))
        .route("/folders/:id", axum::routing::patch(update).delete(delete_one))
}

/// The project's folder tree as a flat list (the client nests it by `parent_id`),
/// each row carrying the count of assets filed directly in it.
async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<FolderNode>>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;
    let rows = sqlx::query_as::<_, FolderNode>(
        "SELECT f.id, f.project_id, f.parent_id, f.name, f.created_at,
                COUNT(a.id) AS asset_count
         FROM folders f
         LEFT JOIN assets a ON a.folder_id = f.id
         WHERE f.project_id = $1
         GROUP BY f.id
         ORDER BY f.name",
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
    Json(body): Json<CreateFolder>,
) -> Result<(StatusCode, Json<Folder>), AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    // A parent (if given) must be a folder in the same project.
    if let Some(pid) = body.parent_id {
        require_folder_in_project(&state, pid, project_id).await?;
    }
    let f = sqlx::query_as::<_, Folder>(
        "INSERT INTO folders (project_id, parent_id, name) VALUES ($1, $2, $3)
         RETURNING id, project_id, parent_id, name, created_at",
    )
    .bind(project_id)
    .bind(body.parent_id)
    .bind(name)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(f)))
}

/// Rename and/or reparent. Reparenting rejects cycles (a folder cannot move into
/// itself or any of its own descendants).
async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateFolder>,
) -> Result<Json<Folder>, AppError> {
    let project_id = folder_project(&state, id).await?;
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;

    if let Some(name) = body.name.as_deref() {
        if name.trim().is_empty() {
            return Err(AppError::BadRequest("name cannot be empty".into()));
        }
    }

    // Reparent target validation: same project, not self, not a descendant.
    let (parent_id, set_parent) = match body.parent_id {
        Some(v) => (v, true),
        None => (None, false),
    };
    if let Some(pid) = parent_id {
        if pid == id {
            return Err(AppError::BadRequest("a folder cannot be its own parent".into()));
        }
        require_folder_in_project(&state, pid, project_id).await?;
        if is_descendant(&state, pid, id).await? {
            return Err(AppError::BadRequest("cannot move a folder into its own subtree".into()));
        }
    }

    let f = sqlx::query_as::<_, Folder>(
        "UPDATE folders SET
           name      = COALESCE($2, name),
           parent_id = CASE WHEN $4 THEN $3::uuid ELSE parent_id END
         WHERE id = $1
         RETURNING id, project_id, parent_id, name, created_at",
    )
    .bind(id)
    .bind(body.name.map(|n| n.trim().to_string()))
    .bind(parent_id)
    .bind(set_parent)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(f))
}

/// Delete a folder. Its subtree of folders cascades (FK ON DELETE CASCADE);
/// assets filed anywhere in that subtree are *unfiled* (folder_id → NULL), not
/// destroyed — the bytes outlive the tree. Editor+.
async fn delete_one(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let project_id = folder_project(&state, id).await?;
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    sqlx::query("DELETE FROM folders WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// The project a folder belongs to (404 if it doesn't exist).
async fn folder_project(state: &AppState, id: Uuid) -> Result<Uuid, AppError> {
    sqlx::query_scalar("SELECT project_id FROM folders WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)
}

/// 400 unless `folder_id` is a folder in `project_id`.
async fn require_folder_in_project(
    state: &AppState,
    folder_id: Uuid,
    project_id: Uuid,
) -> Result<(), AppError> {
    let ok: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM folders WHERE id = $1 AND project_id = $2")
            .bind(folder_id)
            .bind(project_id)
            .fetch_optional(&state.pool)
            .await?;
    ok.map(|_| ()).ok_or_else(|| AppError::BadRequest("parent folder not found in this project".into()))
}

/// Is `candidate` inside the subtree rooted at `ancestor`? Walks parent links up
/// from `candidate` to guard against reparent cycles.
async fn is_descendant(state: &AppState, candidate: Uuid, ancestor: Uuid) -> Result<bool, AppError> {
    let found: Option<Uuid> = sqlx::query_scalar(
        "WITH RECURSIVE up AS (
            SELECT id, parent_id FROM folders WHERE id = $1
            UNION ALL
            SELECT f.id, f.parent_id FROM folders f JOIN up ON f.id = up.parent_id
         )
         SELECT id FROM up WHERE id = $2 LIMIT 1",
    )
    .bind(candidate)
    .bind(ancestor)
    .fetch_optional(&state.pool)
    .await?;
    Ok(found.is_some())
}
