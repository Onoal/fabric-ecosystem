use std::fmt;

use fabric_package_networking_tcp::TcpTransportError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpErrorKind {
    Transport,
    MalformedRequest,
    RequestTooLarge,
    UnsupportedVersion,
    UnsupportedTransferEncoding,
    IncompleteRequest,
    InvalidResponse,
    WriteResponseFailed,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpError {
    pub kind: HttpErrorKind,
    pub detail: String,
}

impl HttpError {
    pub fn new(kind: HttpErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub(crate) fn transport(error: TcpTransportError) -> Self {
        Self::new(HttpErrorKind::Transport, error.detail)
    }
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for HttpError {}
