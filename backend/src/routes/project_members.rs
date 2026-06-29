use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{
    ProjectAccess, ProjectMemberRow, ProjectRole, SetProjectRole, WorkspaceRole,
};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:id/access", get(my_access))
        .route("/projects/:id/members", get(list))
        .route(
            "/projects/:id/members/:user_id",
            axum::routing::put(set_role).delete(clear_role),
        )
}

/// The caller's effective role on the project + whether they may approve. Drives
/// UI gating (approve buttons, review queue).
async fn my_access(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectAccess>, AppError> {
    let role = auth::effective_project_role(&state.pool, id, user.id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(ProjectAccess { role, can_approve: role.can_approve() }))
}

/// Every workspace member with the role they effectively have on this project
/// (override if set, else their workspace role) — the per-project access list.
async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ProjectMemberRow>>, AppError> {
    auth::require_project_access(&state.pool, id, user.id, WorkspaceRole::Viewer).await?;
    let rows: Vec<(Uuid, String, Option<String>, WorkspaceRole, Option<ProjectRole>)> =
        sqlx::query_as(
            "SELECT m.user_id, u.email, u.display_name, m.role, pm.role
             FROM projects p
             JOIN workspace_members m ON m.workspace_id = p.workspace_id
             JOIN users u ON u.id = m.user_id
             LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = m.user_id
             WHERE p.id = $1
             ORDER BY u.email",
        )
        .bind(id)
        .fetch_all(&state.pool)
        .await?;

    let out = rows
        .into_iter()
        .map(|(user_id, email, display_name, ws, ov)| {
            // Workspace owners always resolve to project owner (can't be locked
            // out); otherwise the override wins, else the mapped workspace role.
            let effective = if ws == WorkspaceRole::Owner {
                ProjectRole::Owner
            } else {
                ov.unwrap_or_else(|| ProjectRole::from_workspace(ws))
            };
            ProjectMemberRow {
                user_id,
                email,
                display_name,
                workspace_role: ws,
                project_role: effective,
                overridden: ov.is_some() && ws != WorkspaceRole::Owner,
            }
        })
        .collect();
    Ok(Json(out))
}

/// Set a per-project role override for a workspace member. Project owner only.
async fn set_role(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, target)): Path<(Uuid, Uuid)>,
    Json(body): Json<SetProjectRole>,
) -> Result<StatusCode, AppError> {
    auth::require_project_role(&state.pool, id, user.id, ProjectRole::Owner).await?;

    // The target must belong to the owning workspace (overrides refine an
    // existing member's access; they don't grant access to outsiders).
    let is_member: Option<Uuid> = sqlx::query_scalar(
        "SELECT m.user_id FROM projects p
         JOIN workspace_members m ON m.workspace_id = p.workspace_id
         WHERE p.id = $1 AND m.user_id = $2",
    )
    .bind(id)
    .bind(target)
    .fetch_optional(&state.pool)
    .await?;
    if is_member.is_none() {
        return Err(AppError::BadRequest("user is not a member of this workspace".into()));
    }

    sqlx::query(
        "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (project_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(id)
    .bind(target)
    .bind(body.role)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Remove a per-project override (the member falls back to their workspace role).
async fn clear_role(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, target)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    auth::require_project_role(&state.pool, id, user.id, ProjectRole::Owner).await?;
    sqlx::query("DELETE FROM project_members WHERE project_id = $1 AND user_id = $2")
        .bind(id)
        .bind(target)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
