//! Task lifecycle state machine.

use std::fmt;

use crate::error::TaskError;

/// Task status: `todo -> doing -> done`, `done` is terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Todo,
    Doing,
    Done,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Doing => "doing",
            Self::Done => "done",
        }
    }

    /// Parse a status as stored in the database or received over the wire.
    pub fn parse(value: &str) -> Result<Self, TaskError> {
        match value {
            "todo" => Ok(Self::Todo),
            "doing" => Ok(Self::Doing),
            "done" => Ok(Self::Done),
            other => Err(TaskError::UnknownStatus(other.to_owned())),
        }
    }

    /// Whether `self -> next` is a legal transition.
    pub fn can_transition_to(self, next: Self) -> bool {
        use TaskStatus::*;
        matches!((self, next), (Todo, Doing) | (Doing, Done))
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_transitions_move_forward_only() {
        use TaskStatus::*;
        for (from, to, allowed) in [
            (Todo, Doing, true),
            (Doing, Done, true),
            (Todo, Done, false),
            (Doing, Todo, false),
            (Done, Doing, false),
            (Done, Todo, false),
            (Todo, Todo, false),
        ] {
            assert_eq!(
                from.can_transition_to(to),
                allowed,
                "{from} -> {to} should be allowed={allowed}"
            );
        }
    }

    #[test]
    fn parse_roundtrips_known_values() {
        for status in [TaskStatus::Todo, TaskStatus::Doing, TaskStatus::Done] {
            assert_eq!(
                TaskStatus::parse(status.as_str()).expect("known value"),
                status
            );
        }
        assert!(TaskStatus::parse("archived").is_err());
    }
}
