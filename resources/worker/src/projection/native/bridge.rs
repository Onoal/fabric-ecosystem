use std::net::{SocketAddr, TcpListener};
use std::thread;

use axum::Router;
use tokio::net::TcpListener as TokioTcpListener;
use tokio::runtime::Builder;
use tokio::sync::oneshot;

use crate::WorkerError;
use fabric_projection::ProjectionLease;

pub(crate) struct BridgeLease {
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl ProjectionLease for BridgeLease {
    fn release(mut self: Box<Self>) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(crate) fn bind_loopback_listener() -> Result<(TcpListener, SocketAddr), WorkerError> {
    let listener =
        TcpListener::bind(("127.0.0.1", 0)).map_err(|error| WorkerError::StartFailed {
            message: format!("bind projection loopback listener: {error}"),
        })?;
    let address = listener
        .local_addr()
        .map_err(|error| WorkerError::StartFailed {
            message: format!("resolve projection loopback listener address: {error}"),
        })?;
    Ok((listener, address))
}

pub(crate) fn spawn_router(
    listener: TcpListener,
    router: Router,
) -> Result<BridgeLease, WorkerError> {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let thread = thread::Builder::new()
        .name("fabric-projection-bridge".to_owned())
        .spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("projection listener nonblocking");
            let runtime = Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("projection runtime");
            runtime.block_on(async move {
                let listener =
                    TokioTcpListener::from_std(listener).expect("convert projection listener");
                let server = axum::serve(listener, router);
                let _ = server
                    .with_graceful_shutdown(async {
                        let _ = shutdown_rx.await;
                    })
                    .await;
            });
        })
        .map_err(|error| WorkerError::StartFailed {
            message: format!("spawn projection bridge thread: {error}"),
        })?;
    Ok(BridgeLease {
        shutdown: Some(shutdown_tx),
        thread: Some(thread),
    })
}
