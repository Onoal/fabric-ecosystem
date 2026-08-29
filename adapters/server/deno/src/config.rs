use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct DenoServerConfig {
    pub deno_bin: PathBuf,
    pub runtime_root: PathBuf,
    pub startup_timeout: Duration,
}

impl DenoServerConfig {
    pub fn new(deno_bin: PathBuf, runtime_root: PathBuf) -> Self {
        Self {
            deno_bin,
            runtime_root,
            startup_timeout: Duration::from_secs(10),
        }
    }
}
