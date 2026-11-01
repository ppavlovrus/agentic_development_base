//! Domain events shared between services and workers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Emitted when a task is created; consumed by background workers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCreated {
    pub task_id: Uuid,
    pub title: String,
    pub occurred_at: DateTime<Utc>,
}

/// Routing keys for domain events. In this template they double as queue
/// names: publishers send via the default exchange straight to the queue.
pub mod routing_keys {
    pub const TASK_CREATED: &str = "task.created";
}
