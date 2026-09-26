use std::net::SocketAddr;

use crate::{TcpTransportError, TcpTransportErrorKind};

/// Socket address used by the TCP package public API.
///
/// This intentionally stays package-owned. It is not an OXP endpoint, locator,
/// lane, or remote identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TcpSocketAddress {
    pub host: String,
    pub port: u16,
}

impl TcpSocketAddress {
    pub fn loopback_ephemeral() -> Self {
        Self {
            host: "127.0.0.1".to_owned(),
            port: 0,
        }
    }

    pub(crate) fn parse(&self) -> Result<SocketAddr, TcpTransportError> {
        format!("{}:{}", self.host, self.port)
            .parse::<SocketAddr>()
            .map_err(|error| TcpTransportError {
                kind: TcpTransportErrorKind::InvalidAddress,
                detail: error.to_string(),
            })
    }

    pub(crate) fn is_loopback_host(&self) -> bool {
        self.host
            .parse::<std::net::IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false)
    }
}

impl From<SocketAddr> for TcpSocketAddress {
    fn from(value: SocketAddr) -> Self {
        Self {
            host: value.ip().to_string(),
            port: value.port(),
        }
    }
}
