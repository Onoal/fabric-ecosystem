use fabric::prelude::*;
use fabric_package_networking_tcp::TcpByteStreamTransport;

use crate::HttpServer;

pub fn http_server(transport_name: &'static str) -> impl IntoFabricContribution {
    let transport =
        TcpByteStreamTransport::select(transport_name).expect("valid TCP transport name");
    let component = HttpServer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("transport").expect("role"),
            fabric::authoring::Requires::<TcpByteStreamTransport>::provisional(),
        ),
        &transport,
    );
    FabricContribution::new().component(component)
}
