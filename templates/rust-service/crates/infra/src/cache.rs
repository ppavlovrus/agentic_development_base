//! Redis access.

use redis::aio::ConnectionManager;

use crate::config::RedisSettings;

/// Thin wrapper over a shared, auto-reconnecting connection.
// shortcut: single multiplexed connection; add pooling only under measured contention.
#[derive(Clone)]
pub struct RedisClient {
    conn: ConnectionManager,
}

impl RedisClient {
    pub async fn connect(settings: &RedisSettings) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(settings.url())?;
        Ok(Self {
            conn: ConnectionManager::new(client).await?,
        })
    }

    pub async fn ping(&self) -> Result<(), redis::RedisError> {
        let mut conn = self.conn.clone();
        redis::cmd("PING").query_async::<()>(&mut conn).await
    }

    /// Atomically increment `key`, returning the new value.
    pub async fn incr(&self, key: &str) -> Result<i64, redis::RedisError> {
        let mut conn = self.conn.clone();
        redis::cmd("INCR")
            .arg(key)
            .query_async::<i64>(&mut conn)
            .await
    }
}
