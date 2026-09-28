use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::CatalogError;
use crate::model::{
    ArtifactKind, CatalogArtifact, CatalogDependencies, CatalogIndex, CURRENT_SCHEMA_VERSION,
};
use crate::workspace::{CargoPackage, Workspace};

pub(crate) fn discover_catalog(workspace: &Workspace) -> Result<CatalogIndex, CatalogError> {
    let mut package_names = BTreeSet::new();
    for package in &workspace.packages {
        if !package_names.insert(package.name.clone()) {
            return Err(CatalogError::new(format!(
                "duplicate Cargo package name in workspace: {}",
                package.name
            )));
        }
    }

    let mut classified = BTreeMap::new();
    let mut package_path_by_name = BTreeMap::new();
    for package in &workspace.packages {
        let path = artifact_path(workspace, package)?;
        match classify_path(&path)? {
            Classification::Cataloged { kind, category } => {
                if classified
                    .insert(path.clone(), (package.clone(), kind, category))
                    .is_some()
                {
                    return Err(CatalogError::new(format!(
                        "duplicate catalog artifact path: {path}"
                    )));
                }
                package_path_by_name.insert(package.name.clone(), path);
            }
            Classification::Ignored => {}
        }
    }

    validate_supported_cargo_tomls_are_workspace_members(workspace, classified.keys())?;

    let mut artifacts = Vec::new();
    for (path, (package, kind, category)) in classified {
        let description = package
            .description
            .clone()
            .filter(|description| !description.trim().is_empty())
            .ok_or_else(|| {
                CatalogError::new(format!(
                    "cataloged artifact missing Cargo description: {path}"
                ))
            })?;
        let readme_path = workspace.root.join(&path).join("README.md");
        let readme = readme_path.is_file().then(|| format!("{path}/README.md"));
        artifacts.push(CatalogArtifact {
            kind,
            category,
            path: path.clone(),
            cargo_package: package.name.clone(),
            version: package.version.clone(),
            description,
            publishable: match &package.publish {
                None => true,
                Some(registries) => !registries.is_empty(),
            },
            readme,
            dependencies: dependency_projection(&package, &package_path_by_name),
        });
    }
    artifacts.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(CatalogIndex {
        schema_version: CURRENT_SCHEMA_VERSION,
        artifacts,
    })
}

fn artifact_path(workspace: &Workspace, package: &CargoPackage) -> Result<String, CatalogError> {
    let manifest_dir = package
        .manifest_path
        .parent()
        .ok_or_else(|| CatalogError::new(format!("manifest has no parent: {}", package.name)))?;
    let relative = manifest_dir.strip_prefix(&workspace.root).map_err(|_| {
        CatalogError::new(format!(
            "manifest path is outside workspace root: {}",
            package.manifest_path.display()
        ))
    })?;
    Ok(path_to_slash_string(relative))
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Classification {
    Cataloged {
        kind: ArtifactKind,
        category: Option<String>,
    },
    Ignored,
}

fn classify_path(path: &str) -> Result<Classification, CatalogError> {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["packages", category, _artifact] => Ok(Classification::Cataloged {
            kind: ArtifactKind::Package,
            category: Some((*category).to_owned()),
        }),
        ["hosts", _artifact] => Ok(Classification::Cataloged {
            kind: ArtifactKind::Host,
            category: None,
        }),
        ["compositions", category, _artifact] => Ok(Classification::Cataloged {
            kind: ArtifactKind::Composition,
            category: Some((*category).to_owned()),
        }),
        ["instances", category, _artifact] => Ok(Classification::Cataloged {
            kind: ArtifactKind::Instance,
            category: Some((*category).to_owned()),
        }),
        ["examples", _artifact] => Ok(Classification::Cataloged {
            kind: ArtifactKind::Example,
            category: None,
        }),
        ["tests", ..] | ["tools", ..] => Ok(Classification::Ignored),
        ["packages", ..]
        | ["hosts", ..]
        | ["compositions", ..]
        | ["instances", ..]
        | ["examples", ..] => Err(CatalogError::new(format!(
            "unsupported catalog artifact path: {path}"
        ))),
        _ => Ok(Classification::Ignored),
    }
}

fn validate_supported_cargo_tomls_are_workspace_members<'a>(
    workspace: &Workspace,
    cataloged_paths: impl IntoIterator<Item = &'a String>,
) -> Result<(), CatalogError> {
    let represented: BTreeSet<_> = cataloged_paths.into_iter().cloned().collect();
    for root in ["packages", "hosts", "compositions", "instances", "examples"] {
        let root_path = workspace.root.join(root);
        if !root_path.exists() {
            continue;
        }
        for manifest in find_cargo_tomls(&root_path)? {
            let artifact_dir = manifest.parent().expect("Cargo.toml parent");
            let relative =
                path_to_slash_string(artifact_dir.strip_prefix(&workspace.root).map_err(|_| {
                    CatalogError::new(format!(
                        "manifest path is outside workspace root: {}",
                        manifest.display()
                    ))
                })?);
            match classify_path(&relative)? {
                Classification::Cataloged { .. } => {
                    if !represented.contains(&relative) {
                        return Err(CatalogError::new(format!(
                            "catalog root artifact is not a workspace member: {relative}"
                        )));
                    }
                }
                Classification::Ignored => {}
            }
        }
    }
    Ok(())
}

fn find_cargo_tomls(root: &Path) -> Result<Vec<PathBuf>, CatalogError> {
    let mut manifests = Vec::new();
    let mut pending = vec![root.to_owned()];
    while let Some(path) = pending.pop() {
        let entries = fs::read_dir(&path).map_err(|error| {
            CatalogError::new(format!(
                "failed to read catalog root {}: {error}",
                path.display()
            ))
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                CatalogError::new(format!(
                    "failed to read catalog root entry in {}: {error}",
                    path.display()
                ))
            })?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                pending.push(entry_path);
            } else if entry_path.file_name() == Some(OsStr::new("Cargo.toml")) {
                manifests.push(entry_path);
            }
        }
    }
    Ok(manifests)
}

fn dependency_projection(
    package: &CargoPackage,
    package_path_by_name: &BTreeMap<String, String>,
) -> CatalogDependencies {
    let mut dependencies = CatalogDependencies::default();
    for dependency in &package.dependencies {
        let Some(path) = package_path_by_name.get(&dependency.name) else {
            continue;
        };
        match dependency.kind.as_deref() {
            None => dependencies.normal.push(path.clone()),
            Some("dev") => dependencies.development.push(path.clone()),
            Some("build") => dependencies.build.push(path.clone()),
            Some(_) => {}
        }
    }
    dependencies.normal.sort();
    dependencies.normal.dedup();
    dependencies.development.sort();
    dependencies.development.dedup();
    dependencies.build.sort();
    dependencies.build.dedup();
    dependencies
}

fn path_to_slash_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{CargoDependency, CargoPackage};

    #[test]
    fn classification_uses_repository_topology() {
        assert_eq!(
            classify_path("packages/data/key-value").expect("classification"),
            Classification::Cataloged {
                kind: ArtifactKind::Package,
                category: Some("data".to_owned())
            }
        );
        assert_eq!(
            classify_path("hosts/linux").expect("classification"),
            Classification::Cataloged {
                kind: ArtifactKind::Host,
                category: None
            }
        );
        assert_eq!(
            classify_path("compositions/web/http-server").expect("classification"),
            Classification::Cataloged {
                kind: ArtifactKind::Composition,
                category: Some("web".to_owned())
            }
        );
        assert_eq!(
            classify_path("examples/http-server").expect("classification"),
            Classification::Cataloged {
                kind: ArtifactKind::Example,
                category: None
            }
        );
        assert_eq!(
            classify_path("instances/web/http-server").expect("classification"),
            Classification::Cataloged {
                kind: ArtifactKind::Instance,
                category: Some("web".to_owned())
            }
        );
        assert_eq!(
            classify_path("tests/third-party-consumer").expect("classification"),
            Classification::Ignored
        );
        assert_eq!(
            classify_path("tools/catalog").expect("classification"),
            Classification::Ignored
        );
    }

    #[test]
    fn invalid_topology_rejects_clearly() {
        let error = classify_path("packages/data/foo/nested/bar").expect_err("invalid path");
        assert!(error
            .to_string()
            .contains("unsupported catalog artifact path: packages/data/foo/nested/bar"));

        let error = classify_path("instances/web/http-server/extra").expect_err("invalid path");
        assert!(error
            .to_string()
            .contains("unsupported catalog artifact path: instances/web/http-server/extra"));
    }

    #[test]
    fn dependency_projection_groups_internal_dependencies_only() {
        let package = CargoPackage {
            name: "consumer".to_owned(),
            version: "0.1.0".to_owned(),
            id: "consumer".to_owned(),
            description: Some("Consumer".to_owned()),
            publish: None,
            manifest_path: PathBuf::from("consumer/Cargo.toml"),
            dependencies: vec![
                CargoDependency {
                    name: "normal-internal".to_owned(),
                    kind: None,
                },
                CargoDependency {
                    name: "dev-internal".to_owned(),
                    kind: Some("dev".to_owned()),
                },
                CargoDependency {
                    name: "build-internal".to_owned(),
                    kind: Some("build".to_owned()),
                },
                CargoDependency {
                    name: "external".to_owned(),
                    kind: None,
                },
            ],
        };
        let paths = BTreeMap::from([
            (
                "normal-internal".to_owned(),
                "packages/data/normal".to_owned(),
            ),
            (
                "dev-internal".to_owned(),
                "packages/data/development".to_owned(),
            ),
            (
                "build-internal".to_owned(),
                "packages/data/build".to_owned(),
            ),
        ]);
        assert_eq!(
            dependency_projection(&package, &paths),
            CatalogDependencies {
                normal: vec!["packages/data/normal".to_owned()],
                development: vec!["packages/data/development".to_owned()],
                build: vec!["packages/data/build".to_owned()],
            }
        );
    }
}
