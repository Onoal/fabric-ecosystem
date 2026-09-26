use crate::{transport::TcpByteStreamTransport, TcpSocketAddress};

/// Runtime inspection facts for one TCP transport occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TcpTransportInspection {
    pub requested: TcpSocketAddress,
    pub actual: Option<TcpSocketAddress>,
    pub accepted_connections: usize,
}

fabric::component! {
    pub TcpTransportInspector {
        id: "onoal.package.networking.tcp.inspector";

        relations {
            requires {
                transport: TcpByteStreamTransport;
            }
        }

        api {
            fn inspect_transport(&self) -> TcpTransportInspection;
        }

        runtime {
            fn inspect_transport(&self) -> TcpTransportInspection {
                TcpTransportInspection {
                    requested: self.relations().transport.requested_bind_address(),
                    actual: self.relations().transport.actual_bound_address(),
                    accepted_connections: self.relations().transport.accepted_connections(),
                }
            }
        }
    }
}
