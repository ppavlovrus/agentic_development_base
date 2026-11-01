//! Business entities.

pub mod task;
pub mod task_status;

pub use task::{TITLE_MAX_BYTES, Task, Title};
pub use task_status::TaskStatus;
