//! Infrastructure: PostgreSQL, Redis, RabbitMQ implementations of domain
//! traits, plus configuration, telemetry and startup helpers.
//!
//! Dependencies point inward: `infra` implements `domain` contracts and
//! knows nothing about `app` or the binaries.

pub mod cache;
pub mod config;
pub mod database;
pub mod health;
pub mod messaging;
pub mod shutdown;
pub mod telemetry;
