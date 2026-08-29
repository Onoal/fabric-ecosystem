#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProcessError {
    #[error("process is unavailable")]
    Unavailable,
    #[error("process input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("process protocol violation: {message}")]
    ProtocolViolation { message: String },
    #[error("process integrity failure: {message}")]
    Integrity { message: String },
    #[error("process prepare failed: {message}")]
    PrepareFailed { message: String },
    #[error("process start failed: {message}")]
    StartFailed { message: String },
    #[error("process stop failed: {message}")]
    StopFailed { message: String },
}

impl ProcessError {
    pub(crate) fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }
}
