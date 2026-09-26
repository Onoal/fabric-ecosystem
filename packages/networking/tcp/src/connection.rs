use std::fmt;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::{TcpSocketAddress, TcpTransportError, TcpTransportErrorKind};

/// Runtime value for one accepted or connected TCP byte stream.
///
/// A `TcpConnection` is not a Fabric Resource, Component, System, semantic
/// endpoint identity, or OXP Session. It is a live value tied to the current
/// materialized generation of its owning TCP realization.
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
    pub(crate) fn new(stream: TcpStream, generation_live: Arc<AtomicBool>) -> Self {
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

    /// Writes all bytes to the stream or returns a bounded TCP write error.
    pub fn write_all(&self, bytes: &[u8]) -> Result<(), TcpTransportError> {
        self.ensure_live()?;
        self.stream
            .lock()
            .expect("tcp connection")
            .write_all(bytes)
            .map_err(|error| {
                TcpTransportError::new(TcpTransportErrorKind::WriteFailed, error.to_string())
            })
    }

    /// Reads until peer EOF using standard blocking stream semantics.
    pub fn read_to_end(&self) -> Result<Vec<u8>, TcpTransportError> {
        self.ensure_live()?;
        let mut bytes = Vec::new();
        self.stream
            .lock()
            .expect("tcp connection")
            .read_to_end(&mut bytes)
            .map_err(|error| {
                TcpTransportError::new(TcpTransportErrorKind::ReadFailed, error.to_string())
            })?;
        Ok(bytes)
    }

    /// Reads up to `max_bytes` from the stream.
    ///
    /// This delegates to the underlying blocking TCP stream: it may block until
    /// at least one byte is available, EOF occurs, or an OS error occurs.
    /// Passing `0` is valid and returns an empty buffer immediately.
    pub fn read_some(&self, max_bytes: usize) -> Result<Vec<u8>, TcpTransportError> {
        self.ensure_live()?;
        if max_bytes == 0 {
            return Ok(Vec::new());
        }
        let mut bytes = vec![0; max_bytes];
        let count = self
            .stream
            .lock()
            .expect("tcp connection")
            .read(&mut bytes)
            .map_err(|error| {
                TcpTransportError::new(TcpTransportErrorKind::ReadFailed, error.to_string())
            })?;
        bytes.truncate(count);
        Ok(bytes)
    }

    pub fn shutdown_write(&self) -> Result<(), TcpTransportError> {
        self.ensure_live()?;
        self.stream
            .lock()
            .expect("tcp connection")
            .shutdown(Shutdown::Write)
            .map_err(|error| {
                TcpTransportError::new(TcpTransportErrorKind::ShutdownFailed, error.to_string())
            })
    }

    pub fn shutdown_both(&self) -> Result<(), TcpTransportError> {
        self.ensure_live()?;
        self.stream
            .lock()
            .expect("tcp connection")
            .shutdown(Shutdown::Both)
            .map_err(|error| {
                TcpTransportError::new(TcpTransportErrorKind::ShutdownFailed, error.to_string())
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
