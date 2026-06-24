use anyhow::Context;
use sqlx::postgres::{PgPool, PgPoolOptions};

/// Build a Postgres connection pool from `DATABASE_URL`.
pub async fn connect() -> anyhow::Result<PgPool> {
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .context("failed to connect to Postgres")
}

/// Apply all pending migrations from `./migrations` at startup.
pub async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("failed to run migrations")?;
    Ok(())
}
