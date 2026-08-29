use std::net::TcpListener;
use std::thread;

use axum::Router;
use fabric_resource_worker::WorkerError;
use tokio::net::TcpListener as TokioTcpListener;
use tokio::runtime::Builder;
use tokio::sync::oneshot;

pub(crate) struct BridgeRuntime {
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

pub(crate) fn spawn_router(
    listener: TcpListener,
    router: Router,
) -> Result<BridgeRuntime, WorkerError> {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let thread = thread::Builder::new()
        .name("fabric-resource-worker-bridge".to_owned())
        .spawn(move || {
            listener
                .set_nonblocking(true)
                .expect("bridge listener nonblocking");
            let runtime = Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("bridge runtime");
            runtime.block_on(async move {
                let listener =
                    TokioTcpListener::from_std(listener).expect("convert bridge listener");
                let server = axum::serve(listener, router);
                let _ = server
                    .with_graceful_shutdown(async {
                        let _ = shutdown_rx.await;
                    })
                    .await;
            });
        })
        .map_err(|error| WorkerError::StartFailed {
            message: format!("spawn bridge runtime thread: {error}"),
        })?;
    Ok(BridgeRuntime {
        shutdown: Some(shutdown_tx),
        thread: Some(thread),
    })
}

pub(crate) fn cleanup_bridges(bridges: Vec<BridgeRuntime>) {
    for mut bridge in bridges {
        if let Some(shutdown) = bridge.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = bridge.thread.take() {
            let _ = thread.join();
        }
    }
}
