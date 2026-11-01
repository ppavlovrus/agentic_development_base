//! Composition root: build dependencies, start the HTTP server.

use std::sync::Arc;

use anyhow::Context;
use app::services::TaskService;
use infra::cache::RedisClient;
use infra::config::Settings;
use infra::database::{self, task_repository::PostgresTaskRepository};
use infra::health::DependencyHealth;
use infra::messaging::{RabbitEventPublisher, connect_with_retry};

use server::build_router;
use server::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::load().context("load settings")?;
    infra::telemetry::init("server").context("init telemetry")?;

    let pool = database::connect_pool(&settings.database)
        .await
        .context("connect to postgres")?;
    database::run_migrations(&pool)
        .await
        .context("apply migrations")?;

    let redis = RedisClient::connect(&settings.redis)
        .await
        .context("connect to redis")?;
    let rabbit = connect_with_retry(&settings.rabbitmq)
        .await
        .context("connect to rabbitmq")?;

    let repository = Arc::new(PostgresTaskRepository::new(pool.clone()));
    let publisher = Arc::new(RabbitEventPublisher::new(&rabbit).await?);
    let state = AppState {
        tasks: Arc::new(TaskService::new(repository, publisher)),
        health: Arc::new(DependencyHealth::new(pool, redis, rabbit)),
    };

    let addr = format!(
        "{}:{}",
        settings.application.host, settings.application.port
    );
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    tracing::info!(%addr, "server started");

    axum::serve(listener, build_router(state))
        .with_graceful_shutdown(infra::shutdown::shutdown_signal())
        .await
        .context("server error")?;
    Ok(())
}
