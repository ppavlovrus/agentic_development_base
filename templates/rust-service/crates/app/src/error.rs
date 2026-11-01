//! Application-layer errors.

use domain::entities::TaskStatus;
use domain::error::{PublishError, RepositoryError, TaskError};
use uuid::Uuid;

/// Everything that can fail while executing a use case.
#[derive(Debug, thiserror::Error)]
pub enum TaskServiceError {
    #[error("task not found: {0}")]
    TaskNotFound(Uuid),
    #[error(transparent)]
    InvalidTask(#[from] TaskError),
    #[error("cannot move task from {from} to {to}")]
    InvalidTransition { from: TaskStatus, to: TaskStatus },
    #[error(transparent)]
    Storage(#[from] RepositoryError),
    #[error(transparent)]
    Publishing(#[from] PublishError),
    #[error("internal error: {0}")]
    Internal(String),
}
