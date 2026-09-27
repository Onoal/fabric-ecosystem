use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CounterError {
    pub kind: CounterErrorKind,
    pub detail: String,
}

impl CounterError {
    pub fn read_failed(detail: impl Into<String>) -> Self {
        Self {
            kind: CounterErrorKind::ReadFailed,
            detail: detail.into(),
        }
    }

    pub fn increment_failed(detail: impl Into<String>) -> Self {
        Self {
            kind: CounterErrorKind::IncrementFailed,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for CounterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl Error for CounterError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CounterErrorKind {
    ReadFailed,
    IncrementFailed,
}
