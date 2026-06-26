//! Library crate for the design-studio backend. Exposes the module tree, the
//! shared [`AppState`], and [`app`] (the full router) so both the binary
//! (`main.rs`) and integration tests (`tests/`) build the same application.

pub mod ai;
pub mod auth;
pub mod db;
pub mod error;
pub mod export;
pub mod models;
pub mod ratelimit;
pub mod routes;
pub mod storage;
pub mod turnstile;
pub mod verticals;

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};
use sqlx::postgres::PgPool;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::storage::Storage;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub storage: Arc<Storage>,
}

/// The full application: all API routes + `/health`, with CORS and tracing.
/// The per-IP rate-limit layer is intentionally NOT applied here (it needs
/// `ConnectInfo`); the binary adds it on top. Tests drive this router directly.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(routes::router())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    let db = match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
        Ok(_) => "ok",
        Err(err) => {
            tracing::error!(%err, "health check db query failed");
            "error"
        }
    };

    Json(json!({ "status": "ok", "service": "design-studio-backend", "db": db }))
}
