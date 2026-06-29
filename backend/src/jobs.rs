//! DB-backed async job queue + an in-process worker that drains it.
//!
//! Long / batch generation is decoupled from the request: a handler `enqueue`s
//! a job (mig 0012 `jobs` table) and the client polls its status, while the
//! worker — spawned once in `main` — claims queued jobs one at a time with
//! `FOR UPDATE SKIP LOCKED` (so multiple workers/instances never double-run a
//! job) and runs them via the same `run_generate` core the sync route uses.
//!
//! No external broker: it's a single Postgres table + a tokio task, durable
//! across restarts (a job interrupted mid-run is left `running`; a future sweep
//! could requeue stale ones — out of scope now).

use std::time::Duration;

use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::Job;
use crate::AppState;

pub(crate) const JOB_COLS: &str =
    "id, project_id, kind, status, payload, result, error, attempts, created_at, started_at, finished_at";

const POLL: Duration = Duration::from_millis(500);

/// Enqueue a job and return the queued row.
pub async fn enqueue(
    pool: &PgPool,
    project_id: Uuid,
    kind: &str,
    payload: Value,
) -> Result<Job, AppError> {
    let job = sqlx::query_as::<_, Job>(&format!(
        "INSERT INTO jobs (project_id, kind, payload) VALUES ($1, $2, $3) RETURNING {JOB_COLS}"
    ))
    .bind(project_id)
    .bind(kind)
    .bind(payload)
    .fetch_one(pool)
    .await?;
    Ok(job)
}

/// Atomically claim the oldest queued job (marks it `running`). `SKIP LOCKED`
/// makes concurrent claims safe.
async fn claim(pool: &PgPool) -> Result<Option<Job>, sqlx::Error> {
    sqlx::query_as::<_, Job>(&format!(
        "UPDATE jobs SET status = 'running', started_at = now(), attempts = attempts + 1
         WHERE id = (
             SELECT id FROM jobs WHERE status = 'queued'
             ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED
         )
         RETURNING {JOB_COLS}"
    ))
    .fetch_optional(pool)
    .await
}

async fn finish(pool: &PgPool, id: Uuid, status: &str, result: Option<Value>, error: Option<String>) {
    let r = sqlx::query(
        "UPDATE jobs SET status = $2, result = $3, error = $4, finished_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .bind(result)
    .bind(error)
    .execute(pool)
    .await;
    if let Err(e) = r {
        tracing::error!(job = %id, error = %e, "failed to record job outcome");
    }
}

/// Run one claimed job to completion, recording success/failure.
async fn run(state: &AppState, job: Job) {
    let outcome = match job.kind.as_str() {
        "generate" => {
            let prompt = job.payload.get("prompt").and_then(Value::as_str).unwrap_or("");
            let count = job.payload.get("count").and_then(Value::as_u64).unwrap_or(1) as u32;
            let created_by = job
                .payload
                .get("created_by")
                .and_then(Value::as_str)
                .and_then(|s| s.parse::<Uuid>().ok());
            if prompt.trim().is_empty() {
                Err(AppError::BadRequest("job payload missing 'prompt'".into()))
            } else {
                crate::routes::run_generate(state, job.project_id, prompt, count, created_by).await
            }
        }
        other => Err(AppError::BadRequest(format!("unknown job kind '{other}'"))),
    };

    match outcome {
        Ok(assets) => {
            let ids: Vec<Uuid> = assets.iter().map(|a| a.id).collect();
            finish(&state.pool, job.id, "succeeded", Some(json!({ "asset_ids": ids })), None).await;
        }
        Err(e) => {
            tracing::warn!(job = %job.id, error = %e, "job failed");
            finish(&state.pool, job.id, "failed", None, Some(e.to_string())).await;
        }
    }
}

/// Process up to `max` queued jobs and return how many ran. This is the
/// request-driven counterpart to the polling worker: on a scale-to-zero host
/// (e.g. Cloud Run, where a background loop can't run between requests) a
/// scheduler hits an endpoint that calls this. Safe to run concurrently with
/// the worker (both claim via `SKIP LOCKED`).
pub async fn drain(state: &AppState, max: usize) -> usize {
    let mut done = 0;
    for _ in 0..max {
        match claim(&state.pool).await {
            Ok(Some(job)) => {
                run(state, job).await;
                done += 1;
            }
            Ok(None) => break,
            Err(e) => {
                tracing::error!(error = %e, "job claim failed during drain");
                break;
            }
        }
    }
    done
}

/// Spawn the background worker. Drains queued jobs serially, polling every
/// [`POLL`] when idle. Call once after building `AppState`.
pub fn spawn_worker(state: AppState) {
    tokio::spawn(async move {
        tracing::info!("job worker started");
        loop {
            match claim(&state.pool).await {
                Ok(Some(job)) => run(&state, job).await,
                Ok(None) => tokio::time::sleep(POLL).await,
                Err(e) => {
                    tracing::error!(error = %e, "job claim failed");
                    tokio::time::sleep(POLL).await;
                }
            }
        }
    });
}
