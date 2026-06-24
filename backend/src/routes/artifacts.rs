use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{
    Artifact, ArtifactLink, ArtifactVersion, ArtifactWithHead, CreateArtifact, CreateLink,
    CreateVersion, WorkspaceRole,
};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/artifacts", post(create).get(list))
        .route("/artifacts/:id", get(get_one))
        .route("/artifacts/:id/versions", post(add_version).get(list_versions))
        .route("/artifacts/:id/links", post(add_link).get(list_links))
}

const ARTIFACT_COLS: &str =
    "id, project_id, kind, name, head_version_id, created_at";
const VERSION_COLS: &str =
    "id, artifact_id, parent_id, content, change_source, change_summary, prompt, created_at";

/// Create an artifact together with its first immutable version, atomically.
async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateArtifact>,
) -> Result<(StatusCode, Json<ArtifactWithHead>), AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;

    let mut tx = state.pool.begin().await?;

    let mut artifact = sqlx::query_as::<_, Artifact>(&format!(
        "INSERT INTO artifacts (project_id, kind, name) VALUES ($1, $2, $3) RETURNING {ARTIFACT_COLS}"
    ))
    .bind(project_id)
    .bind(body.kind)
    .bind(body.name)
    .fetch_one(&mut *tx)
    .await?;

    let version = sqlx::query_as::<_, ArtifactVersion>(&format!(
        "INSERT INTO artifact_versions (artifact_id, parent_id, content, change_source, prompt)
         VALUES ($1, NULL, $2, $3, $4) RETURNING {VERSION_COLS}"
    ))
    .bind(artifact.id)
    .bind(body.content)
    .bind(body.change_source)
    .bind(body.prompt)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("UPDATE artifacts SET head_version_id = $1 WHERE id = $2")
        .bind(version.id)
        .bind(artifact.id)
        .execute(&mut *tx)
        .await?;

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

async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<Artifact>>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;

    let rows = sqlx::query_as::<_, Artifact>(&format!(
        "SELECT {ARTIFACT_COLS} FROM artifacts WHERE project_id = $1 ORDER BY created_at DESC"
    ))
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn get_one(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ArtifactWithHead>, AppError> {
    auth::require_artifact_access(&state.pool, id, user.id, WorkspaceRole::Viewer).await?;

    let artifact = sqlx::query_as::<_, Artifact>(&format!(
        "SELECT {ARTIFACT_COLS} FROM artifacts WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let head_version = match artifact.head_version_id {
        Some(vid) => sqlx::query_as::<_, ArtifactVersion>(&format!(
            "SELECT {VERSION_COLS} FROM artifact_versions WHERE id = $1"
        ))
        .bind(vid)
        .fetch_optional(&state.pool)
        .await?,
        None => None,
    };

    Ok(Json(ArtifactWithHead {
        artifact,
        head_version,
    }))
}

/// Append a new immutable version whose `parent_id` is the current head, then
/// advance the artifact's head pointer. Atomic.
async fn add_version(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateVersion>,
) -> Result<(StatusCode, Json<ArtifactVersion>), AppError> {
    auth::require_artifact_access(&state.pool, id, user.id, WorkspaceRole::Editor).await?;

    let mut tx = state.pool.begin().await?;

    // Lock the artifact row so concurrent appends serialize on the head pointer.
    let current_head: Option<Uuid> =
        sqlx::query_scalar("SELECT head_version_id FROM artifacts WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;

    let version = sqlx::query_as::<_, ArtifactVersion>(&format!(
        "INSERT INTO artifact_versions
            (artifact_id, parent_id, content, change_source, change_summary, prompt)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING {VERSION_COLS}"
    ))
    .bind(id)
    .bind(current_head)
    .bind(body.content)
    .bind(body.change_source)
    .bind(body.change_summary)
    .bind(body.prompt)
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

async fn list_versions(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ArtifactVersion>>, AppError> {
    auth::require_artifact_access(&state.pool, id, user.id, WorkspaceRole::Viewer).await?;

    let rows = sqlx::query_as::<_, ArtifactVersion>(&format!(
        "SELECT {VERSION_COLS} FROM artifact_versions WHERE artifact_id = $1 ORDER BY created_at DESC"
    ))
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

/// Create a pipeline edge from this artifact to another. Idempotent on the
/// (from, to, relation) unique edge.
async fn add_link(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateLink>,
) -> Result<(StatusCode, Json<ArtifactLink>), AppError> {
    // Editor on the source, and at least viewer on the target — so you can't
    // link into a workspace you can't see.
    auth::require_artifact_access(&state.pool, id, user.id, WorkspaceRole::Editor).await?;
    auth::require_artifact_access(&state.pool, body.to_artifact_id, user.id, WorkspaceRole::Viewer)
        .await?;

    let link = sqlx::query_as::<_, ArtifactLink>(
        "INSERT INTO artifact_links (from_artifact_id, to_artifact_id, relation)
         VALUES ($1, $2, $3)
         ON CONFLICT (from_artifact_id, to_artifact_id, relation)
         DO UPDATE SET relation = EXCLUDED.relation
         RETURNING id, from_artifact_id, to_artifact_id, relation, created_at",
    )
    .bind(id)
    .bind(body.to_artifact_id)
    .bind(body.relation)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(link)))
}

async fn list_links(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ArtifactLink>>, AppError> {
    auth::require_artifact_access(&state.pool, id, user.id, WorkspaceRole::Viewer).await?;

    let rows = sqlx::query_as::<_, ArtifactLink>(
        "SELECT id, from_artifact_id, to_artifact_id, relation, created_at
         FROM artifact_links WHERE from_artifact_id = $1 ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}
