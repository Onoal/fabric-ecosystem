#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ServerError {
    #[error("server is unavailable")]
    Unavailable,
    #[error("server input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("server protocol violation: {message}")]
    ProtocolViolation { message: String },
    #[error("server prepare failed: {message}")]
    PrepareFailed { message: String },
    #[error("server start failed: {message}")]
    StartFailed { message: String },
    #[error("server http dispatch failed: {message}")]
    DispatchFailed { message: String },
    #[error("server stop failed: {message}")]
    StopFailed { message: String },
}

impl ServerError {
    pub(crate) fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }
}
