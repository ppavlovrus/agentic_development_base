//! API errors mapped to HTTP responses.

use app::error::TaskServiceError;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use domain::error::RepositoryError;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("resource not found")]
    NotFound,
    #[error("{0}")]
    Validation(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("dependency unavailable")]
    ServiceUnavailable,
    #[error("internal error")]
    Internal,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()),
            ApiError::Conflict(_) => (StatusCode::CONFLICT, self.to_string()),
            ApiError::ServiceUnavailable => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            ApiError::Internal => {
                tracing::error!(error = %self, "request failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal error".to_owned(),
                )
            }
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

impl From<TaskServiceError> for ApiError {
    fn from(err: TaskServiceError) -> Self {
        match &err {
            TaskServiceError::TaskNotFound(_) => ApiError::NotFound,
            TaskServiceError::InvalidTask(e) => ApiError::Validation(e.to_string()),
            TaskServiceError::InvalidTransition { .. } => ApiError::Conflict(err.to_string()),
            // Lost compare-and-set race: the task changed concurrently.
            TaskServiceError::Storage(RepositoryError::Conflict(msg)) => {
                ApiError::Conflict(msg.clone())
            }
            TaskServiceError::Storage(RepositoryError::Unavailable(msg)) => {
                tracing::error!(error = %msg, "storage unavailable");
                ApiError::ServiceUnavailable
            }
            TaskServiceError::Storage(other) => {
                tracing::error!(error = %other, "storage failure");
                ApiError::Internal
            }
            // The task row is already committed: retrying the request would
            // duplicate it, so report a generic failure, not 5xx-with-retry-hint.
            TaskServiceError::Publishing(e) => {
                tracing::error!(error = %e, "event publishing failed after insert");
                ApiError::Internal
            }
            TaskServiceError::Internal(e) => {
                tracing::error!(error = %e, "internal service error");
                ApiError::Internal
            }
        }
    }
}
