//! Library crate for the design-studio backend. Exposes the module tree, the
//! shared [`AppState`], and [`app`] (the full router) so both the binary
//! (`main.rs`) and integration tests (`tests/`) build the same application.

pub mod ai;
pub mod auth;
pub mod db;
pub mod error;
pub mod export;
pub mod jobs;
pub mod mirror;
pub mod models;
pub mod moderation;
pub mod ratelimit;
pub mod routes;
pub mod storage;
pub mod turnstile;
pub mod verticals;

use std::sync::Arc;

use axum::http::{header, HeaderValue, Method};
use axum::{extract::State, middleware, response::Response, routing::get, Json, Router};
use serde_json::{json, Value};
use sqlx::postgres::PgPool;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::storage::Storage;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub storage: Arc<Storage>,
}

/// The full application: all API routes + `/health`, with security headers,
/// CORS, and tracing. The per-IP rate-limit layer is intentionally NOT applied
/// here (it needs `ConnectInfo`); the binary adds it on top. Tests drive this
/// router directly.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .merge(routes::router())
        .layer(middleware::from_fn(security_headers))
        .layer(cors_layer())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// CORS policy. By default (`CORS_ALLOWED_ORIGINS` unset) it's permissive for
/// local dev; set a comma-separated origin allowlist in production to lock it
/// down (then credentials are allowed, with an explicit method/header set since
/// `*` can't be combined with credentials).
fn cors_layer() -> CorsLayer {
    match std::env::var("CORS_ALLOWED_ORIGINS") {
        Ok(s) if !s.trim().is_empty() => {
            let origins: Vec<HeaderValue> =
                s.split(',').filter_map(|o| o.trim().parse().ok()).collect();
            CorsLayer::new()
                .allow_origin(origins)
                .allow_credentials(true)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::PATCH,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers([header::CONTENT_TYPE])
        }
        _ => CorsLayer::permissive(),
    }
}

/// Baseline security response headers (defense-in-depth for the API + any proxied
/// assets): block MIME sniffing, framing, and referrer leakage.
async fn security_headers(req: axum::extract::Request, next: middleware::Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    res
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
