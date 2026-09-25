//! TCP byte-stream transport package for Fabric.
//!
//! The first transport capability is deliberately bounded: loopback TCP,
//! byte-oriented exchange, no TLS, no DNS, no OXP endpoint/session vocabulary.

use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use fabric::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TcpSocketAddress {
    pub host: String,
    pub port: u16,
}

impl TcpSocketAddress {
    pub fn loopback_ephemeral() -> Self {
        Self {
            host: "127.0.0.1".to_owned(),
            port: 0,
        }
    }

    fn parse(&self) -> Result<SocketAddr, TcpTransportError> {
        format!("{}:{}", self.host, self.port)
            .parse::<SocketAddr>()
            .map_err(|error| TcpTransportError {
                kind: TcpTransportErrorKind::InvalidAddress,
                detail: error.to_string(),
            })
    }
}

impl From<SocketAddr> for TcpSocketAddress {
    fn from(value: SocketAddr) -> Self {
        Self {
            host: value.ip().to_string(),
            port: value.port(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TcpTransportErrorKind {
    InvalidAddress,
    NotStarted,
    BindFailed,
    ConnectFailed,
    WriteFailed,
    ReadFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TcpTransportError {
    pub kind: TcpTransportErrorKind,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TcpExchange {
    pub requested_remote: TcpSocketAddress,
    pub observed_local: Option<TcpSocketAddress>,
    pub observed_peer: Option<TcpSocketAddress>,
    pub received: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TcpExchangeResult {
    Connected(TcpExchange),
    Failed(TcpTransportError),
}

impl TcpExchangeResult {
    pub fn received(&self) -> Option<&[u8]> {
        match self {
            Self::Connected(exchange) => Some(exchange.received.as_slice()),
            Self::Failed(_) => None,
        }
    }
}

pub struct TcpListenerState {
    actual_address: Mutex<Option<TcpSocketAddress>>,
    accepted_connections: Arc<AtomicUsize>,
    stop_requested: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl Default for TcpListenerState {
    fn default() -> Self {
        Self {
            actual_address: Mutex::new(None),
            accepted_connections: Arc::new(AtomicUsize::new(0)),
            stop_requested: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
        }
    }
}

fabric::resource! {
    pub TcpByteStreamTransport {
        id: "onoal.package.networking.tcp.byte-stream";

        api {
            fn requested_bind_address(&self) -> TcpSocketAddress;
            fn actual_bound_address(&self) -> Option<TcpSocketAddress>;
            fn accepted_connections(&self) -> usize;
            fn exchange(&self, payload: Vec<u8>) -> TcpExchangeResult;
            fn connect_to(&self, remote: TcpSocketAddress, payload: Vec<u8>) -> TcpExchangeResult;
        }
    }
}

fabric::adapter! {
    pub LoopbackTcpByteStream for TcpByteStreamTransport {
        id: "onoal.package.networking.tcp.loopback";

        config {
            requested_bind: TcpSocketAddress;
        }

        state {
            TcpListenerState = TcpListenerState::default();
        }

        runtime {
            fn requested_bind_address(&self) -> TcpSocketAddress {
                self.config.requested_bind.clone()
            }

            fn actual_bound_address(&self) -> Option<TcpSocketAddress> {
                self.state
                    .get()
                    .actual_address
                    .lock()
                    .expect("tcp listener address")
                    .clone()
            }

            fn accepted_connections(&self) -> usize {
                self.state
                    .get()
                    .accepted_connections
                    .load(Ordering::SeqCst)
            }

            fn exchange(&self, payload: Vec<u8>) -> TcpExchangeResult {
                match self.actual_bound_address() {
                    Some(address) => self.connect_to(address, payload),
                    None => TcpExchangeResult::Failed(TcpTransportError {
                        kind: TcpTransportErrorKind::NotStarted,
                        detail: "tcp listener is not started".to_owned(),
                    }),
                }
            }

            fn connect_to(&self, remote: TcpSocketAddress, payload: Vec<u8>) -> TcpExchangeResult {
                connect_write_read(remote, payload)
            }
        }

        lifecycle {
            start {
                let requested = self
                    .config
                    .requested_bind
                    .parse()
                    .map_err(|error| fabric::core::ModuleError::new(error.detail))?;
                let listener = TcpListener::bind(requested)
                    .map_err(|error| fabric::core::ModuleError::new(format!("tcp bind failed: {error}")))?;
                let actual = listener
                    .local_addr()
                    .map(TcpSocketAddress::from)
                    .map_err(|error| fabric::core::ModuleError::new(format!("tcp local_addr failed: {error}")))?;

                self.state.get().stop_requested.store(false, Ordering::SeqCst);
                self.state
                    .get()
                    .accepted_connections
                    .store(0, Ordering::SeqCst);
                *self
                    .state
                    .get()
                    .actual_address
                    .lock()
                    .expect("tcp listener address") = Some(actual.clone());

                let stop_requested = Arc::clone(&self.state.get().stop_requested);
                let accepted_connections = Arc::clone(&self.state.get().accepted_connections);
                let handle = thread::spawn(move || {
                    for accepted in listener.incoming() {
                        if stop_requested.load(Ordering::SeqCst) {
                            break;
                        }
                        match accepted {
                            Ok(mut stream) => {
                                accepted_connections.fetch_add(1, Ordering::SeqCst);
                                let mut bytes = Vec::new();
                                if stream.read_to_end(&mut bytes).is_ok() {
                                    let _ = stream.write_all(&bytes);
                                    let _ = stream.flush();
                                }
                            }
                            Err(_) => {
                                if stop_requested.load(Ordering::SeqCst) {
                                    break;
                                }
                            }
                        }
                    }
                });
                *self.state.get().handle.lock().expect("tcp listener handle") = Some(handle);
                Ok(())
            }

            stop {
                self.state.get().stop_requested.store(true, Ordering::SeqCst);
                let address = self.actual_bound_address();
                if let Some(address) = &address {
                    if let Ok(socket) = address.parse() {
                        let _ = TcpStream::connect(socket);
                    }
                }
                if let Some(handle) = self
                    .state
                    .get()
                    .handle
                    .lock()
                    .expect("tcp listener handle")
                    .take()
                {
                    let _ = handle.join();
                }
                *self
                    .state
                    .get()
                    .actual_address
                    .lock()
                    .expect("tcp listener address") = None;
                Ok(())
            }
        }
    }
}

fn connect_write_read(remote: TcpSocketAddress, payload: Vec<u8>) -> TcpExchangeResult {
    let socket = match remote.parse() {
        Ok(socket) => socket,
        Err(error) => return TcpExchangeResult::Failed(error),
    };
    let mut stream = match TcpStream::connect(socket) {
        Ok(stream) => stream,
        Err(error) => {
            return TcpExchangeResult::Failed(TcpTransportError {
                kind: TcpTransportErrorKind::ConnectFailed,
                detail: error.to_string(),
            });
        }
    };
    let observed_local = stream.local_addr().ok().map(TcpSocketAddress::from);
    let observed_peer = stream.peer_addr().ok().map(TcpSocketAddress::from);
    if let Err(error) = stream.write_all(&payload) {
        return TcpExchangeResult::Failed(TcpTransportError {
            kind: TcpTransportErrorKind::WriteFailed,
            detail: error.to_string(),
        });
    }
    let _ = stream.shutdown(Shutdown::Write);
    let mut received = Vec::new();
    if let Err(error) = stream.read_to_end(&mut received) {
        return TcpExchangeResult::Failed(TcpTransportError {
            kind: TcpTransportErrorKind::ReadFailed,
            detail: error.to_string(),
        });
    }
    TcpExchangeResult::Connected(TcpExchange {
        requested_remote: remote,
        observed_local,
        observed_peer,
        received,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TcpProbeObservation {
    pub requested: TcpSocketAddress,
    pub actual: Option<TcpSocketAddress>,
    pub accepted_connections: usize,
}

fabric::component! {
    pub TcpTransportProbe {
        id: "onoal.package.networking.tcp.probe";

        relations {
            requires {
                transport: TcpByteStreamTransport;
            }
        }

        api {
            fn observe_transport(&self) -> TcpProbeObservation;
            fn round_trip(&self, payload: Vec<u8>) -> TcpExchangeResult;
            fn connect_to(&self, remote: TcpSocketAddress, payload: Vec<u8>) -> TcpExchangeResult;
        }

        runtime {
            fn observe_transport(&self) -> TcpProbeObservation {
                TcpProbeObservation {
                    requested: self.relations().transport.requested_bind_address(),
                    actual: self.relations().transport.actual_bound_address(),
                    accepted_connections: self.relations().transport.accepted_connections(),
                }
            }

            fn round_trip(&self, payload: Vec<u8>) -> TcpExchangeResult {
                self.relations().transport.exchange(payload)
            }

            fn connect_to(&self, remote: TcpSocketAddress, payload: Vec<u8>) -> TcpExchangeResult {
                self.relations().transport.connect_to(remote, payload)
            }
        }
    }
}

fabric::component! {
    pub DualTcpTransportProbe {
        id: "onoal.package.networking.tcp.dual-probe";

        relations {
            requires {
                api: TcpByteStreamTransport;
                control: TcpByteStreamTransport;
            }
        }

        api {
            fn observe_both(&self) -> (TcpProbeObservation, TcpProbeObservation);
            fn round_trip_both(&self) -> (TcpExchangeResult, TcpExchangeResult);
        }

        runtime {
            fn observe_both(&self) -> (TcpProbeObservation, TcpProbeObservation) {
                (
                    TcpProbeObservation {
                        requested: self.relations().api.requested_bind_address(),
                        actual: self.relations().api.actual_bound_address(),
                        accepted_connections: self.relations().api.accepted_connections(),
                    },
                    TcpProbeObservation {
                        requested: self.relations().control.requested_bind_address(),
                        actual: self.relations().control.actual_bound_address(),
                        accepted_connections: self.relations().control.accepted_connections(),
                    },
                )
            }

            fn round_trip_both(&self) -> (TcpExchangeResult, TcpExchangeResult) {
                (
                    self.relations().api.exchange(b"api".to_vec()),
                    self.relations().control.exchange(b"control".to_vec()),
                )
            }
        }
    }
}

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

pub fn tcp_transport_probe(name: &'static str) -> impl IntoFabricContribution {
    let transport = TcpByteStreamTransport::select(name).expect("valid tcp transport name");
    let component = TcpTransportProbe::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("transport").expect("role"),
            fabric::authoring::Requires::<TcpByteStreamTransport>::provisional(),
        ),
        &transport,
    );
    FabricContribution::new().component(component)
}

pub fn dual_tcp_transport_probe(
    api_name: &'static str,
    control_name: &'static str,
) -> impl IntoFabricContribution {
    let api = TcpByteStreamTransport::select(api_name).expect("valid api transport name");
    let control =
        TcpByteStreamTransport::select(control_name).expect("valid control transport name");
    let component = DualTcpTransportProbe::define()
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    fn composition(id: &str) -> Composition {
        Fabric::new(id)
            .expect("fabric")
            .with(loopback_tcp_transport("api"))
            .with(tcp_transport_probe("api"))
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

    fn expect_bytes(result: TcpExchangeResult, bytes: &[u8]) -> TcpExchange {
        match result {
            TcpExchangeResult::Connected(exchange) => {
                assert_eq!(exchange.received, bytes);
                exchange
            }
            TcpExchangeResult::Failed(error) => panic!("tcp exchange failed: {error:?}"),
        }
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
    fn ephemeral_bind_real_connection_bytes_and_two_clients_work() {
        let composition = composition("onoal.package.test.network.round-trip");
        let instance = started_instance(
            &composition,
            "onoal.package.test.network.round-trip.instance",
        );
        let probe = activate::<TcpTransportProbe>(&instance);
        let before = block_on(probe.observe_transport()).expect("observe");
        assert_eq!(before.requested.port, 0);
        let actual = before.actual.expect("actual bound address");
        assert_ne!(actual.port, 0);

        let first = expect_bytes(
            block_on(probe.round_trip(b"first-client".to_vec())).expect("first"),
            b"first-client",
        );
        assert_eq!(first.observed_peer, Some(actual.clone()));
        let second = expect_bytes(
            block_on(probe.round_trip(b"second-client".to_vec())).expect("second"),
            b"second-client",
        );
        assert_eq!(second.observed_peer, Some(actual));
        assert_eq!(
            block_on(probe.observe_transport())
                .expect("observe after")
                .accepted_connections,
            2
        );
    }

    #[test]
    fn multiple_occurrences_bind_independently_and_do_not_leak() {
        let composition = Fabric::new("onoal.package.test.network.occurrences")
            .expect("fabric")
            .with(loopback_tcp_transport("api"))
            .with(loopback_tcp_transport("control"))
            .with(dual_tcp_transport_probe("api", "control"))
            .build()
            .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.package.test.network.occurrences.instance",
        );
        let probe = activate::<DualTcpTransportProbe>(&instance);
        let (api, control) = block_on(probe.observe_both()).expect("observe both");
        let api_address = api.actual.expect("api address");
        let control_address = control.actual.expect("control address");
        assert_ne!(api_address, control_address);

        let (api_result, control_result) =
            block_on(probe.round_trip_both()).expect("round trip both");
        expect_bytes(api_result, b"api");
        expect_bytes(control_result, b"control");

        let (api_after, control_after) = block_on(probe.observe_both()).expect("observe after");
        assert_eq!(api_after.accepted_connections, 1);
        assert_eq!(control_after.accepted_connections, 1);
    }

    #[test]
    fn multi_instance_and_fresh_generation_own_distinct_live_sockets() {
        let composition = composition("onoal.package.test.network.instances");
        let mut first = started_instance(&composition, "onoal.package.test.network.instances.same");
        let second = started_instance(&composition, "onoal.package.test.network.instances.other");

        let first_probe = activate::<TcpTransportProbe>(&first);
        let second_probe = activate::<TcpTransportProbe>(&second);
        let first_address = block_on(first_probe.observe_transport())
            .expect("first observe")
            .actual
            .expect("first address");
        let second_address = block_on(second_probe.observe_transport())
            .expect("second observe")
            .actual
            .expect("second address");
        assert_ne!(first_address, second_address);
        expect_bytes(
            block_on(first_probe.round_trip(b"first".to_vec())).expect("first"),
            b"first",
        );
        assert_eq!(
            block_on(second_probe.observe_transport())
                .expect("second still untouched")
                .accepted_connections,
            0
        );

        first.stop().expect("stop first");
        let fresh = started_instance(&composition, "onoal.package.test.network.instances.same");
        let fresh_probe = activate::<TcpTransportProbe>(&fresh);
        let fresh_observation = block_on(fresh_probe.observe_transport()).expect("fresh observe");
        assert_eq!(fresh_observation.accepted_connections, 0);
        assert!(fresh_observation.actual.is_some());
    }

    #[test]
    fn connection_refused_peer_disconnect_and_stopped_instance_are_bounded() {
        let composition = composition("onoal.package.test.network.failures");
        let mut instance =
            started_instance(&composition, "onoal.package.test.network.failures.instance");
        {
            let probe = activate::<TcpTransportProbe>(&instance);

            let failed = block_on(probe.connect_to(
                TcpSocketAddress {
                    host: "127.0.0.1".to_owned(),
                    port: 9,
                },
                b"refused".to_vec(),
            ))
            .expect("connect refused result");
            assert!(matches!(
                failed,
                TcpExchangeResult::Failed(TcpTransportError {
                    kind: TcpTransportErrorKind::ConnectFailed,
                    ..
                })
            ));

            let address = block_on(probe.observe_transport())
                .expect("observe")
                .actual
                .expect("address");
            let socket = address.parse().expect("socket addr");
            let stream = TcpStream::connect(socket).expect("peer connect");
            drop(stream);
            expect_bytes(
                block_on(probe.round_trip(b"after-disconnect".to_vec())).expect("after disconnect"),
                b"after-disconnect",
            );
            assert_eq!(
                block_on(probe.observe_transport())
                    .expect("observe peer disconnect")
                    .accepted_connections,
                2
            );
        }

        instance.stop().expect("stop");
        let stopped_probe = instance.component::<TcpTransportProbe>().expect("probe");
        assert!(block_on(stopped_probe.round_trip(b"after-stop".to_vec())).is_err());
    }
}
