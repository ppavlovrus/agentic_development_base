//! Background worker: consumes `task.created` events and maintains a Redis
//! counter. The processing loop is deliberately small and explicit — copy it
//! for additional consumers (see skills/add-consumer.md).

use anyhow::Context;
use domain::events::{TaskCreated, routing_keys};
use futures_util::StreamExt;
use infra::cache::RedisClient;
use infra::config::Settings;
use infra::messaging;
use lapin::message::Delivery;
use tokio_util::sync::CancellationToken;

const COUNTER_KEY: &str = "stats:tasks_created";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::load().context("load settings")?;
    infra::telemetry::init("worker").context("init telemetry")?;

    let redis = RedisClient::connect(&settings.redis)
        .await
        .context("connect to redis")?;
    let rabbit = messaging::connect_with_retry(&settings.rabbitmq)
        .await
        .context("connect to rabbitmq")?;
    let mut consumer = messaging::consumer_for(&rabbit, routing_keys::TASK_CREATED)
        .await
        .context("create consumer")?;

    tracing::info!(queue = routing_keys::TASK_CREATED, "worker started");

    let shutdown = CancellationToken::new();
    let watcher = {
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            infra::shutdown::shutdown_signal().await;
            shutdown.cancel();
        })
    };

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            delivery = consumer.next() => {
                let Some(delivery) = delivery else {
                    // Broker cancelled the consumer; nothing more will arrive.
                    tracing::warn!("consumer closed by broker");
                    break;
                };
                let delivery = delivery.context("consumer stream error")?;
                match handle_delivery(&delivery, &redis).await {
                    Ok(count) => tracing::info!(count, "processed task.created"),
                    Err(error) => {
                        tracing::error!(%error, "dropping unprocessable delivery");
                        // No requeue: a poison message must not loop forever.
                        messaging::reject(&delivery).await?;
                        continue;
                    }
                }
                messaging::ack(&delivery).await?;
            }
        }
    }

    watcher.abort();
    tracing::info!("worker stopped");
    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum ProcessingError {
    #[error("decode event: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("redis: {0}")]
    Redis(#[from] redis::RedisError),
}

async fn handle_delivery(delivery: &Delivery, redis: &RedisClient) -> Result<i64, ProcessingError> {
    let event: TaskCreated = serde_json::from_slice(&delivery.data)?;
    let count = redis.incr(COUNTER_KEY).await?;
    tracing::debug!(task_id = %event.task_id, title = %event.title, "task.created received");
    Ok(count)
}
