use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ed25519SigningErrorKind {
    SigningFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ed25519SigningError {
    pub kind: Ed25519SigningErrorKind,
    pub detail: String,
}

impl Ed25519SigningError {
    pub fn signing_failed(detail: impl Into<String>) -> Self {
        Self {
            kind: Ed25519SigningErrorKind::SigningFailed,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Ed25519SigningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl Error for Ed25519SigningError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ed25519ValueErrorKind {
    InvalidPublicKey,
    InvalidSignature,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ed25519ValueError {
    pub kind: Ed25519ValueErrorKind,
    pub detail: String,
}

impl Ed25519ValueError {
    pub fn invalid_public_key(detail: impl Into<String>) -> Self {
        Self {
            kind: Ed25519ValueErrorKind::InvalidPublicKey,
            detail: detail.into(),
        }
    }

    pub fn invalid_signature(detail: impl Into<String>) -> Self {
        Self {
            kind: Ed25519ValueErrorKind::InvalidSignature,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Ed25519ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl Error for Ed25519ValueError {}
