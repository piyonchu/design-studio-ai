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

    // Drain async generation jobs with the in-process worker, unless disabled
    // (JOBS_WORKER=false) — e.g. on a scale-to-zero host where a scheduler
    // hits POST /internal/jobs/drain instead.
    let worker_on = std::env::var("JOBS_WORKER")
        .map(|v| !v.trim().eq_ignore_ascii_case("false"))
        .unwrap_or(true);
    if worker_on {
        jobs::spawn_worker(state.clone());
    } else {
        tracing::info!("in-process job worker disabled (JOBS_WORKER=false); expecting external drain");
    }

    tracing::info!("rate limits: {}", ratelimit::describe());
    let global_limit = GovernorLayer {
        config: ratelimit::global_config(),
    };

    // The library builds the routed app; the binary adds the per-IP rate limit.
    let app = app(state).layer(global_limit);

    // Honor $PORT (Cloud Run / most PaaS inject it), default 8080.
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
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
