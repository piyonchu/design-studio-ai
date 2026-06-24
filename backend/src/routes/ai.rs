use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::ai;
use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{
    Artifact, ArtifactKind, ArtifactVersion, ArtifactWithHead, WorkspaceRole,
};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/artifacts/generate", post(generate))
        .route("/artifacts/:id/ai-edit", post(ai_edit))
}

#[derive(Debug, Deserialize)]
struct GenerateRequest {
    kind: ArtifactKind,
    prompt: String,
    #[serde(default)]
    parent_artifact_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct AiEditRequest {
    prompt: String,
}

const ARTIFACT_COLS: &str = "id, project_id, kind, name, head_version_id, created_at";
const VERSION_COLS: &str =
    "id, artifact_id, parent_id, content, change_source, change_summary, prompt, created_at";

fn derive_name(prompt: &str) -> String {
    let name: String = prompt.trim().chars().take(80).collect();
    if name.is_empty() {
        "Untitled".to_string()
    } else {
        name
    }
}

/// Generate a new artifact (+ initial AI version) from a prompt, optionally
/// derived from a parent artifact whose DSL is fed in as context.
async fn generate(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<GenerateRequest>,
) -> Result<(StatusCode, Json<ArtifactWithHead>), AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;

    // Optional parent context (must be visible to the caller).
    let context: Option<Value> = match body.parent_artifact_id {
        Some(parent_id) => {
            auth::require_artifact_access(&state.pool, parent_id, user.id, WorkspaceRole::Viewer)
                .await?;
            sqlx::query_scalar::<_, Value>(
                "SELECT av.content FROM artifacts a
                 JOIN artifact_versions av ON av.id = a.head_version_id
                 WHERE a.id = $1",
            )
            .bind(parent_id)
            .fetch_optional(&state.pool)
            .await?
        }
        None => None,
    };

    let content = ai::generate_dsl(body.kind, &body.prompt, context.as_ref()).await?;

    let mut tx = state.pool.begin().await?;

    let mut artifact = sqlx::query_as::<_, Artifact>(&format!(
        "INSERT INTO artifacts (project_id, kind, name) VALUES ($1, $2, $3) RETURNING {ARTIFACT_COLS}"
    ))
    .bind(project_id)
    .bind(body.kind)
    .bind(derive_name(&body.prompt))
    .fetch_one(&mut *tx)
    .await?;

    let version = sqlx::query_as::<_, ArtifactVersion>(&format!(
        "INSERT INTO artifact_versions (artifact_id, parent_id, content, change_source, prompt)
         VALUES ($1, NULL, $2, 'ai', $3) RETURNING {VERSION_COLS}"
    ))
    .bind(artifact.id)
    .bind(content)
    .bind(&body.prompt)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("UPDATE artifacts SET head_version_id = $1 WHERE id = $2")
        .bind(version.id)
        .bind(artifact.id)
        .execute(&mut *tx)
        .await?;

    // Record the pipeline edge: parent → generated artifact.
    if let Some(parent_id) = body.parent_artifact_id {
        sqlx::query(
            "INSERT INTO artifact_links (from_artifact_id, to_artifact_id, relation)
             VALUES ($1, $2, 'derived_from')
             ON CONFLICT (from_artifact_id, to_artifact_id, relation) DO NOTHING",
        )
        .bind(parent_id)
        .bind(artifact.id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    artifact.head_version_id = Some(version.id);
    Ok((
        StatusCode::CREATED,
        Json(ArtifactWithHead {
            artifact,
            head_version: Some(version),
        }),
    ))
}

/// Edit an artifact via the AI patch loop: feed the current DSL + a prompt to
/// the model, validate the returned DSL, and append a new immutable version.
async fn ai_edit(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AiEditRequest>,
) -> Result<(StatusCode, Json<ArtifactVersion>), AppError> {
    auth::require_artifact_access(&state.pool, id, user.id, WorkspaceRole::Editor).await?;

    let kind = sqlx::query_scalar::<_, ArtifactKind>("SELECT kind FROM artifacts WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let current = sqlx::query_scalar::<_, Value>(
        "SELECT av.content FROM artifacts a
         JOIN artifact_versions av ON av.id = a.head_version_id
         WHERE a.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let content = ai::generate_dsl(kind, &body.prompt, current.as_ref()).await?;
    let summary = format!("AI edit: {}", derive_name(&body.prompt));

    // Append a version (parent = current head) and advance the head pointer.
    let mut tx = state.pool.begin().await?;
    let current_head: Option<Uuid> =
        sqlx::query_scalar("SELECT head_version_id FROM artifacts WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;

    let version = sqlx::query_as::<_, ArtifactVersion>(&format!(
        "INSERT INTO artifact_versions
            (artifact_id, parent_id, content, change_source, change_summary, prompt)
         VALUES ($1, $2, $3, 'ai', $4, $5) RETURNING {VERSION_COLS}"
    ))
    .bind(id)
    .bind(current_head)
    .bind(content)
    .bind(summary)
    .bind(&body.prompt)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("UPDATE artifacts SET head_version_id = $1 WHERE id = $2")
        .bind(version.id)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(version)))
}
