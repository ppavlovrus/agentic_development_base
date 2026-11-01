//! Pure business logic: entities, repository contracts, domain events.
//!
//! This crate must stay free of infrastructure dependencies (no sqlx, redis,
//! lapin, axum). Contracts are expressed as traits; implementations live in
//! `infra`, orchestration in `app`.

#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod entities;
pub mod error;
pub mod events;
pub mod repositories;
