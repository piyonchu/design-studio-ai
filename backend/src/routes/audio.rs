use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine;
use serde_json::json;
use uuid::Uuid;

use super::assets::{with_url, ASSET_COLS};
use crate::auth::{self, AuthUser};
use crate::error::AppError;
use crate::models::{Asset, GenerateAssets, WorkspaceRole};
use crate::{ai, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/projects/:project_id/audio", post(generate))
}

/// Generate audio assets (SFX / loops) from a text prompt. Mirrors image
/// generation: stored to object storage (or inline), one asset row per clip
/// with `kind='audio'` and the duration kept in `metadata`.
async fn generate(
    State(state): State<AppState>,
    user: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<GenerateAssets>,
) -> Result<(StatusCode, Json<Vec<Asset>>), AppError> {
    auth::require_project_access(&state.pool, project_id, user.id, WorkspaceRole::Editor).await?;
    let count = body.count.unwrap_or(1).clamp(1, 4);

    let mut assets = Vec::with_capacity(count as usize);
    for n in 0..count as usize {
        let clip = ai::audio::generate_audio(&body.prompt, n).await?;

        let s3_key = if state.storage.configured() {
            let key = format!("projects/{project_id}/assets/{}", Uuid::new_v4());
            state.storage.put(&key, &clip.bytes, &clip.mime).await?;
            key
        } else {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&clip.bytes);
            format!("data:{};base64,{b64}", clip.mime)
        };

        let asset = sqlx::query_as::<_, Asset>(&format!(
            "INSERT INTO assets (project_id, kind, s3_key, mime_type, prompt, source_kind, metadata)
             VALUES ($1, 'audio', $2, $3, $4, 'seeded', $5) RETURNING {ASSET_COLS}"
        ))
        .bind(project_id)
        .bind(&s3_key)
        .bind(&clip.mime)
        .bind(&body.prompt)
        .bind(json!({ "duration_ms": clip.duration_ms }))
        .fetch_one(&state.pool)
        .await?;
        ai::embeddings::index_asset_soft(&state.pool, &asset).await;
        assets.push(with_url(asset));
    }
    Ok((StatusCode::CREATED, Json(assets)))
}
