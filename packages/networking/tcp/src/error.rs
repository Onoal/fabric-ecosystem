/// Bounded package-owned TCP error kinds.
///
/// The TCP package does not expose `std::io::Error` or `std::net` errors as
/// semantic API. Implementation errors are translated into this surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TcpTransportErrorKind {
    InvalidAddress,
    NotStarted,
    Stopped,
    ConnectFailed,
    WriteFailed,
    ReadFailed,
    ShutdownFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TcpTransportError {
    pub kind: TcpTransportErrorKind,
    pub detail: String,
}

impl TcpTransportError {
    pub fn new(kind: TcpTransportErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub(crate) fn stopped() -> Self {
        Self::new(
            TcpTransportErrorKind::Stopped,
            "tcp transport generation is stopped",
        )
    }

    pub(crate) fn not_started() -> Self {
        Self::new(
            TcpTransportErrorKind::NotStarted,
            "tcp listener is not started",
        )
    }
}
