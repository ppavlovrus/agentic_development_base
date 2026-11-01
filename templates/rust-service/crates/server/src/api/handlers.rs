//! Task endpoints.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use domain::entities::{Task, TaskStatus};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Task> for TaskResponse {
    fn from(task: Task) -> Self {
        Self {
            id: task.id,
            title: task.title.as_str().to_owned(),
            status: task.status.to_string(),
            created_at: task.created_at,
            updated_at: task.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListTasksQuery {
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskStatusRequest {
    pub status: String,
}

pub async fn create_task(
    State(state): State<AppState>,
    Json(request): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), ApiError> {
    let task = state.tasks.create_task(request.title).await?;
    Ok((StatusCode::CREATED, Json(TaskResponse::from(task))))
}

pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TaskResponse>, ApiError> {
    let task = state.tasks.get_task(id).await?;
    Ok(Json(TaskResponse::from(task)))
}

pub async fn list_tasks(
    State(state): State<AppState>,
    Query(query): Query<ListTasksQuery>,
) -> Result<Json<Vec<TaskResponse>>, ApiError> {
    let tasks = state.tasks.list_tasks(query.limit.unwrap_or(50)).await?;
    Ok(Json(tasks.into_iter().map(TaskResponse::from).collect()))
}

pub async fn update_task_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateTaskStatusRequest>,
) -> Result<Json<TaskResponse>, ApiError> {
    let status =
        TaskStatus::parse(&request.status).map_err(|e| ApiError::Validation(e.to_string()))?;
    let task = state.tasks.update_task_status(id, status).await?;
    Ok(Json(TaskResponse::from(task)))
}
