use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use crate::error::CatalogError;
use crate::model::{ArtifactKind, CatalogArtifact, CatalogDependencies, CatalogIndex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DependencyKind {
    Normal,
    Development,
    Build,
}

impl DependencyKind {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Development => "development",
            Self::Build => "build",
        }
    }

    pub(crate) fn dependencies<'a>(&self, dependencies: &'a CatalogDependencies) -> &'a [String] {
        match self {
            Self::Normal => &dependencies.normal,
            Self::Development => &dependencies.development,
            Self::Build => &dependencies.build,
        }
    }

    pub(crate) fn all() -> [Self; 3] {
        [Self::Normal, Self::Development, Self::Build]
    }
}

impl fmt::Display for DependencyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CargoDependent<'a> {
    pub artifact: &'a CatalogArtifact,
    pub kind: DependencyKind,
}

#[derive(Debug)]
pub struct Catalog {
    index: CatalogIndex,
    by_path: BTreeMap<String, usize>,
    by_package: BTreeMap<String, usize>,
}

impl Catalog {
    pub(crate) fn from_validated(index: CatalogIndex) -> Self {
        let mut by_path = BTreeMap::new();
        let mut by_package = BTreeMap::new();
        for (position, artifact) in index.artifacts.iter().enumerate() {
            by_path.insert(artifact.path.clone(), position);
            by_package.insert(artifact.cargo_package.clone(), position);
        }

        Self {
            index,
            by_path,
            by_package,
        }
    }

    pub fn artifacts(&self) -> &[CatalogArtifact] {
        &self.index.artifacts
    }

    pub fn artifact_by_path(&self, path: &str) -> Result<&CatalogArtifact, CatalogError> {
        self.by_path
            .get(path)
            .map(|position| &self.index.artifacts[*position])
            .ok_or_else(|| CatalogError::new(format!("artifact path not found: {path}")))
    }

    pub fn artifact_by_cargo_package(
        &self,
        cargo_package: &str,
    ) -> Result<&CatalogArtifact, CatalogError> {
        self.by_package
            .get(cargo_package)
            .map(|position| &self.index.artifacts[*position])
            .ok_or_else(|| CatalogError::new(format!("Cargo package not found: {cargo_package}")))
    }

    pub fn artifacts_by_kind(&self, kind: ArtifactKind) -> Vec<&CatalogArtifact> {
        self.index
            .artifacts
            .iter()
            .filter(|artifact| artifact.kind == kind)
            .collect()
    }

    pub fn artifacts_by_category(&self, category: &str) -> Vec<&CatalogArtifact> {
        self.index
            .artifacts
            .iter()
            .filter(|artifact| artifact.category.as_deref() == Some(category))
            .collect()
    }

    pub fn direct_dependencies(&self, path: &str) -> Result<&CatalogDependencies, CatalogError> {
        Ok(&self.artifact_by_path(path)?.dependencies)
    }

    pub fn direct_dependents(&self, path: &str) -> Result<Vec<CargoDependent<'_>>, CatalogError> {
        self.artifact_by_path(path)?;
        let mut dependents = Vec::new();
        for artifact in &self.index.artifacts {
            for kind in DependencyKind::all() {
                if kind
                    .dependencies(&artifact.dependencies)
                    .iter()
                    .any(|dependency| dependency == path)
                {
                    dependents.push(CargoDependent { artifact, kind });
                }
            }
        }
        dependents.sort_by(|left, right| {
            left.artifact
                .path
                .cmp(&right.artifact.path)
                .then(left.kind.cmp(&right.kind))
        });
        Ok(dependents)
    }
}

pub(crate) fn parse_artifact_kind(value: &str) -> Result<ArtifactKind, CatalogError> {
    ArtifactKind::from_str(value)
}

impl FromStr for ArtifactKind {
    type Err = CatalogError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "package" => Ok(Self::Package),
            "host" => Ok(Self::Host),
            "composition" => Ok(Self::Composition),
            "example" => Ok(Self::Example),
            other => Err(CatalogError::new(format!(
                "unsupported artifact kind: {other}"
            ))),
        }
    }
}

pub(crate) fn artifact_kind_label(kind: &ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Package => "package",
        ArtifactKind::Host => "host",
        ArtifactKind::Composition => "composition",
        ArtifactKind::Example => "example",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SCHEMA_VERSION;
    use crate::validation;

    fn artifact(path: &str, kind: ArtifactKind, category: Option<&str>) -> CatalogArtifact {
        CatalogArtifact {
            kind,
            category: category.map(str::to_owned),
            path: path.to_owned(),
            cargo_package: format!("cargo-{}", path.replace('/', "-")),
            version: "0.1.0".to_owned(),
            description: format!("Description for {path}"),
            publishable: true,
            readme: None,
            dependencies: CatalogDependencies::default(),
        }
    }

    fn catalog_with(mut artifacts: Vec<CatalogArtifact>) -> Catalog {
        artifacts.sort_by(|left, right| left.path.cmp(&right.path));
        let index = CatalogIndex {
            schema_version: SCHEMA_VERSION,
            artifacts,
        };
        validation::validate_catalog_index(&index).expect("valid catalog fixture");
        Catalog::from_validated(index)
    }

    #[test]
    fn exact_lookup_and_filters_are_deterministic() {
        let catalog = catalog_with(vec![
            artifact("hosts/linux", ArtifactKind::Host, None),
            artifact(
                "packages/data/key-value",
                ArtifactKind::Package,
                Some("data"),
            ),
            artifact(
                "packages/networking/http",
                ArtifactKind::Package,
                Some("networking"),
            ),
        ]);
        assert_eq!(
            catalog
                .artifact_by_path("packages/networking/http")
                .expect("path")
                .cargo_package,
            "cargo-packages-networking-http"
        );
        assert_eq!(
            catalog
                .artifact_by_cargo_package("cargo-packages-networking-http")
                .expect("package")
                .path,
            "packages/networking/http"
        );
        assert_eq!(catalog.artifacts_by_kind(ArtifactKind::Package).len(), 2);
        assert_eq!(catalog.artifacts_by_category("networking").len(), 1);
        assert_eq!(
            catalog
                .artifact_by_path("packages/foo/bar")
                .unwrap_err()
                .to_string(),
            "artifact path not found: packages/foo/bar"
        );
    }

    #[test]
    fn dependencies_and_dependents_preserve_direct_edge_kind() {
        let tcp = artifact(
            "packages/networking/tcp",
            ArtifactKind::Package,
            Some("networking"),
        );
        let mut http = artifact(
            "packages/networking/http",
            ArtifactKind::Package,
            Some("networking"),
        );
        http.dependencies.normal = vec!["packages/networking/tcp".to_owned()];
        let mut composition = artifact(
            "compositions/web/http-server",
            ArtifactKind::Composition,
            Some("web"),
        );
        composition.dependencies.development = vec!["packages/networking/tcp".to_owned()];
        let catalog = catalog_with(vec![composition, http, tcp]);

        assert_eq!(
            catalog
                .direct_dependencies("packages/networking/http")
                .expect("dependencies")
                .normal,
            vec!["packages/networking/tcp".to_owned()]
        );
        let dependents = catalog
            .direct_dependents("packages/networking/tcp")
            .expect("dependents");
        assert_eq!(dependents.len(), 2);
        assert_eq!(dependents[0].artifact.path, "compositions/web/http-server");
        assert_eq!(dependents[0].kind, DependencyKind::Development);
        assert_eq!(dependents[1].artifact.path, "packages/networking/http");
        assert_eq!(dependents[1].kind, DependencyKind::Normal);
    }
}
