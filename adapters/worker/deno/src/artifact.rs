use std::path::PathBuf;

use fabric_resource_worker::{WorkerError, WorkloadArtifact};

pub trait DenoArtifactResolver: Send + Sync {
    fn resolve(&self, artifact: &WorkloadArtifact) -> Result<ResolvedDenoArtifact, WorkerError>;
}

#[derive(Clone, Debug)]
pub struct ResolvedDenoArtifact {
    pub artifact_file: PathBuf,
    pub module_root: PathBuf,
}
