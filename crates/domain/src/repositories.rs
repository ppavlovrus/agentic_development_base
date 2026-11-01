//! Storage and messaging contracts.
//!
//! Traits only — implementations live in `infra`, use in `app`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::entities::{Task, TaskStatus};
use crate::error::{PublishError, RepositoryError};

/// Persistence boundary for tasks.
#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn insert(&self, task: &Task) -> Result<(), RepositoryError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Task>, RepositoryError>;
    async fn list(&self, limit: u64) -> Result<Vec<Task>, RepositoryError>;
    /// Compare-and-set status transition: applies only when the stored
    /// status is still `from`; `RepositoryError::Conflict` otherwise.
    /// Guards against concurrent updates between read and write.
    async fn update_status(
        &self,
        id: Uuid,
        from: TaskStatus,
        to: TaskStatus,
        now: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;
}

/// Outbound messaging boundary: opaque payloads addressed by routing key.
#[async_trait]
pub trait EventPublisher: Send + Sync {
    async fn publish(&self, routing_key: &str, payload: &[u8]) -> Result<(), PublishError>;
}
