//! Domain error types.

/// Errors caused by invalid task input or illegal state transitions.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum TaskError {
    #[error("title must not be empty")]
    EmptyTitle,
    #[error("title exceeds the limit of {limit} bytes (got {len})")]
    TitleTooLong { len: usize, limit: usize },
    #[error("unknown task status: {0}")]
    UnknownStatus(String),
}

/// Storage access errors, deliberately free of driver-specific details:
/// implementations in `infra` translate their native errors into these.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("storage unavailable: {0}")]
    Unavailable(String),
    #[error("entity not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("stored data is corrupted: {0}")]
    Corrupted(String),
}

/// Failed to hand a message to the broker.
#[derive(Debug, thiserror::Error)]
#[error("message broker unavailable: {0}")]
pub struct PublishError(pub String);
