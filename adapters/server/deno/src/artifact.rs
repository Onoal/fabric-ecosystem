use std::path::PathBuf;

use fabric_resource_server::{ServerArtifact, ServerError};

pub trait DenoServerArtifactResolver: Send + Sync {
    fn resolve(&self, artifact: &ServerArtifact)
    -> Result<ResolvedDenoServerArtifact, ServerError>;
}

#[derive(Clone, Debug)]
pub struct ResolvedDenoServerArtifact {
    pub artifact_file: PathBuf,
    pub module_root: PathBuf,
}
