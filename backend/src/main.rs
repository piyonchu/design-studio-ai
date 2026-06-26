use std::net::SocketAddr;
use std::sync::Arc;

use tower_governor::GovernorLayer;

use design_studio_backend::storage::Storage;
use design_studio_backend::{app, db, jobs, ratelimit, AppState};

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

    // Drain async generation jobs in the background.
    jobs::spawn_worker(state.clone());

    tracing::info!("rate limits: {}", ratelimit::describe());
    let global_limit = GovernorLayer {
        config: ratelimit::global_config(),
    };

    // The library builds the routed app; the binary adds the per-IP rate limit.
    let app = app(state).layer(global_limit);

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
