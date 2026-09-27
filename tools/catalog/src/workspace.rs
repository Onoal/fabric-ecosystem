use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;

use crate::error::CatalogError;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
    workspace_root: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CargoPackage {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) id: String,
    pub(crate) description: Option<String>,
    pub(crate) publish: Option<Vec<String>>,
    pub(crate) manifest_path: PathBuf,
    pub(crate) dependencies: Vec<CargoDependency>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CargoDependency {
    pub(crate) name: String,
    pub(crate) kind: Option<String>,
}

pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
    pub(crate) packages: Vec<CargoPackage>,
}

impl Workspace {
    pub(crate) fn discover() -> Result<Self, CatalogError> {
        let output = Command::new("cargo")
            .args(["metadata", "--format-version", "1", "--no-deps"])
            .output()
            .map_err(|error| CatalogError::new(format!("failed to run cargo metadata: {error}")))?;
        if !output.status.success() {
            return Err(CatalogError::new(format!(
                "cargo metadata failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let metadata: CargoMetadata = serde_json::from_slice(&output.stdout)?;
        let members: BTreeSet<_> = metadata.workspace_members.into_iter().collect();
        let packages = metadata
            .packages
            .into_iter()
            .filter(|package| members.contains(&package.id))
            .collect();
        Ok(Self {
            root: metadata.workspace_root,
            packages,
        })
    }
}
