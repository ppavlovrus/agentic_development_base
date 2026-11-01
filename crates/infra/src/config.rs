//! Environment-driven configuration (`APP__SECTION__KEY=...`, see .env.example).

use config::Config;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub application: ApplicationSettings,
    pub database: DatabaseSettings,
    pub redis: RedisSettings,
    pub rabbitmq: RabbitMqSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplicationSettings {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseSettings {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: SecretString,
    pub database_name: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

impl DatabaseSettings {
    /// Connection URL, also usable as sqlx-cli `DATABASE_URL`.
    pub fn connection_string(&self) -> SecretString {
        SecretString::from(format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port,
            self.database_name
        ))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisSettings {
    pub host: String,
    #[serde(default = "default_redis_port")]
    pub port: u16,
}

impl RedisSettings {
    pub fn url(&self) -> String {
        format!("redis://{}:{}", self.host, self.port)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RabbitMqSettings {
    pub host: String,
    #[serde(default = "default_rabbit_port")]
    pub port: u16,
    pub username: String,
    pub password: SecretString,
}

impl RabbitMqSettings {
    /// AMQP URL with the default vhost (`/`, percent-encoded).
    pub fn amqp_url(&self) -> SecretString {
        SecretString::from(format!(
            "amqp://{}:{}@{}:{}/%2f",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port
        ))
    }
}

impl Settings {
    /// Load settings from `.env` (if present) and the environment.
    /// Missing required keys produce a descriptive error — .env.example is
    /// the source of truth for what must be set.
    pub fn load() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();
        let settings = Config::builder()
            .add_source(
                config::Environment::with_prefix("APP")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()?;
        Ok(settings)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid configuration: {0}")]
pub struct ConfigError(#[from] config::ConfigError);

fn default_host() -> String {
    "0.0.0.0".to_owned()
}

fn default_port() -> u16 {
    8080
}

fn default_max_connections() -> u32 {
    5
}

fn default_redis_port() -> u16 {
    6379
}

fn default_rabbit_port() -> u16 {
    5672
}
