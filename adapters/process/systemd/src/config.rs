use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct SystemdProcessConfig {
    pub runtime_root: PathBuf,
    pub startup_timeout: Duration,
    pub stop_timeout: Duration,
}

impl SystemdProcessConfig {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self {
            runtime_root,
            startup_timeout: Duration::from_secs(10),
            stop_timeout: Duration::from_secs(10),
        }
    }
}
