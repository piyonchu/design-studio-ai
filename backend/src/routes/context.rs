use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value;
use uuid::Uuid;

use crate::ai::{embeddings, llm};
use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{ContextAnswer, ContextHit, SearchQuery, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/context", get(ask))
        .route("/projects/:project_id/context/backfill", post(backfill))
}

/// Ask a question about the project — retrieves the most relevant context
/// snippets (brief, asset prompts, comments, canon) by semantic similarity.
/// (Retrieval only; an LLM synthesis layer can sit on top later.)
async fn ask(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<ContextAnswer>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;
    let Some(vec) = embeddings::embed_text(&q.q, embeddings::SEMANTIC_DIM).await else {
        return Ok(Json(ContextAnswer { answer: String::new(), sources: Vec::new() }));
    };
    let pg = embeddings::to_pgvector(&vec);
    let sources = sqlx::query_as::<_, ContextHit>(
        "SELECT source_kind, source_id, content, 1 - (embedding <=> $2::vector) AS score
         FROM semantic_embeddings
         WHERE project_id = $1
         ORDER BY embedding <=> $2::vector
         LIMIT 8",
    )
    .bind(project_id)
    .bind(pg)
    .fetch_all(&state.pool)
    .await?;

    // Synthesize an answer from the strongest snippets (retrieve → synthesize).
    let notes: Vec<String> = sources
        .iter()
        .filter(|s| s.score > 0.05)
        .take(5)
        .map(|s| s.content.clone())
        .collect();
    let answer = llm::synthesize(&q.q, &notes).await?;

    Ok(Json(ContextAnswer { answer, sources }))
}

/// (Re)build the project's semantic index from scratch: brief + every asset
/// prompt/derivation + every comment + the current canon's style. Cheap (mock
/// embedder) and idempotent — wipes the project's rows first.
async fn backfill(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    sqlx::query("DELETE FROM semantic_embeddings WHERE project_id = $1")
        .bind(project_id)
        .execute(&state.pool)
        .await?;

    let mut indexed = 0u32;

    // Brief.
    if let Some(brief) = sqlx::query_scalar::<_, Option<String>>("SELECT brief FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await?
        .flatten()
    {
        if !brief.trim().is_empty() {
            embeddings::index_semantic(&state.pool, project_id, "brief", None, &brief).await?;
            indexed += 1;
        }
    }

    // Asset prompts / derivations.
    let assets: Vec<(Uuid, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT id, prompt, derivation FROM assets WHERE project_id = $1")
            .bind(project_id)
            .fetch_all(&state.pool)
            .await?;
    for (id, prompt, derivation) in assets {
        if let Some(text) = prompt.or(derivation) {
            embeddings::index_semantic(&state.pool, project_id, "asset_prompt", Some(id), &text).await?;
            indexed += 1;
        }
    }

    // Comments.
    let comments: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT c.id, c.body FROM asset_comments c
         JOIN assets a ON a.id = c.asset_id WHERE a.project_id = $1",
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    for (id, body) in comments {
        embeddings::index_semantic(&state.pool, project_id, "comment", Some(id), &body).await?;
        indexed += 1;
    }

    // Current canon style (flatten the JSON string values).
    if let Some(data) = sqlx::query_scalar::<_, Value>(
        "SELECT data FROM canon WHERE project_id = $1 ORDER BY version DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?
    {
        let text = flatten_strings(&data);
        if !text.trim().is_empty() {
            embeddings::index_semantic(&state.pool, project_id, "canon", None, &text).await?;
            indexed += 1;
        }
    }

    Ok(Json(serde_json::json!({ "indexed": indexed })))
}

/// Collect all string scalars in a JSON value into one space-joined blob.
fn flatten_strings(v: &Value) -> String {
    let mut out: Vec<String> = Vec::new();
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) => out.push(s.clone()),
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            Value::Object(o) => o.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(v, &mut out);
    out.join(" ")
}
