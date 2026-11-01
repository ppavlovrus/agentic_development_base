//! PostgreSQL implementation of [`TaskRepository`].
//!
//! Queries use compile-time checked macros (`sqlx::query!`/`query_as!`).
//! After changing SQL here or the schema: run `just sqlx-prepare`
//! (`cargo sqlx prepare --workspace`) and commit the `.sqlx/` cache.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::entities::{Task, TaskStatus, Title};
use domain::error::RepositoryError;
use domain::repositories::TaskRepository;
use sqlx::PgPool;
use uuid::Uuid;

pub struct PostgresTaskRepository {
    pool: PgPool,
}

impl PostgresTaskRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Raw row shape: `status` stays a string so `domain` needs no sqlx
/// dependency; conversion validates it on the way in.
struct TaskRow {
    id: Uuid,
    title: String,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<TaskRow> for Task {
    type Error = RepositoryError;

    fn try_from(row: TaskRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            title: Title::new(row.title).map_err(|e| RepositoryError::Corrupted(e.to_string()))?,
            status: TaskStatus::parse(&row.status)
                .map_err(|e| RepositoryError::Corrupted(e.to_string()))?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[async_trait]
impl TaskRepository for PostgresTaskRepository {
    async fn insert(&self, task: &Task) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"
            INSERT INTO tasks (id, title, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            task.id,
            task.title.as_str(),
            task.status.as_str(),
            task.created_at,
            task.updated_at,
        )
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;
        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Task>, RepositoryError> {
        let row = sqlx::query_as!(
            TaskRow,
            r#"
            SELECT id, title, status, created_at, updated_at
            FROM tasks
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;
        row.map(Task::try_from).transpose()
    }

    async fn list(&self, limit: u64) -> Result<Vec<Task>, RepositoryError> {
        // Clamped upstream by the service; the cast is therefore lossless.
        let limit = limit as i64;
        let rows = sqlx::query_as!(
            TaskRow,
            r#"
            SELECT id, title, status, created_at, updated_at
            FROM tasks
            ORDER BY created_at DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Task::try_from).collect()
    }

    async fn update_status(
        &self,
        id: Uuid,
        from: TaskStatus,
        to: TaskStatus,
        now: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query!(
            r#"
            UPDATE tasks
            SET status = $3, updated_at = $4
            WHERE id = $1 AND status = $2
            "#,
            id,
            from.as_str(),
            to.as_str(),
            now,
        )
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;
        if result.rows_affected() == 0 {
            // The service verified the task exists just before this call,
            // so reaching here means the status changed concurrently.
            return Err(RepositoryError::Conflict(format!(
                "task {id} changed concurrently"
            )));
        }
        Ok(())
    }
}

/// Translate driver errors into storage-agnostic domain errors.
fn map_db_error(err: sqlx::Error) -> RepositoryError {
    if let sqlx::Error::Database(ref db_err) = err
        && db_err.is_unique_violation()
    {
        return RepositoryError::Conflict(db_err.message().to_owned());
    }
    RepositoryError::Unavailable(err.to_string())
}
