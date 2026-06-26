use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{ActivityEvent, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/projects/:project_id/activity", get(feed))
}

const LIMIT: i64 = 50;

/// A project's merged activity stream — recent asset creations, comments, and
/// canon versions, newest first. Read-only over existing tables (no new schema).
async fn feed(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<ActivityEvent>>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;

    let mut events: Vec<ActivityEvent> = Vec::new();

    // Asset creations — phrased by how the asset entered the library.
    let assets: Vec<(Uuid, String, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, COALESCE(name, role, kind::text) AS label, source_kind, created_at
         FROM assets WHERE project_id = $1 ORDER BY created_at DESC LIMIT $2",
    )
    .bind(project_id)
    .bind(LIMIT)
    .fetch_all(&state.pool)
    .await?;
    for (id, label, source_kind, at) in assets {
        let verb = match source_kind.as_str() {
            "uploaded" => "Uploaded",
            "derived" => "Derived",
            _ => "Generated",
        };
        events.push(ActivityEvent { kind: "asset".into(), at, summary: format!("{verb} {label}"), asset_id: Some(id) });
    }

    // Comments — with author + a snippet.
    let comments: Vec<(Uuid, String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT c.asset_id, c.body, u.email, c.created_at
         FROM asset_comments c
         JOIN assets a ON a.id = c.asset_id
         LEFT JOIN users u ON u.id = c.author_id
         WHERE a.project_id = $1 ORDER BY c.created_at DESC LIMIT $2",
    )
    .bind(project_id)
    .bind(LIMIT)
    .fetch_all(&state.pool)
    .await?;
    for (asset_id, body, email, at) in comments {
        let who = email.unwrap_or_else(|| "someone".into());
        let snippet: String = body.chars().take(60).collect();
        events.push(ActivityEvent {
            kind: "comment".into(),
            at,
            summary: format!("{who} commented: “{snippet}”"),
            asset_id: Some(asset_id),
        });
    }

    // Canon versions — with the auto change-note.
    let canons: Vec<(i32, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT version, change_note, created_at
         FROM canon WHERE project_id = $1 ORDER BY version DESC LIMIT $2",
    )
    .bind(project_id)
    .bind(LIMIT)
    .fetch_all(&state.pool)
    .await?;
    for (version, note, at) in canons {
        let summary = match note {
            Some(n) => format!("Canon v{version} — {n}"),
            None => format!("Canon v{version}"),
        };
        events.push(ActivityEvent { kind: "canon".into(), at, summary, asset_id: None });
    }

    events.sort_by(|a, b| b.at.cmp(&a.at));
    events.truncate(LIMIT as usize);
    Ok(Json(events))
}
