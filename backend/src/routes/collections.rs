use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use uuid::Uuid;

use super::assets::{with_url, ASSET_COLS};
use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{
    AddItems, Asset, Collection, CollectionDetail, CollectionSummary, CreateCollection, WorkspaceRole,
};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/collections", get(list).post(create))
        .route("/collections/:id", get(get_one).delete(delete_one))
        .route("/collections/:id/items", post(add_items))
        .route("/collections/:id/items/:asset_id", delete(remove_item))
}

/// Fetch a collection and authorize the caller via its owning project.
async fn load_authorized(
    state: &AppState,
    id: Uuid,
    user_id: Uuid,
    min: WorkspaceRole,
) -> Result<Collection, AppError> {
    let c = sqlx::query_as::<_, Collection>(
        "SELECT id, project_id, name, cover_asset_id, created_at FROM collections WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    auth::require_project_access(&state.pool, c.project_id, user_id, min).await?;
    Ok(c)
}

/// List a project's collections with item count + a cover asset to thumbnail.
async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<CollectionSummary>>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;
    let rows = sqlx::query_as::<_, CollectionSummary>(
        "SELECT c.id, c.name, c.created_at,
                COUNT(ci.asset_id) AS item_count,
                COALESCE(
                  c.cover_asset_id,
                  (SELECT asset_id FROM collection_items
                   WHERE collection_id = c.id ORDER BY added_at DESC LIMIT 1)
                ) AS cover_asset_id
         FROM collections c
         LEFT JOIN collection_items ci ON ci.collection_id = c.id
         WHERE c.project_id = $1
         GROUP BY c.id
         ORDER BY c.created_at DESC",
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
    Json(body): Json<CreateCollection>,
) -> Result<(StatusCode, Json<Collection>), AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    let c = sqlx::query_as::<_, Collection>(
        "INSERT INTO collections (project_id, name) VALUES ($1, $2)
         RETURNING id, project_id, name, cover_asset_id, created_at",
    )
    .bind(project_id)
    .bind(name)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(c)))
}

/// A collection and its assets (most recently added first).
async fn get_one(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<CollectionDetail>, AppError> {
    let collection = load_authorized(&state, id, user.id, WorkspaceRole::Viewer).await?;
    let assets = sqlx::query_as::<_, Asset>(&format!(
        "SELECT {ASSET_COLS} FROM assets
         JOIN collection_items ci ON ci.asset_id = assets.id
         WHERE ci.collection_id = $1
         ORDER BY ci.added_at DESC"
    ))
    .bind(id)
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(with_url)
    .collect();
    Ok(Json(CollectionDetail { collection, assets }))
}

/// Add assets to a collection. Only assets in the same project are added;
/// duplicates are silently ignored by the (collection_id, asset_id) PK.
async fn add_items(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AddItems>,
) -> Result<StatusCode, AppError> {
    let collection = load_authorized(&state, id, user.id, WorkspaceRole::Editor).await?;
    sqlx::query(
        "INSERT INTO collection_items (collection_id, asset_id)
         SELECT $1, id FROM assets WHERE id = ANY($2::uuid[]) AND project_id = $3
         ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(&body.asset_ids)
    .bind(collection.project_id)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, asset_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    load_authorized(&state, id, user.id, WorkspaceRole::Editor).await?;
    sqlx::query("DELETE FROM collection_items WHERE collection_id = $1 AND asset_id = $2")
        .bind(id)
        .bind(asset_id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_one(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    load_authorized(&state, id, user.id, WorkspaceRole::Editor).await?;
    sqlx::query("DELETE FROM collections WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
