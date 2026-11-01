//! Liveness and readiness probes.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::state::AppState;

/// Process is up. Never touches dependencies.
pub async fn live() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "live" })),
    )
}

/// Dependencies are reachable: PostgreSQL, Redis, RabbitMQ.
pub async fn ready(State(state): State<AppState>) -> Response {
    let report = state.health.check().await;
    let status = if report.is_ready() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(report)).into_response()
}
