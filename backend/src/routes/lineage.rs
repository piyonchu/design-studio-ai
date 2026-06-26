use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use super::assets::{with_url, ASSET_COLS};
use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{Asset, AssetLink, LineageGraph, ReconcileRequest, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/lineage", get(lineage))
        .route("/projects/:project_id/reconcile", post(reconcile))
}

/// The project's asset graph: all assets (nodes) + their derivation edges.
/// One call so the frontend can lay out roots → derivatives + flag stale assets
/// (those whose `canon_version_id` predates the current canon).
async fn lineage(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<LineageGraph>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;

    let assets = sqlx::query_as::<_, Asset>(&format!(
        "SELECT {ASSET_COLS} FROM assets WHERE project_id = $1 ORDER BY created_at ASC"
    ))
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(with_url)
    .collect();

    let links = sqlx::query_as::<_, AssetLink>(
        "SELECT l.from_asset, l.to_asset, l.relation::text AS relation
         FROM asset_links l
         JOIN assets a ON a.id = l.from_asset
         WHERE a.project_id = $1 AND l.relation = 'derived_from'",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(LineageGraph { assets, links }))
}

/// Rebind assets to the current canon — the "keep" half of canon propagation.
/// After a canon change, assets generated under an older version read as stale;
/// "keep" accepts them as-is by stamping the current canon id, clearing the flag
/// without regenerating. (Regenerate is the frontend re-running generate/derive.)
async fn reconcile(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<ReconcileRequest>,
) -> Result<Json<Vec<Asset>>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;

    let current: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM canon WHERE project_id = $1 ORDER BY version DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?;
    let current = current.ok_or_else(|| AppError::BadRequest("project has no canon yet".into()))?;

    let updated = sqlx::query_as::<_, Asset>(&format!(
        "UPDATE assets SET canon_version_id = $1
         WHERE project_id = $2 AND id = ANY($3::uuid[])
         RETURNING {ASSET_COLS}"
    ))
    .bind(current)
    .bind(project_id)
    .bind(&body.asset_ids)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(with_url)
    .collect();

    Ok(Json(updated))
}
