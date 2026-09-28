use fabric_package_networking_tcp::TcpSocketAddress;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpServerCompositionConfig {
    pub transport_name: &'static str,
    pub bind: TcpSocketAddress,
}

impl HttpServerCompositionConfig {
    pub fn local(transport_name: &'static str) -> Self {
        Self {
            transport_name,
            bind: TcpSocketAddress::loopback_ephemeral(),
        }
    }

    pub fn bind(transport_name: &'static str, bind: TcpSocketAddress) -> Self {
        Self {
            transport_name,
            bind,
        }
    }
}
