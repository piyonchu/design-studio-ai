mod ai;
mod auth;
mod db;
mod error;
mod export;
mod models;
mod ratelimit;
mod routes;
mod storage;
mod turnstile;
mod verticals;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};
use sqlx::postgres::PgPool;
use tower_governor::GovernorLayer;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::storage::Storage;

#[derive(Clone)]
pub struct AppState {
    pool: PgPool,
    storage: Arc<Storage>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .init();

    let pool = db::connect().await?;
    db::migrate(&pool).await?;
    tracing::info!("database connected and migrations applied");

    let storage = Arc::new(Storage::from_env().await?);
    let state = AppState { pool, storage };

    tracing::info!("rate limits: {}", ratelimit::describe());
    let global_limit = GovernorLayer {
        config: ratelimit::global_config(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .merge(routes::router())
        .layer(global_limit)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("backend listening on http://{addr}");

    // ConnectInfo gives the governor a peer-IP fallback when no proxy headers.
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
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
