use std::sync::Arc;

use async_trait::async_trait;
use pingora::server::{ShutdownSignal, ShutdownSignalWatch};
use tokio::sync::{Mutex, watch};

#[derive(Clone)]
pub(crate) struct ShutdownController {
    tx: watch::Sender<Option<ShutdownSignalKind>>,
}

pub(crate) struct ChannelShutdownSignalWatch {
    rx: Arc<Mutex<watch::Receiver<Option<ShutdownSignalKind>>>>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ShutdownSignalKind {
    FastShutdown,
}

impl ShutdownController {
    pub(crate) fn new() -> (Self, ChannelShutdownSignalWatch) {
        let (tx, rx) = watch::channel(None);
        (
            Self { tx },
            ChannelShutdownSignalWatch {
                rx: Arc::new(Mutex::new(rx)),
            },
        )
    }

    pub(crate) fn shutdown_fast(&self) {
        let _ = self.tx.send(Some(ShutdownSignalKind::FastShutdown));
    }
}

#[async_trait]
impl ShutdownSignalWatch for ChannelShutdownSignalWatch {
    async fn recv(&self) -> ShutdownSignal {
        let mut rx = self.rx.lock().await;
        loop {
            if let Some(signal) = *rx.borrow() {
                return map_signal(signal);
            }
            if rx.changed().await.is_err() {
                return ShutdownSignal::FastShutdown;
            }
        }
    }
}

fn map_signal(signal: ShutdownSignalKind) -> ShutdownSignal {
    match signal {
        ShutdownSignalKind::FastShutdown => ShutdownSignal::FastShutdown,
    }
}
