//! PostgreSQL connection management and repository implementations.

pub mod task_repository;

use std::time::Duration;

use secrecy::ExposeSecret;
use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::config::DatabaseSettings;

pub type DbPool = PgPool;

/// Connect a pool with sane acquisition timeout (5s covers container startup).
pub async fn connect_pool(settings: &DatabaseSettings) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(settings.max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(settings.connection_string().expose_secret())
        .await
}

/// Apply pending migrations from the workspace-level `migrations/` directory.
/// Called by every binary at startup so the schema is always current.
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("../../migrations").run(pool).await
}
