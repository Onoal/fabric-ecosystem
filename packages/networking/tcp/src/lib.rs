//! TCP byte-stream transport package for Fabric.
//!
//! This package owns one bounded TCP capability:
//!
//! - `TcpByteStreamTransport`, a named Fabric Resource for TCP byte-stream
//!   listener/connector behavior;
//! - `TcpConnection`, a live runtime value for one accepted or connected byte
//!   stream;
//! - `LoopbackTcpByteStream`, the first real OS TCP realization, constrained to
//!   loopback bind addresses;
//! - `TcpTransportInspector`, reusable runtime inspection behavior for facts
//!   such as the actual bound address of an ephemeral listener.
//!
//! TCP remains byte-oriented. It does not define TLS, DNS, HTTP, OXP endpoint
//! vocabulary, application framing, routing, proxying, or connection pooling.

mod address;
mod authoring;
mod connection;
mod error;
mod inspection;
mod loopback;
mod transport;

pub use address::TcpSocketAddress;
pub use authoring::{loopback_tcp_transport, loopback_tcp_transport_on, tcp_transport_inspector};
pub use connection::TcpConnection;
pub use error::{TcpTransportError, TcpTransportErrorKind};
pub use inspection::{
    TcpTransportInspection, TcpTransportInspector, TcpTransportInspectorInstanceApi,
};
pub use loopback::{LoopbackTcpByteStream, LoopbackTcpByteStreamConfig};
pub use transport::{TcpAcceptResult, TcpByteStreamTransport, TcpConnectResult};
