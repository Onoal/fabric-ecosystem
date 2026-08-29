#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum IngressError {
    #[error("ingress is unavailable")]
    Unavailable,
    #[error("ingress input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("ingress binding was not found")]
    NotFound,
    #[error("ingress listener failed: {message}")]
    ListenerFailed { message: String },
    #[error("ingress dispatch failed: {message}")]
    DispatchFailed { message: String },
}

impl IngressError {
    pub(crate) fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }
}
