use std::net::SocketAddr;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct PingoraIngressConfig {
    pub bind_address: SocketAddr,
    pub startup_timeout: Duration,
    pub probe_interval: Duration,
    pub shutdown_timeout: Duration,
}

impl PingoraIngressConfig {
    pub fn new(bind_address: SocketAddr) -> Self {
        Self {
            bind_address,
            startup_timeout: Duration::from_secs(5),
            probe_interval: Duration::from_millis(25),
            shutdown_timeout: Duration::from_secs(2),
        }
    }
}
