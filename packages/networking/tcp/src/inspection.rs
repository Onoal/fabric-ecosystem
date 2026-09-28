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
                    requested: resolve_resource(
                        self.relations().transport.requested_bind_address(),
                    ),
                    actual: resolve_resource(self.relations().transport.actual_bound_address()),
                    accepted_connections: resolve_resource(
                        self.relations().transport.accepted_connections(),
                    ),
                }
            }
        }
    }
}

fn resolve_resource<T>(mut future: fabric::resource::ResourceFuture<'_, T>) -> T {
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => {
            panic!("local tcp resource operation unexpectedly yielded")
        }
    }
}
