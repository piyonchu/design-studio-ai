use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use super::assets::{with_url, ASSET_COLS};
use crate::ai::embeddings;
use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{Asset, ScoredAsset, SearchQuery, SimilarCheck, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/assets/search", get(search))
        .route("/projects/:project_id/assets/similar-check", post(similar_check))
        .route("/projects/:project_id/embeddings/backfill", post(backfill))
        .route("/assets/:id/similar", get(similar))
        .route("/assets/:id/style-fit", get(style_fit))
}

/// How well an asset matches the project's *approved* style: the cosine
/// similarity of its embedding to the nearest approved asset (0–1). A credible,
/// embedding-based check to surface at review time (PLAN §6). `score` is null
/// when the asset has no embedding or there are no other approved assets yet.
async fn style_fit(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project_id: Option<Uuid> = sqlx::query_scalar("SELECT project_id FROM assets WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
    let project_id = project_id.ok_or(AppError::NotFound)?;
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;

    let row: Option<(f64, i64)> = sqlx::query_as(
        "WITH q AS (SELECT embedding FROM visual_embeddings WHERE asset_id = $1)
         SELECT MAX(1 - (e.embedding <=> q.embedding))::float8,
                COUNT(*)::int8
         FROM visual_embeddings e
         JOIN assets a ON a.id = e.asset_id, q
         WHERE a.project_id = $2 AND a.status = 'approved' AND a.id <> $1",
    )
    .bind(id)
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?;

    let (score, basis) = match row {
        Some((s, n)) if n > 0 => (Some(s), n),
        _ => (None, 0),
    };
    Ok(Json(serde_json::json!({ "score": score, "basis": basis })))
}

/// Embed a query and cosine-rank the project's assets. `score = 1 - distance`
/// (1.0 = identical). Assets with no embedding simply don't appear.
async fn ranked(
    state: &AppState,
    project_id: Uuid,
    query_text: &str,
    limit: i64,
) -> Result<Vec<ScoredAsset>, AppError> {
    let Some(vec) = embeddings::embed_text(query_text, embeddings::VISUAL_DIM) else {
        return Ok(Vec::new());
    };
    let pg = embeddings::to_pgvector(&vec);
    let rows = sqlx::query_as::<_, ScoredAsset>(
        "SELECT assets.*, 1 - (e.embedding <=> $2::vector) AS score
         FROM assets
         JOIN visual_embeddings e ON e.asset_id = assets.id
         WHERE assets.project_id = $1
         ORDER BY e.embedding <=> $2::vector
         LIMIT $3",
    )
    .bind(project_id)
    .bind(pg)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows.into_iter().map(|s| ScoredAsset { asset: with_url(s.asset), score: s.score }).collect())
}

/// Smart search: semantic / keyword over the asset library.
async fn search(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Vec<ScoredAsset>>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;
    Ok(Json(ranked(&state, project_id, &q.q, 40).await?))
}

/// Pre-generate dedup nudge: does something close to this prompt already exist?
/// Returns only strong matches so the UI can warn "a similar asset already exists".
async fn similar_check(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<SimilarCheck>,
) -> Result<Json<Vec<ScoredAsset>>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;
    let mut hits = ranked(&state, project_id, &body.prompt, 5).await?;
    hits.retain(|s| s.score >= 0.6);
    Ok(Json(hits))
}

/// "Find visually similar" to a given asset (its nearest neighbours, excluding
/// itself). Authorized via the asset's project.
async fn similar(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ScoredAsset>>, AppError> {
    let project_id: Option<Uuid> = sqlx::query_scalar("SELECT project_id FROM assets WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
    let project_id = project_id.ok_or(AppError::NotFound)?;
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;

    let rows = sqlx::query_as::<_, ScoredAsset>(
        "SELECT assets.*, 1 - (e.embedding <=> q.embedding) AS score
         FROM assets
         JOIN visual_embeddings e ON e.asset_id = assets.id
         JOIN visual_embeddings q ON q.asset_id = $1
         WHERE assets.project_id = $2 AND assets.id <> $1
         ORDER BY e.embedding <=> q.embedding
         LIMIT 12",
    )
    .bind(id)
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows.into_iter().map(|s| ScoredAsset { asset: with_url(s.asset), score: s.score }).collect()))
}

/// Index every asset in the project that has no embedding yet (covers imports
/// and anything created before the pipeline existed). Returns how many.
async fn backfill(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    let assets = sqlx::query_as::<_, Asset>(&format!(
        "SELECT {ASSET_COLS} FROM assets
         WHERE project_id = $1 AND id NOT IN (SELECT asset_id FROM visual_embeddings)"
    ))
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    let mut indexed = 0;
    for a in &assets {
        embeddings::index_asset(&state.pool, a.id, &embeddings::caption_from(a)).await?;
        indexed += 1;
    }
    Ok(Json(serde_json::json!({ "indexed": indexed })))
}
