mod database;
mod kv;
mod runtime;

use std::net::{SocketAddr, TcpListener};

use fabric_resource_worker::WorkerError;

pub(crate) use database::{DatabaseBridgeConfig, database_router};
pub(crate) use kv::{KvBridgeConfig, kv_router};
pub(crate) use runtime::{BridgeRuntime, cleanup_bridges, spawn_router};

pub(crate) fn bind_loopback_listener() -> Result<(TcpListener, SocketAddr), WorkerError> {
    let listener =
        TcpListener::bind(("127.0.0.1", 0)).map_err(|error| WorkerError::StartFailed {
            message: format!("bind loopback listener: {error}"),
        })?;
    let address = listener
        .local_addr()
        .map_err(|error| WorkerError::StartFailed {
            message: format!("resolve loopback listener address: {error}"),
        })?;
    Ok((listener, address))
}

pub(crate) fn database_base_url(address: SocketAddr) -> String {
    format!("http://127.0.0.1:{}/db", address.port())
}

pub(crate) fn kv_base_url(address: SocketAddr) -> String {
    format!("http://127.0.0.1:{}/kv", address.port())
}
