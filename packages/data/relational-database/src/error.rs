use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelationalDatabaseErrorKind {
    NotStarted,
    Stopped,
    ExecuteFailed,
    QueryFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationalDatabaseError {
    pub kind: RelationalDatabaseErrorKind,
    pub detail: String,
}

impl RelationalDatabaseError {
    pub fn new(kind: RelationalDatabaseErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub fn not_started() -> Self {
        Self::new(
            RelationalDatabaseErrorKind::NotStarted,
            "relational database has not started",
        )
    }

    pub fn stopped() -> Self {
        Self::new(
            RelationalDatabaseErrorKind::Stopped,
            "relational database generation is stopped",
        )
    }
}

impl fmt::Display for RelationalDatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl Error for RelationalDatabaseError {}
