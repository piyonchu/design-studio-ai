use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::jobs::{self, JOB_COLS};
use crate::models::{EnqueueGenerate, Job, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects/:project_id/jobs", post(enqueue).get(list))
        .route("/jobs/:job_id", get(get_one))
        // Machine endpoint (Cloud Scheduler / cron) — distinct prefix so it
        // never collides with `/jobs/:job_id`.
        .route("/internal/jobs/drain", post(drain))
}

/// Drain queued jobs on demand — for scale-to-zero hosts where the in-process
/// worker can't run between requests, a scheduler calls this every minute.
/// Guarded by a shared secret (`JOBS_DRAIN_SECRET`); 404 when unset so the
/// endpoint is inert unless deliberately enabled.
async fn drain(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, AppError> {
    let secret = std::env::var("JOBS_DRAIN_SECRET")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .ok_or(AppError::NotFound)?;
    let provided = headers.get("x-drain-secret").and_then(|v| v.to_str().ok()).unwrap_or("");
    // Constant-ish comparison is overkill for a deploy secret; a plain check is fine.
    if provided != secret {
        return Err(AppError::Unauthorized);
    }
    let processed = jobs::drain(&state, 25).await;
    Ok(Json(json!({ "processed": processed })))
}

/// Enqueue an async generation. Returns the `queued` job immediately; the
/// worker runs it and the client polls `GET /jobs/:id` for the result.
async fn enqueue(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<EnqueueGenerate>,
) -> Result<(StatusCode, Json<Job>), AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    if body.prompt.trim().is_empty() {
        return Err(AppError::BadRequest("prompt is required".into()));
    }
    // Reject disallowed prompts at enqueue for immediate feedback (the worker's
    // run_generate re-checks, so the gate holds regardless of entry point).
    crate::moderation::check_prompt(&body.prompt)?;
    let payload = json!({
        "prompt": body.prompt,
        "count": body.count.unwrap_or(1).clamp(1, 4),
    });
    let job = jobs::enqueue(&state.pool, project_id, "generate", payload).await?;
    Ok((StatusCode::CREATED, Json(job)))
}

/// Recent jobs for a project, newest first (for a status list / banner).
async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<Job>>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;
    let rows = sqlx::query_as::<_, Job>(&format!(
        "SELECT {JOB_COLS} FROM jobs WHERE project_id = $1 ORDER BY created_at DESC LIMIT 50"
    ))
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

/// One job's current status (polled by the client). Access is gated on the
/// job's project.
async fn get_one(
    State(state): State<AppState>,
    user: AuthUser,
    Path(job_id): Path<Uuid>,
) -> Result<Json<Job>, AppError> {
    let job = sqlx::query_as::<_, Job>(&format!("SELECT {JOB_COLS} FROM jobs WHERE id = $1"))
        .bind(job_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;
    auth::require_project_access(&state.pool, job.project_id, user.id, WorkspaceRole::Viewer).await?;
    Ok(Json(job))
}
