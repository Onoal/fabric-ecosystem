mod state;

use std::net::{TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;

use crate::{
    transport::{TcpAcceptResult, TcpByteStreamTransport, TcpConnectResult},
    TcpConnection, TcpSocketAddress, TcpTransportError, TcpTransportErrorKind,
};

use state::TcpListenerState;

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
                    return TcpConnectResult::Failed(TcpTransportError::not_started());
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
                if !self.config.requested_bind.is_loopback_host() {
                    return Err(fabric::core::ModuleError::new(format!(
                        "loopback tcp transport requires a loopback bind host, got {}",
                        self.config.requested_bind.host
                    )));
                }
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

fn connect_tcp(
    remote: TcpSocketAddress,
    generation_live: Arc<std::sync::atomic::AtomicBool>,
) -> TcpConnectResult {
    let socket = match remote.parse() {
        Ok(socket) => socket,
        Err(error) => return TcpConnectResult::Failed(error),
    };
    match TcpStream::connect(socket) {
        Ok(stream) => TcpConnectResult::Connected(TcpConnection::new(stream, generation_live)),
        Err(error) => TcpConnectResult::Failed(TcpTransportError::new(
            TcpTransportErrorKind::ConnectFailed,
            error.to_string(),
        )),
    }
}
