//! Shared application state injected into handlers.

use std::sync::Arc;

use app::services::TaskService;
use infra::health::HealthProbe;

#[derive(Clone)]
pub struct AppState {
    pub tasks: Arc<TaskService>,
    pub health: Arc<dyn HealthProbe>,
}
