//! TCP byte-stream transport package for Fabric.
//!
//! The first transport capability is deliberately bounded: loopback TCP,
//! byte-stream mechanics, no TLS, no DNS, no OXP endpoint/session vocabulary.

use std::collections::VecDeque;
use std::fmt;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
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
    Stopped,
    ConnectFailed,
    WriteFailed,
    ReadFailed,
    ShutdownFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TcpTransportError {
    pub kind: TcpTransportErrorKind,
    pub detail: String,
}

impl TcpTransportError {
    fn stopped() -> Self {
        Self {
            kind: TcpTransportErrorKind::Stopped,
            detail: "tcp transport generation is stopped".to_owned(),
        }
    }
}

#[derive(Clone)]
pub struct TcpConnection {
    stream: Arc<Mutex<TcpStream>>,
    generation_live: Arc<AtomicBool>,
    local: Option<TcpSocketAddress>,
    peer: Option<TcpSocketAddress>,
}

impl fmt::Debug for TcpConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpConnection")
            .field("local", &self.local)
            .field("peer", &self.peer)
            .field(
                "generation_live",
                &self.generation_live.load(Ordering::SeqCst),
            )
            .finish_non_exhaustive()
    }
}

impl TcpConnection {
    fn new(stream: TcpStream, generation_live: Arc<AtomicBool>) -> Self {
        let local = stream.local_addr().ok().map(TcpSocketAddress::from);
        let peer = stream.peer_addr().ok().map(TcpSocketAddress::from);
        Self {
            stream: Arc::new(Mutex::new(stream)),
            generation_live,
            local,
            peer,
        }
    }

    pub fn local_address(&self) -> Option<TcpSocketAddress> {
        self.local.clone()
    }

    pub fn peer_address(&self) -> Option<TcpSocketAddress> {
        self.peer.clone()
    }

    pub fn write_all(&self, bytes: &[u8]) -> Result<(), TcpTransportError> {
        self.ensure_live()?;
        self.stream
            .lock()
            .expect("tcp connection")
            .write_all(bytes)
            .map_err(|error| TcpTransportError {
                kind: TcpTransportErrorKind::WriteFailed,
                detail: error.to_string(),
            })
    }

    pub fn read_to_end(&self) -> Result<Vec<u8>, TcpTransportError> {
        self.ensure_live()?;
        let mut bytes = Vec::new();
        self.stream
            .lock()
            .expect("tcp connection")
            .read_to_end(&mut bytes)
            .map_err(|error| TcpTransportError {
                kind: TcpTransportErrorKind::ReadFailed,
                detail: error.to_string(),
            })?;
        Ok(bytes)
    }

    pub fn shutdown_write(&self) -> Result<(), TcpTransportError> {
        self.ensure_live()?;
        self.stream
            .lock()
            .expect("tcp connection")
            .shutdown(Shutdown::Write)
            .map_err(|error| TcpTransportError {
                kind: TcpTransportErrorKind::ShutdownFailed,
                detail: error.to_string(),
            })
    }

    pub fn shutdown_both(&self) -> Result<(), TcpTransportError> {
        self.ensure_live()?;
        self.stream
            .lock()
            .expect("tcp connection")
            .shutdown(Shutdown::Both)
            .map_err(|error| TcpTransportError {
                kind: TcpTransportErrorKind::ShutdownFailed,
                detail: error.to_string(),
            })
    }

    fn ensure_live(&self) -> Result<(), TcpTransportError> {
        if self.generation_live.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(TcpTransportError::stopped())
        }
    }
}

#[derive(Clone, Debug)]
pub enum TcpConnectResult {
    Connected(TcpConnection),
    Failed(TcpTransportError),
}

#[derive(Clone, Debug)]
pub enum TcpAcceptResult {
    Accepted(TcpConnection),
    Stopped,
    Failed(TcpTransportError),
}

#[derive(Clone)]
struct AcceptedConnections {
    queue: Arc<(Mutex<VecDeque<TcpStream>>, Condvar)>,
}

impl Default for AcceptedConnections {
    fn default() -> Self {
        Self {
            queue: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
        }
    }
}

pub struct TcpListenerState {
    actual_address: Mutex<Option<TcpSocketAddress>>,
    accepted_connections: Arc<AtomicUsize>,
    stop_requested: Arc<AtomicBool>,
    generation_live: Arc<AtomicBool>,
    accepted: AcceptedConnections,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl Default for TcpListenerState {
    fn default() -> Self {
        Self {
            actual_address: Mutex::new(None),
            accepted_connections: Arc::new(AtomicUsize::new(0)),
            stop_requested: Arc::new(AtomicBool::new(false)),
            generation_live: Arc::new(AtomicBool::new(false)),
            accepted: AcceptedConnections::default(),
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
            fn connect(&self, remote: TcpSocketAddress) -> TcpConnectResult;
            fn accept(&self) -> TcpAcceptResult;
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

            fn connect(&self, remote: TcpSocketAddress) -> TcpConnectResult {
                if !self.state.get().generation_live.load(Ordering::SeqCst) {
                    return TcpConnectResult::Failed(TcpTransportError {
                        kind: TcpTransportErrorKind::NotStarted,
                        detail: "tcp listener is not started".to_owned(),
                    });
                }
                connect_tcp(remote, Arc::clone(&self.state.get().generation_live))
            }

            fn accept(&self) -> TcpAcceptResult {
                if !self.state.get().generation_live.load(Ordering::SeqCst) {
                    return TcpAcceptResult::Stopped;
                }
                let (queue, available) = &*self.state.get().accepted.queue;
                let mut queued = queue.lock().expect("accepted tcp queue");
                loop {
                    if let Some(stream) = queued.pop_front() {
                        return TcpAcceptResult::Accepted(TcpConnection::new(
                            stream,
                            Arc::clone(&self.state.get().generation_live),
                        ));
                    }
                    if self.state.get().stop_requested.load(Ordering::SeqCst) {
                        return TcpAcceptResult::Stopped;
                    }
                    queued = available.wait(queued).expect("accepted tcp queue wait");
                }
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
                self.state.get().generation_live.store(true, Ordering::SeqCst);
                self.state
                    .get()
                    .accepted_connections
                    .store(0, Ordering::SeqCst);
                {
                    let (queue, _) = &*self.state.get().accepted.queue;
                    queue.lock().expect("accepted tcp queue").clear();
                }
                *self
                    .state
                    .get()
                    .actual_address
                    .lock()
                    .expect("tcp listener address") = Some(actual);

                let stop_requested = Arc::clone(&self.state.get().stop_requested);
                let generation_live = Arc::clone(&self.state.get().generation_live);
                let accepted_connections = Arc::clone(&self.state.get().accepted_connections);
                let accepted = self.state.get().accepted.clone();
                let handle = thread::spawn(move || {
                    for accepted_stream in listener.incoming() {
                        if stop_requested.load(Ordering::SeqCst) {
                            break;
                        }
                        match accepted_stream {
                            Ok(stream) => {
                                if stop_requested.load(Ordering::SeqCst) {
                                    break;
                                }
                                accepted_connections.fetch_add(1, Ordering::SeqCst);
                                let (queue, available) = &*accepted.queue;
                                queue.lock().expect("accepted tcp queue").push_back(stream);
                                available.notify_one();
                            }
                            Err(_) => {
                                if stop_requested.load(Ordering::SeqCst) {
                                    break;
                                }
                            }
                        }
                    }
                    generation_live.store(false, Ordering::SeqCst);
                    let (_, available) = &*accepted.queue;
                    available.notify_all();
                });
                *self.state.get().handle.lock().expect("tcp listener handle") = Some(handle);
                Ok(())
            }

            stop {
                self.state.get().stop_requested.store(true, Ordering::SeqCst);
                self.state.get().generation_live.store(false, Ordering::SeqCst);
                let address = self.actual_bound_address();
                if let Some(address) = &address {
                    if let Ok(socket) = address.parse() {
                        let _ = TcpStream::connect(socket);
                    }
                }
                {
                    let (_, available) = &*self.state.get().accepted.queue;
                    available.notify_all();
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
                {
                    let (queue, _) = &*self.state.get().accepted.queue;
                    queue.lock().expect("accepted tcp queue").clear();
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

fn connect_tcp(remote: TcpSocketAddress, generation_live: Arc<AtomicBool>) -> TcpConnectResult {
    let socket = match remote.parse() {
        Ok(socket) => socket,
        Err(error) => return TcpConnectResult::Failed(error),
    };
    match TcpStream::connect(socket) {
        Ok(stream) => TcpConnectResult::Connected(TcpConnection::new(stream, generation_live)),
        Err(error) => TcpConnectResult::Failed(TcpTransportError {
            kind: TcpTransportErrorKind::ConnectFailed,
            detail: error.to_string(),
        }),
    }
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
            fn connect(&self, remote: TcpSocketAddress) -> TcpConnectResult;
        }

        runtime {
            fn observe_transport(&self) -> TcpProbeObservation {
                TcpProbeObservation {
                    requested: self.relations().transport.requested_bind_address(),
                    actual: self.relations().transport.actual_bound_address(),
                    accepted_connections: self.relations().transport.accepted_connections(),
                }
            }

            fn connect(&self, remote: TcpSocketAddress) -> TcpConnectResult {
                self.relations().transport.connect(remote)
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
                        TcpAcceptResult::Stopped => return Err(TcpTransportError::stopped()),
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

    fn composition(id: &str) -> Composition {
        let transport = TcpByteStreamTransport::select("api").expect("transport selection");
        let echo = TestEchoServer::define().select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("transport").expect("role"),
                fabric::authoring::Requires::<TcpByteStreamTransport>::provisional(),
            ),
            &transport,
        );

        Fabric::new(id)
            .expect("fabric")
            .with(loopback_tcp_transport("api"))
            .with(tcp_transport_probe("api"))
            .component(echo)
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

    fn echo_client(
        address: TcpSocketAddress,
        payload: &'static [u8],
    ) -> thread::JoinHandle<Vec<u8>> {
        thread::spawn(move || {
            let socket = address.parse().expect("socket addr");
            let mut stream = TcpStream::connect(socket).expect("tcp client connect");
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
        let probe = activate::<TcpTransportProbe>(&instance);
        let echo = activate::<TestEchoServer>(&instance);
        let observation = block_on(probe.observe_transport()).expect("observe");
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
            block_on(probe.observe_transport())
                .expect("observe after")
                .accepted_connections,
            2
        );
    }

    #[test]
    fn connect_returns_runtime_connection_value_without_echo_semantics() {
        let composition = composition("onoal.package.test.network.connect");
        let instance =
            started_instance(&composition, "onoal.package.test.network.connect.instance");
        let probe = activate::<TcpTransportProbe>(&instance);
        let echo = activate::<TestEchoServer>(&instance);
        let actual = block_on(probe.observe_transport())
            .expect("observe")
            .actual
            .expect("address");

        let connection = match block_on(probe.connect(actual)).expect("connect") {
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
        assert_eq!(api.accepted_connections, 0);
        assert_eq!(control.accepted_connections, 0);
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

        first.stop().expect("stop first");
        let fresh = started_instance(&composition, "onoal.package.test.network.instances.same");
        let fresh_probe = activate::<TcpTransportProbe>(&fresh);
        let fresh_observation = block_on(fresh_probe.observe_transport()).expect("fresh observe");
        assert_eq!(fresh_observation.accepted_connections, 0);
        assert!(fresh_observation.actual.is_some());
    }

    #[test]
    fn failures_peer_disconnect_stale_connection_and_stopped_instance_are_bounded() {
        let composition = composition("onoal.package.test.network.failures");
        let mut instance =
            started_instance(&composition, "onoal.package.test.network.failures.instance");
        let stale_connection = {
            let probe = activate::<TcpTransportProbe>(&instance);
            let echo = activate::<TestEchoServer>(&instance);

            let failed = block_on(probe.connect(TcpSocketAddress {
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

            let address = block_on(probe.observe_transport())
                .expect("observe")
                .actual
                .expect("address");
            let socket = address.parse().expect("socket addr");
            let stream = TcpStream::connect(socket).expect("peer connect");
            drop(stream);
            assert!(matches!(
                block_on(echo.serve_one_echo()).expect("peer disconnect"),
                Err(TcpTransportError {
                    kind: TcpTransportErrorKind::ReadFailed,
                    ..
                }) | Ok(0)
            ));

            let connected = match block_on(probe.connect(address)).expect("connect stale") {
                TcpConnectResult::Connected(connection) => connection,
                TcpConnectResult::Failed(error) => panic!("connect failed: {error:?}"),
            };
            let accepted_connections = block_on(probe.observe_transport())
                .expect("observe peer disconnect")
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
        let stopped_probe = instance.component::<TcpTransportProbe>().expect("probe");
        assert!(block_on(stopped_probe.connect(TcpSocketAddress::loopback_ephemeral())).is_err());
    }
}
