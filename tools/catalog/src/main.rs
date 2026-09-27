use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u8 = 1;
const JSON_PATH: &str = "catalog/index.json";
const MARKDOWN_PATH: &str = "catalog/index.md";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), CatalogError> {
    let command = env::args().nth(1).ok_or_else(|| {
        CatalogError::new("missing command: expected `write` or `check`".to_owned())
    })?;
    let workspace = Workspace::discover()?;
    let catalog = discover_catalog(&workspace)?;
    let rendered = render_catalog(&catalog)?;
    match command.as_str() {
        "write" => write_catalog(&workspace.root, &rendered),
        "check" => check_catalog(&workspace.root, &rendered),
        other => Err(CatalogError::new(format!(
            "unknown command `{other}`: expected `write` or `check`"
        ))),
    }
}

#[derive(Debug)]
struct CatalogError {
    message: String,
}

impl CatalogError {
    fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CatalogError {}

impl From<std::io::Error> for CatalogError {
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<serde_json::Error> for CatalogError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(error.to_string())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
    workspace_root: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
    id: String,
    description: Option<String>,
    publish: Option<Vec<String>>,
    manifest_path: PathBuf,
    dependencies: Vec<CargoDependency>,
}

#[derive(Clone, Debug, Deserialize)]
struct CargoDependency {
    name: String,
    kind: Option<String>,
}

struct Workspace {
    root: PathBuf,
    packages: Vec<CargoPackage>,
}

impl Workspace {
    fn discover() -> Result<Self, CatalogError> {
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

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CatalogIndex {
    schema_version: u8,
    artifacts: Vec<CatalogArtifact>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CatalogArtifact {
    kind: ArtifactKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    path: String,
    cargo_package: String,
    version: String,
    description: String,
    publishable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    readme: Option<String>,
    dependencies: CatalogDependencies,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
struct CatalogDependencies {
    normal: Vec<String>,
    development: Vec<String>,
    build: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ArtifactKind {
    Package,
    Host,
    Composition,
    Example,
}

impl ArtifactKind {
    fn heading(&self) -> &'static str {
        match self {
            Self::Package => "Packages",
            Self::Host => "Hosts",
            Self::Composition => "Compositions",
            Self::Example => "Examples",
        }
    }
}

fn discover_catalog(workspace: &Workspace) -> Result<CatalogIndex, CatalogError> {
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
        schema_version: SCHEMA_VERSION,
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
        ["examples", _artifact] => Ok(Classification::Cataloged {
            kind: ArtifactKind::Example,
            category: None,
        }),
        ["tests", ..] | ["tools", ..] => Ok(Classification::Ignored),
        ["packages", ..] | ["hosts", ..] | ["compositions", ..] | ["examples", ..] => Err(
            CatalogError::new(format!("unsupported catalog artifact path: {path}")),
        ),
        _ => Ok(Classification::Ignored),
    }
}

fn validate_supported_cargo_tomls_are_workspace_members<'a>(
    workspace: &Workspace,
    cataloged_paths: impl IntoIterator<Item = &'a String>,
) -> Result<(), CatalogError> {
    let represented: BTreeSet<_> = cataloged_paths.into_iter().cloned().collect();
    for root in ["packages", "hosts", "compositions", "examples"] {
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
        for entry in fs::read_dir(path)? {
            let entry = entry?;
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

struct RenderedCatalog {
    json: String,
    markdown: String,
}

fn render_catalog(catalog: &CatalogIndex) -> Result<RenderedCatalog, CatalogError> {
    let mut json = serde_json::to_string_pretty(catalog)?;
    json.push('\n');
    Ok(RenderedCatalog {
        json,
        markdown: render_markdown(catalog),
    })
}

fn render_markdown(catalog: &CatalogIndex) -> String {
    let mut output = String::new();
    output.push_str("# Fabric Ecosystem Catalog\n\n");
    output.push_str("Generated by `cargo run -p fabric-ecosystem-catalog-tool -- write`.\n");
    output.push_str("See [catalog/README.md](README.md) for the Catalog contract.\n\n");
    output.push_str(&format!("Schema version: `{}`\n\n", catalog.schema_version));

    for kind in [
        ArtifactKind::Package,
        ArtifactKind::Host,
        ArtifactKind::Composition,
        ArtifactKind::Example,
    ] {
        output.push_str(&format!("## {}\n\n", kind.heading()));
        let artifacts: Vec<_> = catalog
            .artifacts
            .iter()
            .filter(|artifact| artifact.kind == kind)
            .collect();
        if artifacts.is_empty() {
            output.push_str("_None._\n\n");
            continue;
        }
        if artifacts.iter().any(|artifact| artifact.category.is_some()) {
            let mut categories = BTreeSet::new();
            for artifact in &artifacts {
                categories.insert(artifact.category.as_deref().unwrap_or(""));
            }
            for category in categories {
                if !category.is_empty() {
                    output.push_str(&format!("### {category}\n\n"));
                }
                for artifact in artifacts
                    .iter()
                    .filter(|artifact| artifact.category.as_deref().unwrap_or("") == category)
                {
                    push_markdown_item(&mut output, artifact);
                }
                output.push('\n');
            }
        } else {
            for artifact in artifacts {
                push_markdown_item(&mut output, artifact);
            }
            output.push('\n');
        }
    }
    output
}

fn push_markdown_item(output: &mut String, artifact: &CatalogArtifact) {
    let name = artifact
        .path
        .rsplit('/')
        .next()
        .expect("artifact path name");
    output.push_str(&format!(
        "- [{}]({}) — {}",
        name,
        markdown_link(&artifact.path),
        artifact.description
    ));
    if let Some(readme) = &artifact.readme {
        output.push_str(&format!(" ([README]({}))", markdown_link(readme)));
    }
    output.push('\n');
}

fn markdown_link(path: &str) -> String {
    format!("../{path}")
}

fn write_catalog(root: &Path, rendered: &RenderedCatalog) -> Result<(), CatalogError> {
    fs::write(root.join(JSON_PATH), &rendered.json)?;
    fs::write(root.join(MARKDOWN_PATH), &rendered.markdown)?;
    Ok(())
}

fn check_catalog(root: &Path, rendered: &RenderedCatalog) -> Result<(), CatalogError> {
    let committed_json = fs::read_to_string(root.join(JSON_PATH)).map_err(|error| {
        CatalogError::new(format!("failed to read committed {JSON_PATH}: {error}"))
    })?;
    let committed_markdown = fs::read_to_string(root.join(MARKDOWN_PATH)).map_err(|error| {
        CatalogError::new(format!("failed to read committed {MARKDOWN_PATH}: {error}"))
    })?;
    let mut stale = Vec::new();
    if committed_json != rendered.json {
        stale.push(JSON_PATH);
    }
    if committed_markdown != rendered.markdown {
        stale.push(MARKDOWN_PATH);
    }
    if stale.is_empty() {
        Ok(())
    } else {
        Err(CatalogError::new(format!(
            "committed catalog output is stale: {}",
            stale.join(", ")
        )))
    }
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

    fn artifact(path: &str, kind: ArtifactKind, category: Option<&str>) -> CatalogArtifact {
        CatalogArtifact {
            kind,
            category: category.map(str::to_owned),
            path: path.to_owned(),
            cargo_package: format!("package-{path}"),
            version: "0.1.0".to_owned(),
            description: format!("Description for {path}"),
            publishable: true,
            readme: None,
            dependencies: CatalogDependencies::default(),
        }
    }

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

    #[test]
    fn rendering_is_deterministically_sorted_by_path() {
        let mut catalog = CatalogIndex {
            schema_version: SCHEMA_VERSION,
            artifacts: vec![
                artifact(
                    "packages/networking/tcp",
                    ArtifactKind::Package,
                    Some("networking"),
                ),
                artifact(
                    "packages/data/key-value",
                    ArtifactKind::Package,
                    Some("data"),
                ),
            ],
        };
        catalog
            .artifacts
            .sort_by(|left, right| left.path.cmp(&right.path));
        let rendered = render_catalog(&catalog).expect("render");
        assert!(
            rendered
                .json
                .find("packages/data/key-value")
                .expect("data package")
                < rendered
                    .json
                    .find("packages/networking/tcp")
                    .expect("networking package")
        );
    }

    #[test]
    fn json_contains_schema_version_and_required_fields() {
        let catalog = CatalogIndex {
            schema_version: SCHEMA_VERSION,
            artifacts: vec![artifact(
                "packages/data/key-value",
                ArtifactKind::Package,
                Some("data"),
            )],
        };
        let rendered = render_catalog(&catalog).expect("render");
        assert!(rendered.json.contains("\"schemaVersion\": 1"));
        assert!(rendered.json.contains("\"cargoPackage\""));
        assert!(rendered.json.ends_with('\n'));
    }

    #[test]
    fn markdown_groups_by_kind_and_category() {
        let catalog = CatalogIndex {
            schema_version: SCHEMA_VERSION,
            artifacts: vec![
                artifact("hosts/linux", ArtifactKind::Host, None),
                artifact(
                    "packages/data/key-value",
                    ArtifactKind::Package,
                    Some("data"),
                ),
            ],
        };
        let markdown = render_markdown(&catalog);
        assert!(markdown.contains("## Packages"));
        assert!(markdown.contains("### data"));
        assert!(markdown.contains("- [key-value](../packages/data/key-value)"));
        assert!(markdown.contains("## Hosts"));
        assert!(markdown.contains("- [linux](../hosts/linux)"));
    }

    #[test]
    fn committed_catalog_output_is_current() {
        let workspace = Workspace::discover().expect("workspace");
        let catalog = discover_catalog(&workspace).expect("catalog");
        let rendered = render_catalog(&catalog).expect("rendered catalog");
        check_catalog(&workspace.root, &rendered).expect("committed catalog");
    }
}
