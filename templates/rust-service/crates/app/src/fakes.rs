//! Hand-written test doubles for the domain traits, reusable from any
//! crate's tests (`app::fakes`). No mocking framework needed.

use std::sync::{Mutex, RwLock};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::entities::{Task, TaskStatus};
use domain::error::{PublishError, RepositoryError};
use domain::repositories::{EventPublisher, TaskRepository};
use uuid::Uuid;

/// In-memory [`TaskRepository`]: append-only list, statuses updated in place.
pub struct InMemoryTaskRepository {
    tasks: RwLock<Vec<Task>>,
}

impl InMemoryTaskRepository {
    pub fn new() -> Self {
        Self {
            tasks: RwLock::new(Vec::new()),
        }
    }

    /// Point-in-time copy of stored tasks.
    pub fn snapshot(&self) -> Vec<Task> {
        self.tasks.read().expect("task lock poisoned").clone()
    }
}

impl Default for InMemoryTaskRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskRepository for InMemoryTaskRepository {
    async fn insert(&self, task: &Task) -> Result<(), RepositoryError> {
        let mut tasks = self.tasks.write().expect("task lock poisoned");
        if tasks.iter().any(|t| t.id == task.id) {
            return Err(RepositoryError::Conflict(format!(
                "task {} already exists",
                task.id
            )));
        }
        tasks.push(task.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Task>, RepositoryError> {
        Ok(self
            .tasks
            .read()
            .expect("task lock poisoned")
            .iter()
            .find(|t| t.id == id)
            .cloned())
    }

    async fn list(&self, limit: u64) -> Result<Vec<Task>, RepositoryError> {
        let tasks = self.tasks.read().expect("task lock poisoned");
        Ok(tasks
            .iter()
            .take(limit.min(usize::MAX as u64) as usize)
            .cloned()
            .collect())
    }

    async fn update_status(
        &self,
        id: Uuid,
        from: TaskStatus,
        to: TaskStatus,
        now: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let mut tasks = self.tasks.write().expect("task lock poisoned");
        let Some(task) = tasks.iter_mut().find(|t| t.id == id) else {
            return Err(RepositoryError::NotFound(format!("task {id}")));
        };
        if task.status != from {
            return Err(RepositoryError::Conflict(format!(
                "task {id} changed concurrently"
            )));
        }
        task.status = to;
        task.updated_at = now;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Building blocks for a stored task without going through the service.
    fn stored_task(status: TaskStatus) -> Task {
        Task {
            id: Uuid::now_v7(),
            title: domain::entities::Title::new("fake".to_owned()).expect("valid title"),
            status,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn update_status_is_compare_and_set() {
        let repo = InMemoryTaskRepository::new();
        let task = stored_task(TaskStatus::Todo);
        repo.insert(&task).await.expect("insert");

        repo.update_status(task.id, TaskStatus::Todo, TaskStatus::Doing, Utc::now())
            .await
            .expect("fresh status matches");

        // Stale `from` must conflict instead of overwriting.
        let err = repo
            .update_status(task.id, TaskStatus::Todo, TaskStatus::Doing, Utc::now())
            .await
            .expect_err("stale from");
        assert!(matches!(err, RepositoryError::Conflict(_)));
    }
}

/// [`EventPublisher`] that records everything published for assertions.
#[derive(Default)]
pub struct CapturingPublisher {
    published: Mutex<Vec<(String, Vec<u8>)>>,
}

impl CapturingPublisher {
    pub fn published(&self) -> Vec<(String, Vec<u8>)> {
        self.published
            .lock()
            .expect("publisher lock poisoned")
            .clone()
    }
}

#[async_trait]
impl EventPublisher for CapturingPublisher {
    async fn publish(&self, routing_key: &str, payload: &[u8]) -> Result<(), PublishError> {
        self.published
            .lock()
            .expect("publisher lock poisoned")
            .push((routing_key.to_owned(), payload.to_vec()));
        Ok(())
    }
}
