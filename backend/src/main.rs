mod db;

use std::net::SocketAddr;

use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};
use sqlx::postgres::PgPool;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

#[derive(Clone)]
struct AppState {
    pool: PgPool,
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

    let state = AppState { pool };

    let app = Router::new()
        .route("/health", get(health))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("backend listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

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
