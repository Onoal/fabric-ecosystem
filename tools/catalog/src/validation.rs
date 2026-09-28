use std::collections::BTreeSet;

use crate::error::CatalogError;
use crate::model::{ArtifactKind, CatalogArtifact, CatalogIndex, CATALOG_V1, CATALOG_V2};
use crate::query::{artifact_kind_label, DependencyKind};

pub(crate) fn validate_catalog_index(index: &CatalogIndex) -> Result<(), CatalogError> {
    let schema = CatalogSchema::from_version(index.schema_version)?;
    let mut artifact_paths = BTreeSet::new();
    let mut cargo_packages = BTreeSet::new();
    let mut previous_path: Option<&str> = None;

    for artifact in &index.artifacts {
        if let Some(previous) = previous_path {
            if previous == artifact.path {
                return Err(CatalogError::new(format!(
                    "invalid Catalog {} structure: duplicate artifact path: {}",
                    schema.label(),
                    artifact.path
                )));
            }
            if previous > artifact.path.as_str() {
                return Err(CatalogError::new(format!(
                    "invalid Catalog {} structure: artifacts must be sorted by path: {previous} before {}",
                    schema.label(),
                    artifact.path
                )));
            }
        }
        previous_path = Some(&artifact.path);

        validate_artifact(schema, artifact)?;

        artifact_paths.insert(artifact.path.as_str());
        if !cargo_packages.insert(artifact.cargo_package.as_str()) {
            return Err(CatalogError::new(format!(
                "invalid Catalog {} structure: duplicate Cargo package: {}",
                schema.label(),
                artifact.cargo_package
            )));
        }
    }

    for artifact in &index.artifacts {
        for kind in DependencyKind::all() {
            validate_dependency_group(schema, artifact, kind, &artifact_paths)?;
        }
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CatalogSchema {
    V1,
    V2,
}

impl CatalogSchema {
    fn from_version(version: u8) -> Result<Self, CatalogError> {
        match version {
            CATALOG_V1 => Ok(Self::V1),
            CATALOG_V2 => Ok(Self::V2),
            other => Err(CatalogError::new(format!(
                "unsupported Catalog schema version: {other}"
            ))),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::V1 => "v1",
            Self::V2 => "v2",
        }
    }
}

fn validate_artifact(
    schema: CatalogSchema,
    artifact: &CatalogArtifact,
) -> Result<(), CatalogError> {
    let path = classify_catalog_path(schema, &artifact.path)?;
    if path.kind != artifact.kind {
        return Err(CatalogError::new(format!(
            "invalid Catalog {} structure: kind/path mismatch for {}: declared {} but path implies {}",
            schema.label(),
            artifact.path,
            artifact_kind_label(&artifact.kind),
            artifact_kind_label(&path.kind)
        )));
    }

    match (&path.category, &artifact.category) {
        (Some(expected), Some(actual)) if actual == expected => {}
        (Some(expected), Some(actual)) => {
            return Err(CatalogError::new(format!(
                "invalid Catalog {} structure: category/path mismatch for {}: declared {actual} but path implies {expected}",
                schema.label(),
                artifact.path
            )));
        }
        (Some(_), None) => {
            return Err(CatalogError::new(format!(
                "invalid Catalog {} structure: {} requires category: {}",
                schema.label(),
                artifact_kind_label(&artifact.kind),
                artifact.path
            )));
        }
        (None, Some(_)) => {
            return Err(CatalogError::new(format!(
                "invalid Catalog {} structure: {} must not have category: {}",
                schema.label(),
                artifact_kind_label(&artifact.kind),
                artifact.path
            )));
        }
        (None, None) => {}
    }

    if artifact.cargo_package.is_empty() {
        return Err(CatalogError::new(format!(
            "invalid Catalog {} structure: cargoPackage must be non-empty: {}",
            schema.label(),
            artifact.path
        )));
    }
    if artifact.version.is_empty() {
        return Err(CatalogError::new(format!(
            "invalid Catalog {} structure: version must be non-empty: {}",
            schema.label(),
            artifact.path
        )));
    }
    if artifact.description.trim().is_empty() {
        return Err(CatalogError::new(format!(
            "invalid Catalog {} structure: description must contain non-whitespace content: {}",
            schema.label(),
            artifact.path
        )));
    }
    if let Some(readme) = &artifact.readme {
        let expected = format!("{}/README.md", artifact.path);
        if readme != &expected {
            return Err(CatalogError::new(format!(
                "invalid Catalog {} structure: README path for {} must be {expected}",
                schema.label(),
                artifact.path
            )));
        }
    }

    Ok(())
}

fn validate_dependency_group(
    schema: CatalogSchema,
    artifact: &CatalogArtifact,
    kind: DependencyKind,
    artifact_paths: &BTreeSet<&str>,
) -> Result<(), CatalogError> {
    let mut previous: Option<&str> = None;
    for dependency in kind.dependencies(&artifact.dependencies) {
        if let Some(previous_dependency) = previous {
            if previous_dependency == dependency {
                return Err(CatalogError::new(format!(
                    "invalid Catalog {} structure: duplicate {} Cargo dependency reference in {}: {}",
                    schema.label(),
                    kind.label(),
                    artifact.path,
                    dependency
                )));
            }
            if previous_dependency > dependency.as_str() {
                return Err(CatalogError::new(format!(
                    "invalid Catalog {} structure: {} Cargo dependencies in {} must be sorted lexicographically",
                    schema.label(),
                    kind.label(),
                    artifact.path
                )));
            }
        }
        previous = Some(dependency);

        if !artifact_paths.contains(dependency.as_str()) {
            return Err(CatalogError::new(format!(
                "invalid Catalog {} structure: dangling {} Cargo dependency reference in {}: {}",
                schema.label(),
                kind.label(),
                artifact.path,
                dependency
            )));
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct PathClassification {
    kind: ArtifactKind,
    category: Option<String>,
}

fn classify_catalog_path(
    schema: CatalogSchema,
    path: &str,
) -> Result<PathClassification, CatalogError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(CatalogError::new(format!(
            "invalid Catalog {} path: {path}",
            schema.label()
        )));
    }

    let segments = path.split('/').collect::<Vec<_>>();
    let classification = match segments.as_slice() {
        ["packages", category, _artifact] => Ok(PathClassification {
            kind: ArtifactKind::Package,
            category: Some((*category).to_owned()),
        }),
        ["hosts", _artifact] => Ok(PathClassification {
            kind: ArtifactKind::Host,
            category: None,
        }),
        ["compositions", category, _artifact] => Ok(PathClassification {
            kind: ArtifactKind::Composition,
            category: Some((*category).to_owned()),
        }),
        ["instances", category, _artifact] if schema == CatalogSchema::V2 => {
            Ok(PathClassification {
                kind: ArtifactKind::Instance,
                category: Some((*category).to_owned()),
            })
        }
        ["examples", _artifact] => Ok(PathClassification {
            kind: ArtifactKind::Example,
            category: None,
        }),
        _ => Err(CatalogError::new(format!(
            "invalid Catalog {} topology: {path}",
            schema.label()
        ))),
    }?;
    Ok(classification)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CatalogDependencies, CATALOG_V1, CATALOG_V2};

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

    fn validate_as(
        schema_version: u8,
        artifacts: Vec<CatalogArtifact>,
    ) -> Result<(), CatalogError> {
        validate_catalog_index(&CatalogIndex {
            schema_version,
            artifacts,
        })
    }

    fn validate(artifacts: Vec<CatalogArtifact>) -> Result<(), CatalogError> {
        validate_as(CATALOG_V2, artifacts)
    }

    #[test]
    fn valid_catalog_topologies_are_accepted() {
        validate(vec![
            artifact(
                "compositions/web/http-server",
                ArtifactKind::Composition,
                Some("web"),
            ),
            artifact("examples/http-server", ArtifactKind::Example, None),
            artifact("hosts/linux", ArtifactKind::Host, None),
            artifact(
                "instances/web/http-server",
                ArtifactKind::Instance,
                Some("web"),
            ),
            artifact(
                "packages/networking/http",
                ArtifactKind::Package,
                Some("networking"),
            ),
        ])
        .expect("valid topology");
    }

    #[test]
    fn v1_rejects_instance_topology() {
        assert!(validate_as(
            CATALOG_V1,
            vec![artifact(
                "instances/web/http-server",
                ArtifactKind::Instance,
                Some("web")
            )]
        )
        .unwrap_err()
        .to_string()
        .contains("invalid Catalog v1 topology"));
    }

    #[test]
    fn unsupported_schema_rejects() {
        assert_eq!(
            validate_as(CATALOG_V2 + 1, Vec::new())
                .unwrap_err()
                .to_string(),
            "unsupported Catalog schema version: 3"
        );
    }

    #[test]
    fn kind_and_category_must_match_path() {
        assert!(validate(vec![artifact(
            "hosts/linux",
            ArtifactKind::Package,
            Some("data")
        )])
        .unwrap_err()
        .to_string()
        .contains("kind/path mismatch"));

        assert!(validate(vec![artifact(
            "compositions/web/http-server",
            ArtifactKind::Composition,
            Some("networking")
        )])
        .unwrap_err()
        .to_string()
        .contains("category/path mismatch"));

        assert!(validate(vec![artifact(
            "instances/web/http-server",
            ArtifactKind::Instance,
            Some("networking")
        )])
        .unwrap_err()
        .to_string()
        .contains("category/path mismatch"));

        assert!(validate(vec![artifact(
            "packages/data/key-value",
            ArtifactKind::Package,
            None
        )])
        .unwrap_err()
        .to_string()
        .contains("requires category"));

        assert!(validate(vec![artifact(
            "hosts/linux",
            ArtifactKind::Host,
            Some("linux")
        )])
        .unwrap_err()
        .to_string()
        .contains("must not have category"));

        assert!(validate(vec![artifact(
            "examples/http-server",
            ArtifactKind::Example,
            Some("web")
        )])
        .unwrap_err()
        .to_string()
        .contains("must not have category"));
    }

    #[test]
    fn canonical_paths_are_required() {
        for path in [
            "/packages/data/key-value",
            "packages/data/key-value/",
            "packages/./key-value",
            "packages/../key-value",
            "packages\\data\\key-value",
        ] {
            assert!(
                validate(vec![artifact(path, ArtifactKind::Package, Some("data"))])
                    .unwrap_err()
                    .to_string()
                    .contains("invalid Catalog v2 path"),
                "{path}"
            );
        }

        assert!(validate(vec![artifact(
            "packages/data/key-value/extra",
            ArtifactKind::Package,
            Some("data")
        )])
        .unwrap_err()
        .to_string()
        .contains("invalid Catalog v2 topology"));
    }

    #[test]
    fn value_invariants_are_required() {
        let mut empty_package = artifact(
            "packages/data/key-value",
            ArtifactKind::Package,
            Some("data"),
        );
        empty_package.cargo_package.clear();
        assert!(validate(vec![empty_package])
            .unwrap_err()
            .to_string()
            .contains("cargoPackage must be non-empty"));

        let mut empty_version = artifact(
            "packages/data/key-value",
            ArtifactKind::Package,
            Some("data"),
        );
        empty_version.version.clear();
        assert!(validate(vec![empty_version])
            .unwrap_err()
            .to_string()
            .contains("version must be non-empty"));

        let mut blank_description = artifact(
            "packages/data/key-value",
            ArtifactKind::Package,
            Some("data"),
        );
        blank_description.description = "   ".to_owned();
        assert!(validate(vec![blank_description])
            .unwrap_err()
            .to_string()
            .contains("description must contain"));

        let mut bad_readme = artifact(
            "packages/data/key-value",
            ArtifactKind::Package,
            Some("data"),
        );
        bad_readme.readme = Some("README.md".to_owned());
        assert!(validate(vec![bad_readme])
            .unwrap_err()
            .to_string()
            .contains("README path"));
    }

    #[test]
    fn canonical_artifact_order_is_required() {
        validate(vec![
            artifact("hosts/linux", ArtifactKind::Host, None),
            artifact(
                "packages/data/key-value",
                ArtifactKind::Package,
                Some("data"),
            ),
        ])
        .expect("sorted");

        assert!(validate(vec![
            artifact(
                "packages/data/key-value",
                ArtifactKind::Package,
                Some("data"),
            ),
            artifact("hosts/linux", ArtifactKind::Host, None),
        ])
        .unwrap_err()
        .to_string()
        .contains("artifacts must be sorted"));
    }

    #[test]
    fn dependencies_must_resolve_be_unique_and_sorted() {
        let data = artifact(
            "packages/data/key-value",
            ArtifactKind::Package,
            Some("data"),
        );
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
        http.dependencies.normal = vec![
            "packages/data/key-value".to_owned(),
            "packages/networking/tcp".to_owned(),
        ];
        validate(vec![data.clone(), http.clone(), tcp.clone()]).expect("sorted dependencies");

        let mut unsorted = http.clone();
        unsorted.dependencies.normal = vec![
            "packages/networking/tcp".to_owned(),
            "packages/data/key-value".to_owned(),
        ];
        assert!(validate(vec![data.clone(), unsorted, tcp.clone()])
            .unwrap_err()
            .to_string()
            .contains("must be sorted lexicographically"));

        let mut duplicate = http.clone();
        duplicate.dependencies.normal = vec![
            "packages/networking/tcp".to_owned(),
            "packages/networking/tcp".to_owned(),
        ];
        assert!(validate(vec![data.clone(), duplicate, tcp])
            .unwrap_err()
            .to_string()
            .contains("duplicate normal Cargo dependency reference"));

        let mut dangling = http;
        dangling.dependencies.normal = vec!["packages/unknown/missing".to_owned()];
        assert!(validate(vec![data, dangling])
            .unwrap_err()
            .to_string()
            .contains("dangling normal Cargo dependency reference"));
    }
}
