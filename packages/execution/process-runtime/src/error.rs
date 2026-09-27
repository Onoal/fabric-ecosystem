use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessExecutionErrorKind {
    NotStarted,
    Stopped,
    SpawnFailed,
    StdinWriteFailed,
    WaitFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessExecutionError {
    pub kind: ProcessExecutionErrorKind,
    pub detail: String,
}

impl ProcessExecutionError {
    pub fn new(kind: ProcessExecutionErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub fn not_started() -> Self {
        Self::new(
            ProcessExecutionErrorKind::NotStarted,
            "process runtime has not started",
        )
    }

    pub fn stopped() -> Self {
        Self::new(
            ProcessExecutionErrorKind::Stopped,
            "process runtime generation is stopped",
        )
    }

    pub fn spawn_failed(detail: impl Into<String>) -> Self {
        Self::new(ProcessExecutionErrorKind::SpawnFailed, detail)
    }

    pub fn stdin_write_failed(detail: impl Into<String>) -> Self {
        Self::new(ProcessExecutionErrorKind::StdinWriteFailed, detail)
    }

    pub fn wait_failed(detail: impl Into<String>) -> Self {
        Self::new(ProcessExecutionErrorKind::WaitFailed, detail)
    }
}

impl fmt::Display for ProcessExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl Error for ProcessExecutionError {}
