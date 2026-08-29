use std::path::PathBuf;

use fabric_resource_process::{ProcessArtifact, ProcessError};

pub trait ProcessArtifactResolver: Send + Sync {
    fn resolve(&self, artifact: &ProcessArtifact) -> Result<ResolvedProcessArtifact, ProcessError>;
}

#[derive(Clone, Debug)]
pub struct ResolvedProcessArtifact {
    pub artifact_file: PathBuf,
    pub execution_root: PathBuf,
}
