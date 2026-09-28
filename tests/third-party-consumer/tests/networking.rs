mod support;

use fabric::prelude::*;
use fabric_package_networking_tcp::{
    TcpAcceptResult, TcpByteStreamTransport, TcpConnectResult, TcpSocketAddress, TcpTransportError,
    TcpTransportInspector, TcpTransportInspectorInstanceApi,
};
use futures::executor::block_on;
use std::io::Write;
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::thread;
use support::fabric::{activate, started_instance};

fn actual_address(inspector: &BoundComponent<'_, TcpTransportInspector>) -> TcpSocketAddress {
    block_on(inspector.inspect_transport())
        .expect("inspect transport")
        .actual
        .expect("actual address")
}

fabric::component! {
    TcpConsumer {
        id: "onoal.test.third-party.networking.tcp-consumer";

        relations {
            requires {
                transport: TcpByteStreamTransport;
            }
        }

        api {
            fn accept_once(&self) -> Result<Option<Vec<u8>>, TcpTransportError>;
            fn connect_once(&self, remote: TcpSocketAddress) -> Result<(), TcpTransportError>;
        }

        runtime {
            fn accept_once(&self) -> Result<Option<Vec<u8>>, TcpTransportError> {
                match resolve_resource(self.relations().transport.accept()) {
                    TcpAcceptResult::Accepted(connection) => connection.read_to_end().map(Some),
                    TcpAcceptResult::Stopped => Ok(None),
                    TcpAcceptResult::Failed(error) => Err(error),
                }
            }

            fn connect_once(&self, remote: TcpSocketAddress) -> Result<(), TcpTransportError> {
                match resolve_resource(self.relations().transport.connect(remote)) {
                    TcpConnectResult::Connected(connection) => connection.shutdown_both(),
                    TcpConnectResult::Failed(error) => Err(error),
                }
            }
        }
    }
}

fn tcp_consumer(transport_name: &'static str) -> impl IntoFabricContribution {
    let transport = TcpByteStreamTransport::select(transport_name).expect("transport selection");
    let component = TcpConsumer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("transport").expect("role"),
            fabric::authoring::Requires::<TcpByteStreamTransport>::provisional(),
        ),
        &transport,
    );
    FabricContribution::new().component(component)
}

fn resolve_resource<T>(mut future: fabric::resource::ResourceFuture<'_, T>) -> T {
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => {
            panic!("third-party TCP resource operation unexpectedly yielded")
        }
    }
}

#[test]
fn external_consumer_uses_public_tcp_transport_and_inspector() {
    let composition = Fabric::new("onoal.test.third-party.networking.tcp")
        .expect("fabric")
        .with(fabric_package_networking_tcp::loopback_tcp_transport("api"))
        .with(fabric_package_networking_tcp::tcp_transport_inspector(
            "api",
        ))
        .with(tcp_consumer("api"))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.test.third-party.networking.tcp.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let consumer = activate::<TcpConsumer>(&instance);
    let address = actual_address(&inspector);

    let client = thread::spawn(move || {
        let socket: SocketAddr = format!("{}:{}", address.host, address.port)
            .parse()
            .expect("socket addr");
        let mut stream = TcpStream::connect(socket).expect("client connect");
        stream.write_all(b"tcp").expect("write");
        stream.shutdown(Shutdown::Write).expect("shutdown");
    });

    let bytes = block_on(consumer.accept_once())
        .expect("accept")
        .expect("tcp result")
        .expect("connection bytes");
    client.join().expect("client");

    assert_eq!(bytes, b"tcp");
    let observation = block_on(inspector.inspect_transport()).expect("inspect after accept");
    assert_eq!(observation.accepted_connections, 1);
}

#[test]
fn external_consumer_uses_public_tcp_connectivity() {
    let composition = Fabric::new("onoal.test.third-party.networking.tcp-connect")
        .expect("fabric")
        .with(fabric_package_networking_tcp::loopback_tcp_transport("api"))
        .with(fabric_package_networking_tcp::tcp_transport_inspector(
            "api",
        ))
        .with(tcp_consumer("api"))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.test.third-party.networking.tcp-connect.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let consumer = activate::<TcpConsumer>(&instance);
    let address = actual_address(&inspector);

    block_on(consumer.connect_once(address))
        .expect("connect")
        .expect("connected");
}
