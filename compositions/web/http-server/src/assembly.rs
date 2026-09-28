use fabric::prelude::*;

use crate::HttpServerCompositionConfig;

pub fn http_server_stack(config: HttpServerCompositionConfig) -> impl IntoFabricContribution {
    FabricContribution::new()
        .with(fabric_package_networking_tcp::loopback_tcp_transport_on(
            config.transport_name,
            config.bind,
        ))
        .with(fabric_package_networking_tcp::tcp_transport_inspector(
            config.transport_name,
        ))
        .with(fabric_package_networking_http::http_server(
            config.transport_name,
        ))
}

pub fn local_http_server(transport_name: &'static str) -> impl IntoFabricContribution {
    http_server_stack(HttpServerCompositionConfig::local(transport_name))
}
