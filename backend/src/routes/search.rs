use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use super::assets::{asset_bytes, with_url, ASSET_COLS};
use crate::ai::embeddings;
use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::mirror;
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

/// Visual style-fit of an asset vs the project's *approved* assets: the max
/// cosine similarity of its pixel embedding to any approved asset's, plus how
/// many approved peers it was compared against. `(None, 0)` when there's nothing
/// to compare to (no approved peers / no embeddings). Reused by the endpoint and
/// the QA gate (auto-scoring on generation).
pub(crate) async fn style_fit_score(
    pool: &sqlx::PgPool,
    asset_id: Uuid,
    project_id: Uuid,
) -> Result<(Option<f64>, i64), AppError> {
    // MAX(...) is NULL when there's no other approved asset with a visual
    // embedding to compare against, so column 0 must decode as Option.
    let row: Option<(Option<f64>, i64)> = sqlx::query_as(
        "WITH q AS (SELECT embedding_visual FROM visual_embeddings WHERE asset_id = $1)
         SELECT MAX(1 - (e.embedding_visual <=> q.embedding_visual))::float8,
                COUNT(*)::int8
         FROM visual_embeddings e
         JOIN assets a ON a.id = e.asset_id, q
         WHERE a.project_id = $2 AND a.status = 'approved' AND a.id <> $1
           AND e.embedding_visual IS NOT NULL AND q.embedding_visual IS NOT NULL",
    )
    .bind(asset_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    Ok(match row {
        Some((Some(s), n)) if n > 0 => (Some(s), n),
        _ => (None, 0),
    })
}

/// How well an asset matches the project's *approved* style via pixel embedding.
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

    let (score, basis) = style_fit_score(&state.pool, id, project_id).await?;
    Ok(Json(serde_json::json!({ "score": score, "basis": basis })))
}

#[derive(Clone, Copy)]
enum RankMode {
    /// Board search — fuse caption + cross-modal visual space.
    Search,
    /// Pre-generate dedup — emphasize caption similarity.
    Dedup,
}

/// Embed a query and rank project assets. Fuses text + visual columns when both
/// are present. `score` is in [0, 1] (1.0 = identical).
async fn ranked(
    state: &AppState,
    project_id: Uuid,
    query_text: &str,
    limit: i64,
    mode: RankMode,
) -> Result<Vec<ScoredAsset>, AppError> {
    let text_w = match mode {
        RankMode::Dedup => 0.75,
        RankMode::Search => 0.5,
    };
    let visual_w = 1.0 - text_w;

    let text_q = embeddings::embed_text(query_text, embeddings::VISUAL_DIM).await;
    let visual_q = if visual_w > 0.0 {
        embeddings::embed_query_visual_space(query_text, embeddings::VISUAL_DIM).await
    } else {
        None
    };

    if text_q.is_none() && visual_q.is_none() {
        return Ok(Vec::new());
    }

    let text_pg = text_q.as_ref().map(|v| embeddings::to_pgvector(v));
    let visual_pg = visual_q.as_ref().map(|v| embeddings::to_pgvector(v));

    let rows = sqlx::query_as::<_, ScoredAsset>(
        "SELECT assets.*,
                (
                    COALESCE((1 - (e.embedding_text <=> $2::vector)) * $5::float8, 0)
                  + COALESCE((1 - (e.embedding_visual <=> $3::vector)) * $6::float8, 0)
                ) AS score
         FROM assets
         JOIN visual_embeddings e ON e.asset_id = assets.id
         WHERE assets.project_id = $1
           AND (
                ($2::vector IS NOT NULL AND e.embedding_text IS NOT NULL)
             OR ($3::vector IS NOT NULL AND e.embedding_visual IS NOT NULL)
           )
         ORDER BY score DESC
         LIMIT $4",
    )
    .bind(project_id)
    .bind(text_pg)
    .bind(visual_pg)
    .bind(limit)
    .bind(text_w)
    .bind(visual_w)
    .fetch_all(&state.pool)
    .await?;

    Ok(rows.into_iter().map(|s| ScoredAsset { asset: with_url(s.asset), score: s.score }).collect())
}

async fn search(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Vec<ScoredAsset>>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;
    Ok(Json(ranked(&state, project_id, &q.q, 40, RankMode::Search).await?))
}

async fn similar_check(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<SimilarCheck>,
) -> Result<Json<Vec<ScoredAsset>>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;
    let mut hits = ranked(&state, project_id, &body.prompt, 5, RankMode::Dedup).await?;
    hits.retain(|s| s.score >= 0.6);
    Ok(Json(hits))
}

/// Nearest pixel neighbours of an asset (excludes itself).
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
        "SELECT assets.*, 1 - (e.embedding_visual <=> q.embedding_visual) AS score
         FROM assets
         JOIN visual_embeddings e ON e.asset_id = assets.id
         JOIN visual_embeddings q ON q.asset_id = $1
         WHERE assets.project_id = $2 AND assets.id <> $1
           AND e.embedding_visual IS NOT NULL AND q.embedding_visual IS NOT NULL
         ORDER BY e.embedding_visual <=> q.embedding_visual
         LIMIT 12",
    )
    .bind(id)
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows.into_iter().map(|s| ScoredAsset { asset: with_url(s.asset), score: s.score }).collect()))
}

async fn load_image_bytes(state: &AppState, asset: &Asset) -> Option<Vec<u8>> {
    if !embeddings::is_image_asset(asset) {
        return None;
    }
    if let Some((bytes, _)) = mirror::read_any(asset.project_id, asset.id) {
        return Some(bytes);
    }
    if let Ok((bytes, _)) = asset_bytes(state, &asset.s3_key, asset.mime_type.as_deref()).await {
        return Some(bytes);
    }
    None
}

/// Index every image asset missing a text or visual embedding (covers imports,
/// legacy rows, and deploy mirror re-import).
async fn backfill(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    let assets = sqlx::query_as::<_, Asset>(&format!(
        "SELECT {ASSET_COLS} FROM assets a
         WHERE a.project_id = $1 AND a.kind <> 'audio'
           AND (
                a.id NOT IN (SELECT asset_id FROM visual_embeddings)
             OR a.id IN (
                    SELECT asset_id FROM visual_embeddings
                    WHERE embedding_text IS NULL OR embedding_visual IS NULL
                )
           )"
    ))
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    let mut indexed = 0;
    for a in &assets {
        let bytes = load_image_bytes(&state, a).await;
        embeddings::index_asset_with_bytes(&state.pool, a, bytes.as_deref()).await?;
        indexed += 1;
    }
    Ok(Json(serde_json::json!({ "indexed": indexed })))
}
