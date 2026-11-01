//! API unit tests: router + handlers over in-memory fakes, no I/O.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use app::fakes::{CapturingPublisher, InMemoryTaskRepository};
use app::services::TaskService;
use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use infra::health::{HealthProbe, HealthReport};
use serde_json::Value;
use tower::ServiceExt; // for `oneshot`

use server::build_router;
use server::state::AppState;

struct StubHealth;

#[async_trait]
impl HealthProbe for StubHealth {
    async fn check(&self) -> HealthReport {
        HealthReport::all_ok()
    }
}

fn test_router() -> axum::Router {
    let repository = Arc::new(InMemoryTaskRepository::new());
    let publisher = Arc::new(CapturingPublisher::default());
    let state = AppState {
        tasks: Arc::new(TaskService::new(repository, publisher)),
        health: Arc::new(StubHealth),
    };
    build_router(state)
}

async fn respond(router: &axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("responses are JSON")
    };
    (status, body)
}

async fn post_task(router: &axum::Router, title: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri("/api/tasks")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "title": title }).to_string(),
        ))
        .unwrap();
    respond(router, request).await
}

async fn patch_status(router: &axum::Router, id: &str, status: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("PATCH")
        .uri(format!("/api/tasks/{id}/status"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "status": status }).to_string(),
        ))
        .unwrap();
    respond(router, request).await
}

#[tokio::test]
async fn create_task_returns_201_with_task() {
    let router = test_router();

    let (status, body) = post_task(&router, "write the template").await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["title"], "write the template");
    assert_eq!(body["status"], "todo");
    assert!(body["id"].is_string(), "id must be a UUID string: {body}");
}

#[tokio::test]
async fn create_task_rejects_empty_title_with_422() {
    let router = test_router();

    let (status, body) = post_task(&router, "   ").await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "title must not be empty");
}

#[tokio::test]
async fn get_missing_task_returns_404() {
    let router = test_router();

    let (status, _body) = respond(
        &router,
        Request::builder()
            .uri("/api/tasks/00000000-0000-0000-0000-000000000000")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn status_transitions_enforced_over_http() {
    let router = test_router();
    let (_, created) = post_task(&router, "task one").await;
    let id = created["id"].as_str().unwrap();

    // todo -> doing is legal.
    let (status, body) = patch_status(&router, id, "doing").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "doing");

    // doing -> todo is not.
    let (status, _) = patch_status(&router, id, "todo").await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Unknown status values are a validation error.
    let (status, _) = patch_status(&router, id, "archived").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn list_returns_created_tasks() {
    let router = test_router();
    post_task(&router, "task one").await;
    post_task(&router, "task two").await;

    let (status, body) = respond(
        &router,
        Request::builder()
            .uri("/api/tasks?limit=10")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn health_live_never_touches_dependencies() {
    let router = test_router();

    let (status, body) = respond(
        &router,
        Request::builder()
            .uri("/health/live")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "live");
}
