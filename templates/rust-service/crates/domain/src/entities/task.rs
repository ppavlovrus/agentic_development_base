//! Task entity and its value objects.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::TaskError;

use super::TaskStatus;

/// Upper bound for a task title, in bytes.
pub const TITLE_MAX_BYTES: usize = 200;

/// Non-empty, trimmed and bounded task title.
/// Invalid titles are unrepresentable: build one through [`Title::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Title(String);

impl Title {
    pub fn new(raw: String) -> Result<Self, TaskError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(TaskError::EmptyTitle);
        }
        if trimmed.len() > TITLE_MAX_BYTES {
            return Err(TaskError::TitleTooLong {
                len: trimmed.len(),
                limit: TITLE_MAX_BYTES,
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A task. Invariants are enforced by constructors and service methods;
/// values coming back from storage are re-validated on mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: Uuid,
    pub title: Title,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_trims_surrounding_whitespace() {
        let title = Title::new("  write docs  ".to_owned()).expect("valid title");
        assert_eq!(title.as_str(), "write docs");
    }

    #[test]
    fn title_rejects_whitespace_only_input() {
        let err = Title::new("   ".to_owned()).expect_err("whitespace-only is invalid");
        assert_eq!(err, TaskError::EmptyTitle);
    }

    #[test]
    fn title_rejects_input_over_limit() {
        let long = "a".repeat(TITLE_MAX_BYTES + 1);
        let err = Title::new(long).expect_err("too long");
        assert_eq!(
            err,
            TaskError::TitleTooLong {
                len: TITLE_MAX_BYTES + 1,
                limit: TITLE_MAX_BYTES
            }
        );
    }
}
