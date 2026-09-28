use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct TempDatabase {
    path: PathBuf,
}

impl TempDatabase {
    pub fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        Self {
            path: std::env::temp_dir().join(format!(
                "onoal-fabric-third-party-{label}-{}-{nanos}.db",
                std::process::id()
            )),
        }
    }

    pub fn path_buf(&self) -> PathBuf {
        self.path.clone()
    }
}

impl Drop for TempDatabase {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
