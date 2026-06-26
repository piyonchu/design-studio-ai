use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{Canon, CreateCanon, WorkspaceRole};
use crate::AppState;

pub fn router() -> Router<AppState> {
    // GET = current canon, POST = append a new version.
    Router::new()
        .route("/projects/:project_id/canon", get(latest).post(create))
        .route("/projects/:project_id/canon/history", get(history))
}

const CANON_COLS: &str = "id, project_id, parent_id, version, data, change_note, created_at";

/// Diff two canon `data` blobs into a short, human "what changed" note. Pure +
/// deterministic — no LLM. Returns None when nothing meaningful changed.
fn diff_note(parent: Option<&Value>, next: &Value) -> Option<String> {
    let Some(parent) = parent else {
        return Some("Initial canon.".to_string());
    };
    let style = |v: &Value| {
        v.get("style")
            .and_then(Value::as_object)
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect::<std::collections::BTreeMap<_, _>>()
            })
            .unwrap_or_default()
    };
    let negs = |v: &Value| {
        v.get("negative")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let (ps, ns) = (style(parent), style(next));
    let mut parts: Vec<String> = Vec::new();

    for (k, nv) in &ns {
        match ps.get(k) {
            None => parts.push(format!("set {k} to “{nv}”")),
            Some(pv) if pv != nv => parts.push(format!("{k}: “{pv}” → “{nv}”")),
            _ => {}
        }
    }
    for k in ps.keys() {
        if !ns.contains_key(k) {
            parts.push(format!("cleared {k}"));
        }
    }

    let (pn, nn) = (negs(parent), negs(next));
    let added = nn.iter().filter(|x| !pn.contains(x)).count();
    let removed = pn.iter().filter(|x| !nn.contains(x)).count();
    if added > 0 {
        parts.push(format!("+{added} negative{}", if added > 1 { "s" } else { "" }));
    }
    if removed > 0 {
        parts.push(format!("-{removed} negative{}", if removed > 1 { "s" } else { "" }));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::diff_note;
    use serde_json::json;

    #[test]
    fn initial_canon_has_a_note() {
        assert_eq!(diff_note(None, &json!({})).as_deref(), Some("Initial canon."));
    }

    #[test]
    fn changed_field_names_old_and_new() {
        let prev = json!({ "style": { "palette": "warm earthy" } });
        let next = json!({ "style": { "palette": "high-contrast neon" } });
        let note = diff_note(Some(&prev), &next).unwrap();
        assert!(note.contains("palette"));
        assert!(note.contains("warm earthy") && note.contains("high-contrast neon"));
    }

    #[test]
    fn added_negative_is_counted() {
        let prev = json!({ "style": {}, "negative": [] });
        let next = json!({ "style": {}, "negative": ["no text"] });
        assert!(diff_note(Some(&prev), &next).unwrap().contains("+1 negative"));
    }

    #[test]
    fn no_change_is_none() {
        let v = json!({ "style": { "palette": "warm" }, "negative": ["x"] });
        assert!(diff_note(Some(&v), &v).is_none());
    }
}

/// The current (highest-version) canon for a project; 404 if none defined yet.
async fn latest(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Canon>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;

    let canon = sqlx::query_as::<_, Canon>(&format!(
        "SELECT {CANON_COLS} FROM canon WHERE project_id = $1 ORDER BY version DESC LIMIT 1"
    ))
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(canon))
}

/// Append a new canon version: parent = current head, version auto-incremented.
/// Immutable lineage so a style change is "v2, keep or regenerate?" not a destroy.
async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateCanon>,
) -> Result<(StatusCode, Json<Canon>), AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;

    let head: Option<(Uuid, i32, Value)> = sqlx::query_as(
        "SELECT id, version, data FROM canon WHERE project_id = $1 ORDER BY version DESC LIMIT 1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?;
    let (parent_id, version, parent_data) = match head {
        Some((id, v, data)) => (Some(id), v + 1, Some(data)),
        None => (None, 1, None),
    };
    let note = diff_note(parent_data.as_ref(), &body.data);

    let canon = sqlx::query_as::<_, Canon>(&format!(
        "INSERT INTO canon (project_id, parent_id, version, data, change_note)
         VALUES ($1, $2, $3, $4, $5) RETURNING {CANON_COLS}"
    ))
    .bind(project_id)
    .bind(parent_id)
    .bind(version)
    .bind(&body.data)
    .bind(note)
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(canon)))
}

/// Full canon version history, newest first — each with its diff note.
async fn history(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<Canon>>, AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Viewer).await?;
    let rows = sqlx::query_as::<_, Canon>(&format!(
        "SELECT {CANON_COLS} FROM canon WHERE project_id = $1 ORDER BY version DESC"
    ))
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}
