use std::collections::VecDeque;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use crate::TcpSocketAddress;

#[derive(Clone)]
pub(crate) struct AcceptedConnections {
    pub(crate) queue: Arc<(Mutex<VecDeque<TcpStream>>, Condvar)>,
}

impl Default for AcceptedConnections {
    fn default() -> Self {
        Self {
            queue: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
        }
    }
}

pub struct TcpListenerState {
    pub(crate) actual_address: Mutex<Option<TcpSocketAddress>>,
    pub(crate) accepted_connections: Arc<AtomicUsize>,
    pub(crate) stop_requested: Arc<AtomicBool>,
    pub(crate) generation_live: Arc<AtomicBool>,
    pub(crate) accepted: AcceptedConnections,
    pub(crate) handle: Mutex<Option<JoinHandle<()>>>,
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
