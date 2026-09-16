use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LocalProcessDefinitionError {
    #[error("local process program is invalid")]
    InvalidProgram,
    #[error("local process argument is invalid")]
    InvalidArgument,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LocalProcessRuntimeError {
    #[error("local process has already been started")]
    AlreadyStarted,
    #[error("local process spawn failed: {message}")]
    SpawnFailed { message: String },
    #[error("local process status is unavailable: {message}")]
    StatusUnavailable { message: String },
    #[error("local process stop failed: {message}")]
    StopFailed { message: String },
}

impl LocalProcessRuntimeError {
    pub(crate) fn spawn_failed(error: impl fmt::Display) -> Self {
        Self::SpawnFailed {
            message: error.to_string(),
        }
    }

    pub(crate) fn status_unavailable(error: impl fmt::Display) -> Self {
        Self::StatusUnavailable {
            message: error.to_string(),
        }
    }
}
