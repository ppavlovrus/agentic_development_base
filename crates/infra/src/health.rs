//! Dependency health checks for readiness probes.

use async_trait::async_trait;
use lapin::Connection;
use serde::Serialize;

use crate::cache::RedisClient;
use crate::database::DbPool;

/// Readiness contract: production code probes real dependencies, tests
/// provide stubs (the same dependency-inversion pattern as repositories).
#[async_trait]
pub trait HealthProbe: Send + Sync {
    async fn check(&self) -> HealthReport;
}

#[derive(Debug, Serialize)]
pub struct HealthReport {
    pub postgres: bool,
    pub redis: bool,
    pub rabbit: bool,
}

impl HealthReport {
    pub fn all_ok() -> Self {
        Self {
            postgres: true,
            redis: true,
            rabbit: true,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.postgres && self.redis && self.rabbit
    }
}

/// Probes PostgreSQL, Redis and RabbitMQ.
pub struct DependencyHealth {
    pool: DbPool,
    redis: RedisClient,
    rabbit: Connection,
}

impl DependencyHealth {
    pub fn new(pool: DbPool, redis: RedisClient, rabbit: Connection) -> Self {
        Self {
            pool,
            redis,
            rabbit,
        }
    }
}

#[async_trait]
impl HealthProbe for DependencyHealth {
    async fn check(&self) -> HealthReport {
        // Probes run concurrently so readiness latency is the slowest probe,
        // not their sum.
        let (postgres, redis) = tokio::join!(
            sqlx::query("SELECT 1").execute(&self.pool),
            self.redis.ping()
        );
        HealthReport {
            postgres: postgres.is_ok(),
            redis: redis.is_ok(),
            rabbit: self.rabbit.status().connected(),
        }
    }
}
