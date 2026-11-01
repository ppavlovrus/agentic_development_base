//! Task use cases: validation, persistence, event publishing.

use std::sync::Arc;

use chrono::Utc;
use domain::entities::{Task, TaskStatus, Title};
use domain::events::{TaskCreated, routing_keys};
use domain::repositories::{EventPublisher, TaskRepository};
use uuid::Uuid;

use crate::error::TaskServiceError;

pub struct TaskService {
    repository: Arc<dyn TaskRepository>,
    publisher: Arc<dyn EventPublisher>,
}

impl TaskService {
    pub fn new(repository: Arc<dyn TaskRepository>, publisher: Arc<dyn EventPublisher>) -> Self {
        Self {
            repository,
            publisher,
        }
    }

    #[tracing::instrument(skip(self, raw_title), fields(task.title = raw_title))]
    pub async fn create_task(&self, raw_title: String) -> Result<Task, TaskServiceError> {
        let title = Title::new(raw_title)?;
        let now = Utc::now();
        let task = Task {
            id: Uuid::now_v7(),
            title,
            status: TaskStatus::Todo,
            created_at: now,
            updated_at: now,
        };
        self.repository.insert(&task).await?;

        // shortcut: publish-after-insert without the outbox pattern; a broker
        // outage can lose the event — adopt an outbox when events must be reliable.
        let event = TaskCreated {
            task_id: task.id,
            title: task.title.as_str().to_owned(),
            occurred_at: now,
        };
        let payload =
            serde_json::to_vec(&event).map_err(|e| TaskServiceError::Internal(e.to_string()))?;
        self.publisher
            .publish(routing_keys::TASK_CREATED, &payload)
            .await?;

        Ok(task)
    }

    pub async fn get_task(&self, id: Uuid) -> Result<Task, TaskServiceError> {
        self.repository
            .find_by_id(id)
            .await?
            .ok_or(TaskServiceError::TaskNotFound(id))
    }

    pub async fn list_tasks(&self, limit: u64) -> Result<Vec<Task>, TaskServiceError> {
        const MAX_LIMIT: u64 = 200;
        let limit = limit.clamp(1, MAX_LIMIT);
        Ok(self.repository.list(limit).await?)
    }

    #[tracing::instrument(skip(self))]
    pub async fn update_task_status(
        &self,
        id: Uuid,
        next: TaskStatus,
    ) -> Result<Task, TaskServiceError> {
        let task = self.get_task(id).await?;
        if !task.status.can_transition_to(next) {
            return Err(TaskServiceError::InvalidTransition {
                from: task.status,
                to: next,
            });
        }
        let now = Utc::now();
        // Compare-and-set: fails with Conflict if another writer moved the
        // task between our read and this write.
        self.repository
            .update_status(id, task.status, next, now)
            .await?;
        Ok(Task {
            status: next,
            updated_at: now,
            ..task
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use domain::error::TaskError;

    use super::*;
    use crate::fakes::{CapturingPublisher, InMemoryTaskRepository};

    fn service() -> (
        TaskService,
        Arc<InMemoryTaskRepository>,
        Arc<CapturingPublisher>,
    ) {
        let repo = Arc::new(InMemoryTaskRepository::new());
        let publisher = Arc::new(CapturingPublisher::default());
        (
            TaskService::new(repo.clone(), publisher.clone()),
            repo,
            publisher,
        )
    }

    #[tokio::test]
    async fn create_persists_and_publishes() {
        let (service, repo, publisher) = service();

        let task = service
            .create_task("  write docs  ".to_owned())
            .await
            .expect("create");

        assert_eq!(task.title.as_str(), "write docs");
        assert_eq!(task.status, TaskStatus::Todo);
        assert_eq!(repo.snapshot().len(), 1);
        let (routing_key, payload) = &publisher.published()[0];
        assert_eq!(routing_key, "task.created");
        let event: TaskCreated = serde_json::from_slice(payload).expect("valid event json");
        assert_eq!(event.task_id, task.id);
    }

    #[tokio::test]
    async fn create_rejects_empty_title() {
        let (service, _repo, _publisher) = service();
        let err = service
            .create_task("   ".to_owned())
            .await
            .expect_err("invalid");
        assert!(matches!(
            err,
            TaskServiceError::InvalidTask(TaskError::EmptyTitle)
        ));
    }

    #[tokio::test]
    async fn get_missing_task_returns_not_found() {
        let (service, _repo, _publisher) = service();
        let err = service.get_task(Uuid::now_v7()).await.expect_err("missing");
        assert!(matches!(err, TaskServiceError::TaskNotFound(_)));
    }

    #[tokio::test]
    async fn status_moves_forward_only() {
        let (service, _repo, _publisher) = service();
        let task = service
            .create_task("task".to_owned())
            .await
            .expect("create");

        let task = service
            .update_task_status(task.id, TaskStatus::Doing)
            .await
            .expect("todo -> doing");
        assert_eq!(task.status, TaskStatus::Doing);

        let err = service
            .update_task_status(task.id, TaskStatus::Todo)
            .await
            .expect_err("going back is illegal");
        assert!(matches!(err, TaskServiceError::InvalidTransition { .. }));
    }
}
