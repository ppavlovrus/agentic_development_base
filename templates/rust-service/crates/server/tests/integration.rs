//! End-to-end test against real PostgreSQL, Redis and RabbitMQ in throwaway
//! containers. Requires docker; run via `just test-int`. Marked `#[ignore]`
//! so plain `cargo test` stays offline and fast.
//!
//! NOTE: a testcontainers container is removed when its handle is dropped —
//! helpers therefore return the handle and tests keep it alive until the end.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use app::services::TaskService;
use domain::events::{TaskCreated, routing_keys};
use futures_util::StreamExt;
use infra::cache::RedisClient;
use infra::config::{DatabaseSettings, RabbitMqSettings, RedisSettings};
use infra::database::{self, task_repository::PostgresTaskRepository};
use infra::messaging::{self, RabbitEventPublisher};
use secrecy::{ExposeSecret, SecretString};
use sqlx::postgres::PgPoolOptions;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::rabbitmq::RabbitMq;
use testcontainers_modules::redis::Redis;
use testcontainers_modules::testcontainers::ContainerAsync;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

async fn start_postgres() -> (DatabaseSettings, ContainerAsync<Postgres>) {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let settings = DatabaseSettings {
        host: "127.0.0.1".into(),
        port,
        username: "postgres".into(),
        password: SecretString::from("postgres"),
        database_name: "postgres".into(),
        max_connections: 5,
    };
    (settings, container)
}

async fn start_redis() -> (RedisSettings, ContainerAsync<Redis>) {
    let container = Redis::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(6379).await.unwrap();
    (
        RedisSettings {
            host: "127.0.0.1".into(),
            port,
        },
        container,
    )
}

async fn start_rabbitmq() -> (RabbitMqSettings, ContainerAsync<RabbitMq>) {
    let container = RabbitMq::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5672).await.unwrap();
    let settings = RabbitMqSettings {
        host: "127.0.0.1".into(),
        port,
        username: "guest".into(),
        password: SecretString::from("guest"),
    };
    (settings, container)
}

#[tokio::test]
#[ignore = "requires docker (testcontainers)"]
async fn task_lifecycle_end_to_end() {
    // Arrange: throwaway containers and real infrastructure clients.
    let (db, _pg) = start_postgres().await;
    let (redis_settings, _redis_container) = start_redis().await;
    let (rabbit_settings, _rabbit) = start_rabbitmq().await;

    let pool = database::connect_pool(&db).await.unwrap();
    database::run_migrations(&pool).await.unwrap();
    let redis = RedisClient::connect(&redis_settings).await.unwrap();
    let rabbit = messaging::connect_with_retry(&rabbit_settings)
        .await
        .unwrap();

    let repository = Arc::new(PostgresTaskRepository::new(pool.clone()));
    let publisher = Arc::new(RabbitEventPublisher::new(&rabbit).await.unwrap());
    let service = TaskService::new(repository, publisher);

    // Act & assert: create persists and the event is consumable.
    let task = service.create_task("end-to-end".to_owned()).await.unwrap();
    let stored = service.get_task(task.id).await.unwrap();
    assert_eq!(stored.title.as_str(), "end-to-end");
    assert_eq!(stored.status.as_str(), "todo");

    let mut consumer = messaging::consumer_for(&rabbit, routing_keys::TASK_CREATED)
        .await
        .unwrap();
    let delivery = tokio::time::timeout(Duration::from_secs(15), consumer.next())
        .await
        .expect("event arrives")
        .expect("stream stays open")
        .expect("delivery is ok");
    let event: TaskCreated = serde_json::from_slice(&delivery.data).unwrap();
    assert_eq!(event.task_id, task.id);
    assert_eq!(event.title, "end-to-end");
    messaging::ack(&delivery).await.unwrap();

    // Worker-style processing: bump the redis counter.
    let count = redis.incr("stats:tasks_created").await.unwrap();
    assert_eq!(count, 1);

    // Status update goes through storage as well.
    let moved = service
        .update_task_status(task.id, domain::entities::TaskStatus::Doing)
        .await
        .unwrap();
    assert_eq!(moved.status.as_str(), "doing");

    // And a fresh read sees the persisted status.
    let reread = service.get_task(task.id).await.unwrap();
    assert_eq!(reread.status.as_str(), "doing");

    // Compare-and-set: an update from a stale status conflicts instead of
    // overwriting the current one.
    use domain::error::RepositoryError;
    use domain::repositories::TaskRepository;

    let direct: Arc<dyn TaskRepository> = Arc::new(PostgresTaskRepository::new(pool.clone()));
    let err = direct
        .update_status(
            task.id,
            domain::entities::TaskStatus::Todo,
            domain::entities::TaskStatus::Doing,
            chrono::Utc::now(),
        )
        .await
        .expect_err("stale from must conflict");
    assert!(matches!(err, RepositoryError::Conflict(_)));
}

#[tokio::test]
#[ignore = "requires docker (testcontainers)"]
async fn pool_connects_via_url_builder() {
    // Guards the connection-string assembly used everywhere else.
    let (db, _pg) = start_postgres().await;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(db.connection_string().expose_secret())
        .await
        .unwrap();
    let row: (i32,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await.unwrap();
    assert_eq!(row.0, 1);
}
