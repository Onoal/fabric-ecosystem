use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogErrorKind {
    WriteFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogError {
    pub kind: LogErrorKind,
    pub detail: String,
}

impl LogError {
    pub fn new(kind: LogErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub fn write_failed(detail: impl Into<String>) -> Self {
        Self::new(LogErrorKind::WriteFailed, detail)
    }
}

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl Error for LogError {}
