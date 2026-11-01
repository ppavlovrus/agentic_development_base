//! HTTP API wiring: router, state, handlers. The binary in `main.rs` is a
//! thin composition root; everything testable lives in this library.

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod api;
pub mod state;

use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::{get, post};
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::api::handlers;
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    let api = Router::new()
        .route(
            "/tasks",
            post(handlers::create_task).get(handlers::list_tasks),
        )
        .route("/tasks/{id}", get(handlers::get_task))
        .route(
            "/tasks/{id}/status",
            axum::routing::patch(handlers::update_task_status),
        )
        .with_state(state.clone());

    Router::new()
        .nest("/api", api)
        .route("/health/live", get(api::health::live))
        .route("/health/ready", get(api::health::ready))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        // shortcut: fully permissive for template DX; restrict origins before production.
        .layer(CorsLayer::very_permissive())
}
