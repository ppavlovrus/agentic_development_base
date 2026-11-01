//! RabbitMQ access: publishing with confirms and consumer creation.
//!
//! Topology is intentionally minimal: one durable queue per routing key,
//! publishing through the default exchange. Declare is idempotent, so both
//! publishers and consumers call [`ensure_queue`] at startup.

use std::time::Duration;

use async_trait::async_trait;
use domain::error::PublishError;
use domain::repositories::EventPublisher;
use lapin::options::{
    BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicPublishOptions, BasicQosOptions,
    ConfirmSelectOptions, QueueDeclareOptions,
};
use lapin::types::FieldTable;
use lapin::{BasicProperties, Channel, Connection, ConnectionProperties, Consumer};
use secrecy::ExposeSecret;

use crate::config::RabbitMqSettings;

/// Connect with a short retry window so slow container starts don't kill
/// the binary.
// shortcut: retries cover startup only; a mid-run connection loss surfaces as
// an error and the process exits — let the supervisor restart it.
pub async fn connect_with_retry(settings: &RabbitMqSettings) -> Result<Connection, lapin::Error> {
    const ATTEMPTS: u8 = 10;
    const DELAY: Duration = Duration::from_millis(500);

    let url = settings.amqp_url();
    let mut last_err = None;
    for attempt in 1..=ATTEMPTS {
        match Connection::connect(url.expose_secret(), ConnectionProperties::default()).await {
            Ok(connection) => return Ok(connection),
            Err(err) => {
                tracing::warn!(attempt, %ATTEMPTS, error = %err, "rabbitmq connect failed");
                last_err = Some(err);
                tokio::time::sleep(DELAY).await;
            }
        }
    }
    Err(last_err.expect("at least one attempt was made"))
}

/// Idempotently declare the durable queue backing `routing_key`.
pub async fn ensure_queue(channel: &Channel, routing_key: &str) -> Result<(), lapin::Error> {
    channel
        .queue_declare(
            routing_key.into(),
            QueueDeclareOptions {
                durable: true,
                ..QueueDeclareOptions::default()
            },
            FieldTable::default(),
        )
        .await?;
    Ok(())
}

/// Publisher with broker confirms enabled: every publish resolves only after
/// the broker acknowledged the message.
pub struct RabbitEventPublisher {
    channel: Channel,
}

impl RabbitEventPublisher {
    pub async fn new(connection: &Connection) -> Result<Self, lapin::Error> {
        let channel = connection.create_channel().await?;
        channel
            .confirm_select(ConfirmSelectOptions::default())
            .await?;
        // Declare every event queue up front: a message routed to a missing
        // queue is silently dropped by the broker. When adding a new domain
        // event, declare its queue here as well (see skills/add-entity.md).
        ensure_queue(&channel, domain::events::routing_keys::TASK_CREATED).await?;
        Ok(Self { channel })
    }
}

#[async_trait]
impl EventPublisher for RabbitEventPublisher {
    async fn publish(&self, routing_key: &str, payload: &[u8]) -> Result<(), PublishError> {
        let confirmation = self
            .channel
            .basic_publish(
                "".into(),
                routing_key.into(),
                BasicPublishOptions::default(),
                payload,
                // delivery_mode 2 = persistent: survive broker restarts
                BasicProperties::default().with_delivery_mode(2),
            )
            .await
            .map_err(|e| PublishError(e.to_string()))?
            .await
            .map_err(|e| PublishError(e.to_string()))?;
        if !confirmation.is_ack() {
            return Err(PublishError(format!(
                "broker nacked a message for {routing_key}"
            )));
        }
        Ok(())
    }
}

/// Create a consumer with manual acknowledgements for `routing_key`'s queue.
pub async fn consumer_for(
    connection: &Connection,
    routing_key: &str,
) -> Result<Consumer, lapin::Error> {
    let channel = connection.create_channel().await?;
    ensure_queue(&channel, routing_key).await?;
    // Fair dispatch with manual acks: without a prefetch limit the broker
    // would push the whole queue to the first consumer.
    channel.basic_qos(50, BasicQosOptions::default()).await?;
    channel
        .basic_consume(
            routing_key.into(),
            format!("{routing_key}-worker").into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
}

/// Acknowledge a processed delivery.
pub async fn ack(delivery: &lapin::message::Delivery) -> Result<(), lapin::Error> {
    delivery.ack(BasicAckOptions::default()).await.map(|_| ())
}

/// Reject a broken delivery without requeueing: a poison message must not
/// loop forever.
pub async fn reject(delivery: &lapin::message::Delivery) -> Result<(), lapin::Error> {
    delivery
        .nack(BasicNackOptions {
            requeue: false,
            ..BasicNackOptions::default()
        })
        .await
        .map(|_| ())
}
