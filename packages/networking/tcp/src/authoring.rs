use fabric::prelude::*;

use crate::{
    inspection::TcpTransportInspector,
    loopback::{LoopbackTcpByteStream, LoopbackTcpByteStreamConfig},
    transport::TcpByteStreamTransport,
    TcpSocketAddress,
};

pub fn loopback_tcp_transport(name: &'static str) -> impl IntoFabricContribution {
    loopback_tcp_transport_on(name, TcpSocketAddress::loopback_ephemeral())
}

pub fn loopback_tcp_transport_on(
    name: &'static str,
    requested_bind: TcpSocketAddress,
) -> impl IntoFabricContribution {
    let selected = TcpByteStreamTransport::select(name).expect("valid tcp transport name");
    FabricContribution::new().resource(
        selected
            .using(LoopbackTcpByteStream::new(LoopbackTcpByteStreamConfig {
                requested_bind,
            }))
            .expect("LoopbackTcpByteStream supports TcpByteStreamTransport"),
    )
}

pub fn tcp_transport_inspector(name: &'static str) -> impl IntoFabricContribution {
    let transport = TcpByteStreamTransport::select(name).expect("valid tcp transport name");
    let component = TcpTransportInspector::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("transport").expect("role"),
            fabric::authoring::Requires::<TcpByteStreamTransport>::provisional(),
        ),
        &transport,
    );
    FabricContribution::new().component(component)
}
