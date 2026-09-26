use fabric::prelude::*;
use fabric_package_networking_tcp::{
    loopback_tcp_transport, loopback_tcp_transport_on, tcp_transport_inspector,
    LoopbackTcpByteStream, LoopbackTcpByteStreamConfig, TcpAcceptResult, TcpByteStreamTransport,
    TcpConnectResult, TcpSocketAddress, TcpTransportError, TcpTransportErrorKind,
    TcpTransportInspection, TcpTransportInspector, TcpTransportInspectorInstanceApi,
};
use futures::executor::block_on;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::thread;

fabric::component! {
    TestEchoServer {
        id: "onoal.package.networking.tcp.test-echo-server";

        relations {
            requires {
                transport: TcpByteStreamTransport;
            }
        }

        api {
            fn serve_one_echo(&self) -> Result<usize, TcpTransportError>;
        }

        runtime {
            fn serve_one_echo(&self) -> Result<usize, TcpTransportError> {
                let connection = match self.relations().transport.accept() {
                    TcpAcceptResult::Accepted(connection) => connection,
                    TcpAcceptResult::Stopped => {
                        return Err(TcpTransportError::new(
                            TcpTransportErrorKind::Stopped,
                            "tcp transport stopped before accepting test echo connection",
                        ))
                    }
                    TcpAcceptResult::Failed(error) => return Err(error),
                };
                let bytes = connection.read_to_end()?;
                let count = bytes.len();
                connection.write_all(&bytes)?;
                connection.shutdown_both()?;
                Ok(count)
            }
        }
    }
}

fabric::component! {
    TestIncrementalReader {
        id: "onoal.package.networking.tcp.test-incremental-reader";

        relations {
            requires {
                transport: TcpByteStreamTransport;
            }
        }

        api {
            fn read_first_bytes(&self, max_bytes: usize) -> Result<Vec<u8>, TcpTransportError>;
        }

        runtime {
            fn read_first_bytes(&self, max_bytes: usize) -> Result<Vec<u8>, TcpTransportError> {
                let connection = match self.relations().transport.accept() {
                    TcpAcceptResult::Accepted(connection) => connection,
                    TcpAcceptResult::Stopped => {
                        return Err(TcpTransportError::new(
                            TcpTransportErrorKind::Stopped,
                            "tcp transport stopped before incremental read",
                        ))
                    }
                    TcpAcceptResult::Failed(error) => return Err(error),
                };
                connection.read_some(max_bytes)
            }
        }
    }
}

fabric::component! {
    TestDualTransportInspector {
        id: "onoal.package.networking.tcp.test-dual-inspector";

        relations {
            requires {
                api: TcpByteStreamTransport;
                control: TcpByteStreamTransport;
            }
        }

        api {
            fn inspect_both(&self) -> (TcpTransportInspection, TcpTransportInspection);
        }

        runtime {
            fn inspect_both(&self) -> (TcpTransportInspection, TcpTransportInspection) {
                (
                    TcpTransportInspection {
                        requested: self.relations().api.requested_bind_address(),
                        actual: self.relations().api.actual_bound_address(),
                        accepted_connections: self.relations().api.accepted_connections(),
                    },
                    TcpTransportInspection {
                        requested: self.relations().control.requested_bind_address(),
                        actual: self.relations().control.actual_bound_address(),
                        accepted_connections: self.relations().control.accepted_connections(),
                    },
                )
            }
        }
    }
}

fn bind_test_echo(transport_name: &'static str) -> impl IntoFabricContribution {
    let transport = TcpByteStreamTransport::select(transport_name).expect("transport selection");
    let component = TestEchoServer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("transport").expect("role"),
            fabric::authoring::Requires::<TcpByteStreamTransport>::provisional(),
        ),
        &transport,
    );
    FabricContribution::new().component(component)
}

fn bind_incremental_reader(transport_name: &'static str) -> impl IntoFabricContribution {
    let transport = TcpByteStreamTransport::select(transport_name).expect("transport selection");
    let component = TestIncrementalReader::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("transport").expect("role"),
            fabric::authoring::Requires::<TcpByteStreamTransport>::provisional(),
        ),
        &transport,
    );
    FabricContribution::new().component(component)
}

fn bind_dual_inspector(
    api_name: &'static str,
    control_name: &'static str,
) -> impl IntoFabricContribution {
    let api = TcpByteStreamTransport::select(api_name).expect("api selection");
    let control = TcpByteStreamTransport::select(control_name).expect("control selection");
    let component = TestDualTransportInspector::define()
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("api").expect("role"),
                fabric::authoring::Requires::<TcpByteStreamTransport>::provisional(),
            ),
            &api,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("control").expect("role"),
                fabric::authoring::Requires::<TcpByteStreamTransport>::provisional(),
            ),
            &control,
        );
    FabricContribution::new().component(component)
}

fn composition(id: &str) -> Composition {
    Fabric::new(id)
        .expect("fabric")
        .with(loopback_tcp_transport("api"))
        .with(tcp_transport_inspector("api"))
        .with(bind_test_echo("api"))
        .build()
        .expect("composition")
}

fn incremental_composition(id: &str) -> Composition {
    Fabric::new(id)
        .expect("fabric")
        .with(loopback_tcp_transport("api"))
        .with(tcp_transport_inspector("api"))
        .with(bind_incremental_reader("api"))
        .build()
        .expect("composition")
}

fn started_instance(composition: &Composition, id: &str) -> Instance {
    let mut instance = composition
        .materialize_on(id, &HostDescriptor::native())
        .expect("instance");
    instance.start().expect("start");
    instance
}

fn activate<C: fabric::authoring::ComponentDefinition>(
    instance: &Instance,
) -> BoundComponent<'_, C> {
    let component = instance.component::<C>().expect("component");
    component.reconcile().expect("reconcile");
    component
}

fn to_socket(address: &TcpSocketAddress) -> std::net::SocketAddr {
    format!("{}:{}", address.host, address.port)
        .parse()
        .expect("socket addr")
}

fn echo_client(address: TcpSocketAddress, payload: &'static [u8]) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut stream = TcpStream::connect(to_socket(&address)).expect("tcp client connect");
        stream.write_all(payload).expect("tcp client write");
        stream
            .shutdown(Shutdown::Write)
            .expect("tcp shutdown write");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("tcp client read");
        response
    })
}

#[test]
fn composition_declares_tcp_transport_without_live_bound_port() {
    let helper = Fabric::new("onoal.package.test.network.inspect.helper")
        .expect("fabric")
        .with(loopback_tcp_transport("api"))
        .build()
        .expect("composition");
    let explicit = Fabric::new("onoal.package.test.network.inspect.explicit")
        .expect("fabric")
        .resource(
            TcpByteStreamTransport::select("api")
                .expect("transport")
                .using(LoopbackTcpByteStream::new(LoopbackTcpByteStreamConfig {
                    requested_bind: TcpSocketAddress::loopback_ephemeral(),
                }))
                .expect("adapter"),
        )
        .build()
        .expect("composition");

    let helper_transport = helper.resources().next().expect("helper transport");
    let explicit_transport = explicit.resources().next().expect("explicit transport");
    assert_eq!(
        helper_transport.resource_id(),
        explicit_transport.resource_id()
    );
    assert_eq!(helper_transport.name(), explicit_transport.name());
    assert_eq!(
        helper_transport
            .realization()
            .adapter_definition_id()
            .expect("helper adapter"),
        explicit_transport
            .realization()
            .adapter_definition_id()
            .expect("explicit adapter")
    );
    assert_eq!(helper.components().count(), 0);
}

#[test]
fn component_owned_echo_uses_real_tcp_transport_for_two_connections() {
    let composition = composition("onoal.package.test.network.echo");
    let instance = started_instance(&composition, "onoal.package.test.network.echo.instance");
    let inspector = activate::<TcpTransportInspector>(&instance);
    let echo = activate::<TestEchoServer>(&instance);
    let observation = block_on(inspector.inspect_transport()).expect("inspect");
    assert_eq!(observation.requested.port, 0);
    let actual = observation.actual.expect("actual bound address");
    assert_ne!(actual.port, 0);

    let first = echo_client(actual.clone(), b"first-client");
    assert_eq!(
        block_on(echo.serve_one_echo()).expect("first echo"),
        Ok(b"first-client".len())
    );
    assert_eq!(first.join().expect("first client"), b"first-client");

    let second = echo_client(actual, b"second-client");
    assert_eq!(
        block_on(echo.serve_one_echo()).expect("second echo"),
        Ok(b"second-client".len())
    );
    assert_eq!(second.join().expect("second client"), b"second-client");
    assert_eq!(
        block_on(inspector.inspect_transport())
            .expect("inspect after")
            .accepted_connections,
        2
    );
}

#[test]
fn connect_returns_runtime_connection_value_without_echo_semantics() {
    let composition = composition("onoal.package.test.network.connect");
    let instance = started_instance(&composition, "onoal.package.test.network.connect.instance");
    let inspector = activate::<TcpTransportInspector>(&instance);
    let echo = activate::<TestEchoServer>(&instance);
    let actual = block_on(inspector.inspect_transport())
        .expect("inspect")
        .actual
        .expect("address");

    let connection = match block_on(inspector.connect(actual)).expect("connect") {
        TcpConnectResult::Connected(connection) => connection,
        TcpConnectResult::Failed(error) => panic!("connect failed: {error:?}"),
    };
    connection.write_all(b"handle-bytes").expect("write");
    connection.shutdown_write().expect("shutdown write");
    assert_eq!(
        block_on(echo.serve_one_echo()).expect("echo"),
        Ok(b"handle-bytes".len())
    );
    assert_eq!(connection.read_to_end().expect("read"), b"handle-bytes");
}

#[test]
fn incremental_read_returns_bytes_without_waiting_for_peer_eof() {
    let composition = incremental_composition("onoal.package.test.network.incremental");
    let instance = started_instance(
        &composition,
        "onoal.package.test.network.incremental.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let reader = activate::<TestIncrementalReader>(&instance);
    let actual = block_on(inspector.inspect_transport())
        .expect("inspect")
        .actual
        .expect("address");
    let (read_complete_tx, read_complete_rx) = std::sync::mpsc::channel();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(to_socket(&actual)).expect("tcp client connect");
        stream.write_all(b"partial-before-eof").expect("write");
        read_complete_rx.recv().expect("read completion");
    });

    let bytes = block_on(reader.read_first_bytes(7)).expect("read_some");
    assert_eq!(bytes, Ok(b"partial".to_vec()));
    read_complete_tx.send(()).expect("notify client");
    client.join().expect("client");
}

#[test]
fn read_to_end_observes_peer_eof() {
    let composition = composition("onoal.package.test.network.eof");
    let instance = started_instance(&composition, "onoal.package.test.network.eof.instance");
    let inspector = activate::<TcpTransportInspector>(&instance);
    let echo = activate::<TestEchoServer>(&instance);
    let actual = block_on(inspector.inspect_transport())
        .expect("inspect")
        .actual
        .expect("address");

    let client = echo_client(actual, b"eof-bytes");
    assert_eq!(
        block_on(echo.serve_one_echo()).expect("echo"),
        Ok(b"eof-bytes".len())
    );
    assert_eq!(client.join().expect("client"), b"eof-bytes");
}

#[test]
fn multiple_occurrences_bind_independently_and_do_not_leak() {
    let composition = Fabric::new("onoal.package.test.network.occurrences")
        .expect("fabric")
        .with(loopback_tcp_transport("api"))
        .with(loopback_tcp_transport("control"))
        .with(bind_dual_inspector("api", "control"))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.package.test.network.occurrences.instance",
    );
    let inspector = activate::<TestDualTransportInspector>(&instance);
    let (api, control) = block_on(inspector.inspect_both()).expect("inspect both");
    let api_address = api.actual.expect("api address");
    let control_address = control.actual.expect("control address");
    assert_ne!(api_address, control_address);
    assert_eq!(api.accepted_connections, 0);
    assert_eq!(control.accepted_connections, 0);
}

#[test]
fn multi_instance_and_fresh_generation_own_distinct_live_sockets() {
    let composition = composition("onoal.package.test.network.instances");
    let mut first = started_instance(&composition, "onoal.package.test.network.instances.same");
    let second = started_instance(&composition, "onoal.package.test.network.instances.other");

    let first_inspector = activate::<TcpTransportInspector>(&first);
    let second_inspector = activate::<TcpTransportInspector>(&second);
    let first_address = block_on(first_inspector.inspect_transport())
        .expect("first inspect")
        .actual
        .expect("first address");
    let second_address = block_on(second_inspector.inspect_transport())
        .expect("second inspect")
        .actual
        .expect("second address");
    assert_ne!(first_address, second_address);

    first.stop().expect("stop first");
    let fresh = started_instance(&composition, "onoal.package.test.network.instances.same");
    let fresh_inspector = activate::<TcpTransportInspector>(&fresh);
    let fresh_observation = block_on(fresh_inspector.inspect_transport()).expect("fresh inspect");
    assert_eq!(fresh_observation.accepted_connections, 0);
    assert!(fresh_observation.actual.is_some());
}

#[test]
fn failures_peer_disconnect_stale_connection_and_stopped_instance_are_bounded() {
    let composition = composition("onoal.package.test.network.failures");
    let mut instance =
        started_instance(&composition, "onoal.package.test.network.failures.instance");
    let stale_connection = {
        let inspector = activate::<TcpTransportInspector>(&instance);
        let echo = activate::<TestEchoServer>(&instance);

        let failed = block_on(inspector.connect(TcpSocketAddress {
            host: "127.0.0.1".to_owned(),
            port: 9,
        }))
        .expect("connect refused result");
        assert!(matches!(
            failed,
            TcpConnectResult::Failed(TcpTransportError {
                kind: TcpTransportErrorKind::ConnectFailed,
                ..
            })
        ));

        let address = block_on(inspector.inspect_transport())
            .expect("inspect")
            .actual
            .expect("address");
        let stream = TcpStream::connect(to_socket(&address)).expect("peer connect");
        drop(stream);
        assert!(matches!(
            block_on(echo.serve_one_echo()).expect("peer disconnect"),
            Err(TcpTransportError {
                kind: TcpTransportErrorKind::ReadFailed,
                ..
            }) | Ok(0)
        ));

        let connected = match block_on(inspector.connect(address)).expect("connect stale") {
            TcpConnectResult::Connected(connection) => connection,
            TcpConnectResult::Failed(error) => panic!("connect failed: {error:?}"),
        };
        let accepted_connections = block_on(inspector.inspect_transport())
            .expect("inspect peer disconnect")
            .accepted_connections;
        assert!(
            (1..=2).contains(&accepted_connections),
            "accepted connection count is scheduling-dependent after an unused client connect"
        );
        connected
    };

    instance.stop().expect("stop");
    assert!(matches!(
        stale_connection.write_all(b"after-stop"),
        Err(TcpTransportError {
            kind: TcpTransportErrorKind::Stopped,
            ..
        })
    ));
    let stopped_inspector = instance
        .component::<TcpTransportInspector>()
        .expect("inspector");
    assert!(block_on(stopped_inspector.connect(TcpSocketAddress::loopback_ephemeral())).is_err());
}

#[test]
fn stopped_instance_rejects_waiting_accept() {
    let composition = incremental_composition("onoal.package.test.network.stop-unblocks");
    let mut instance = started_instance(
        &composition,
        "onoal.package.test.network.stop-unblocks.instance",
    );
    instance.stop().expect("stop");

    let reader = instance
        .component::<TestIncrementalReader>()
        .expect("reader after stop");
    assert!(block_on(reader.read_first_bytes(1)).is_err());
}

#[test]
fn invalid_address_and_non_loopback_bind_are_bounded() {
    let invalid = TcpSocketAddress {
        host: "not an address".to_owned(),
        port: 0,
    };
    let composition = Fabric::new("onoal.package.test.network.invalid-bind")
        .expect("fabric")
        .with(loopback_tcp_transport_on("api", invalid))
        .build()
        .expect("composition");
    assert!(composition
        .materialize_on(
            "onoal.package.test.network.invalid-bind.instance",
            &HostDescriptor::native()
        )
        .expect("instance")
        .start()
        .is_err());

    let non_loopback = TcpSocketAddress {
        host: "0.0.0.0".to_owned(),
        port: 0,
    };
    let composition = Fabric::new("onoal.package.test.network.non-loopback")
        .expect("fabric")
        .with(loopback_tcp_transport_on("api", non_loopback))
        .build()
        .expect("composition");
    assert!(composition
        .materialize_on(
            "onoal.package.test.network.non-loopback.instance",
            &HostDescriptor::native()
        )
        .expect("instance")
        .start()
        .is_err());
}
