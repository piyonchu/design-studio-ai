use axum::routing::get;
use axum::{Json, Router};

use crate::ai::usage::{self, Usage};
use crate::auth::AuthUser;
use crate::error::AppError;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/usage", get(key_usage))
}

/// The shared OpenRouter key's remaining budget (dev visibility). Auth-gated so
/// it isn't exposed publicly; always succeeds (mock / stale fallback inside).
async fn key_usage(_user: AuthUser) -> Result<Json<Usage>, AppError> {
    Ok(Json(usage::key_balance().await))
}
