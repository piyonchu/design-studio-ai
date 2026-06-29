use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{CreateWorkspace, InviteMember, Workspace, WorkspaceMember, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/workspaces", get(list).post(create))
        .route("/workspaces/:id/members", get(members).post(invite))
        .route("/workspaces/:id/members/:user_id", axum::routing::delete(remove_member))
}

/// List a workspace's members (any member may view the team).
async fn members(
    State(state): State<AppState>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Vec<WorkspaceMember>>, AppError> {
    auth::require_member(&state.pool, workspace_id, user.id, WorkspaceRole::Viewer).await?;
    let rows = sqlx::query_as::<_, WorkspaceMember>(
        "SELECT m.user_id, u.email, u.display_name, m.role
         FROM workspace_members m JOIN users u ON u.id = m.user_id
         WHERE m.workspace_id = $1
         ORDER BY m.role DESC, u.email ASC",
    )
    .bind(workspace_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

/// Invite an existing user (by email) to the workspace. Owner-only. There's no
/// email-invite flow yet — the person must already have an account.
async fn invite(
    State(state): State<AppState>,
    user: AuthUser,
    Path(workspace_id): Path<Uuid>,
    Json(body): Json<InviteMember>,
) -> Result<(StatusCode, Json<WorkspaceMember>), AppError> {
    auth::require_member(&state.pool, workspace_id, user.id, WorkspaceRole::Owner).await?;
    let role = body.role.unwrap_or(WorkspaceRole::Editor);
    let email = body.email.trim().to_lowercase();

    let invitee: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE lower(email) = $1")
        .bind(&email)
        .fetch_optional(&state.pool)
        .await?;
    let invitee = invitee.ok_or_else(|| {
        AppError::BadRequest("no account with that email — they need to sign up first".into())
    })?;

    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (workspace_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(workspace_id)
    .bind(invitee)
    .bind(role)
    .execute(&state.pool)
    .await?;

    let member = sqlx::query_as::<_, WorkspaceMember>(
        "SELECT m.user_id, u.email, u.display_name, m.role
         FROM workspace_members m JOIN users u ON u.id = m.user_id
         WHERE m.workspace_id = $1 AND m.user_id = $2",
    )
    .bind(workspace_id)
    .bind(invitee)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(member)))
}

/// Remove a member. Owner-only; can't remove the last owner.
async fn remove_member(
    State(state): State<AppState>,
    user: AuthUser,
    Path((workspace_id, target)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    auth::require_member(&state.pool, workspace_id, user.id, WorkspaceRole::Owner).await?;
    let owners: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_members WHERE workspace_id = $1 AND role = 'owner'",
    )
    .bind(workspace_id)
    .fetch_one(&state.pool)
    .await?;
    let target_is_owner: bool = sqlx::query_scalar(
        "SELECT role = 'owner' FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace_id)
    .bind(target)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or(false);
    if target_is_owner && owners <= 1 {
        return Err(AppError::BadRequest("can't remove the workspace's only owner".into()));
    }
    sqlx::query("DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
        .bind(workspace_id)
        .bind(target)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
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
