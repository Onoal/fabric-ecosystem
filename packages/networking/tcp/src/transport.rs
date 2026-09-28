use crate::{TcpConnection, TcpSocketAddress, TcpTransportError};

#[derive(Clone, Debug)]
pub enum TcpConnectResult {
    Connected(TcpConnection),
    Failed(TcpTransportError),
}

/// Result of accepting a queued TCP connection.
///
/// `accept()` is blocking while the realization is live and no connection is
/// queued. Stopping the owning Instance unblocks the wait and returns
/// `Stopped`.
#[derive(Clone, Debug)]
pub enum TcpAcceptResult {
    Accepted(TcpConnection),
    Stopped,
    Failed(TcpTransportError),
}

fabric::resource! {
    pub TcpByteStreamTransport {
        id: "onoal.package.networking.tcp.byte-stream";

        api {
            async fn requested_bind_address(&self) -> TcpSocketAddress;
            async fn actual_bound_address(&self) -> Option<TcpSocketAddress>;
            async fn accepted_connections(&self) -> usize;
            async fn connect(&self, remote: TcpSocketAddress) -> TcpConnectResult;
            async fn accept(&self) -> TcpAcceptResult;
        }
    }
}
