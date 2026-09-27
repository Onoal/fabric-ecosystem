use crate::model::{CatalogArtifact, CatalogDependencies};
use crate::query::{artifact_kind_label, CargoDependent, DependencyKind};

pub(crate) fn render_list<'a>(artifacts: impl IntoIterator<Item = &'a CatalogArtifact>) -> String {
    let mut output = String::new();
    for artifact in artifacts {
        output.push_str(&artifact_summary(artifact));
        output.push('\n');
    }
    output
}

pub(crate) fn render_show(artifact: &CatalogArtifact) -> String {
    let mut output = String::new();
    output.push_str(&format!("kind: {}\n", artifact_kind_label(&artifact.kind)));
    if let Some(category) = &artifact.category {
        output.push_str(&format!("category: {category}\n"));
    }
    output.push_str(&format!("path: {}\n", artifact.path));
    output.push_str(&format!("cargo package: {}\n", artifact.cargo_package));
    output.push_str(&format!("version: {}\n", artifact.version));
    output.push_str(&format!("description: {}\n", artifact.description));
    output.push_str(&format!("publishable: {}\n", artifact.publishable));
    if let Some(readme) = &artifact.readme {
        output.push_str(&format!("README: {readme}\n"));
    }
    output.push_str("Cargo dependencies:\n");
    push_dependency_groups(&mut output, &artifact.dependencies);
    output
}

pub(crate) fn render_dependencies(dependencies: &CatalogDependencies) -> String {
    let mut output = String::new();
    push_dependency_groups(&mut output, dependencies);
    output
}

pub(crate) fn render_dependents(dependents: &[CargoDependent<'_>]) -> String {
    let mut output = String::new();
    for kind in DependencyKind::all() {
        output.push_str(&format!("{}:\n", kind.label()));
        for dependent in dependents.iter().filter(|dependent| dependent.kind == kind) {
            output.push_str(&format!("  {}\n", dependent.artifact.path));
        }
    }
    output
}

fn artifact_summary(artifact: &CatalogArtifact) -> String {
    format!(
        "{}  {}  {}",
        artifact_kind_label(&artifact.kind),
        artifact.path,
        artifact.cargo_package
    )
}

fn push_dependency_groups(output: &mut String, dependencies: &CatalogDependencies) {
    push_dependency_group(output, "normal", &dependencies.normal);
    push_dependency_group(output, "development", &dependencies.development);
    push_dependency_group(output, "build", &dependencies.build);
}

fn push_dependency_group(output: &mut String, label: &str, paths: &[String]) {
    output.push_str(&format!("  {label}:\n"));
    for path in paths {
        output.push_str(&format!("    {path}\n"));
    }
}
