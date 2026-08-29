use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ConnectivityError {
    #[error("connectivity is unavailable")]
    Unavailable,
    #[error("{message}")]
    InvalidInput { message: String },
    #[error("connectivity reachability not found")]
    NotFound,
    #[error("connectivity reachability is not active")]
    Inactive,
    #[error("{message}")]
    Integrity { message: String },
    #[error("{message}")]
    Persistence { message: String },
    #[error("{message}")]
    ActivationFailed { message: String },
}

impl ConnectivityError {
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    pub fn integrity(message: impl Into<String>) -> Self {
        Self::Integrity {
            message: message.into(),
        }
    }

    pub fn persistence(message: impl Into<String>) -> Self {
        Self::Persistence {
            message: message.into(),
        }
    }

    pub fn activation_failed(message: impl Into<String>) -> Self {
        Self::ActivationFailed {
            message: message.into(),
        }
    }
}
