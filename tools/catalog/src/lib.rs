mod discovery;
mod error;
mod model;
mod output;
mod presentation;
mod query;
mod reader;
mod render;
mod validation;
mod workspace;

use std::env;
use std::path::Path;

pub use error::CatalogError;
pub use model::{ArtifactKind, CatalogArtifact, CatalogDependencies};
pub use query::{CargoDependent, Catalog, DependencyKind};

pub fn run_from_env() -> Result<(), CatalogError> {
    let command = Command::parse(env::args().nth(1))?;
    run(command)
}

pub fn check_committed_catalog() -> Result<(), CatalogError> {
    let workspace = workspace::Workspace::discover()?;
    let rendered = render_workspace_catalog(&workspace)?;
    output::check_catalog(&workspace.root, &rendered)
}

pub fn read_committed_catalog_from(root: &Path) -> Result<Catalog, CatalogError> {
    reader::read_catalog_from(root)
}

fn run(command: Command) -> Result<(), CatalogError> {
    match command {
        Command::Write => {
            let workspace = workspace::Workspace::discover()?;
            let rendered = render_workspace_catalog(&workspace)?;
            output::write_catalog(&workspace.root, &rendered)
        }
        Command::Check => {
            let workspace = workspace::Workspace::discover()?;
            let rendered = render_workspace_catalog(&workspace)?;
            output::check_catalog(&workspace.root, &rendered)
        }
        Command::Validate => {
            let catalog = reader::read_catalog_from(Path::new("."))?;
            println!("Catalog v1 valid: {} artifacts", catalog.artifacts().len());
            Ok(())
        }
        Command::List => {
            let catalog = reader::read_catalog_from(Path::new("."))?;
            print!("{}", presentation::render_list(catalog.artifacts()));
            Ok(())
        }
        Command::ListKind(kind) => {
            let catalog = reader::read_catalog_from(Path::new("."))?;
            print!(
                "{}",
                presentation::render_list(catalog.artifacts_by_kind(kind))
            );
            Ok(())
        }
        Command::ListCategory(category) => {
            let catalog = reader::read_catalog_from(Path::new("."))?;
            print!(
                "{}",
                presentation::render_list(catalog.artifacts_by_category(&category))
            );
            Ok(())
        }
        Command::Show(path) => {
            let catalog = reader::read_catalog_from(Path::new("."))?;
            let artifact = catalog.artifact_by_path(&path)?;
            print!("{}", presentation::render_show(artifact));
            Ok(())
        }
        Command::ShowPackage(cargo_package) => {
            let catalog = reader::read_catalog_from(Path::new("."))?;
            let artifact = catalog.artifact_by_cargo_package(&cargo_package)?;
            print!("{}", presentation::render_show(artifact));
            Ok(())
        }
        Command::Deps(path) => {
            let catalog = reader::read_catalog_from(Path::new("."))?;
            let dependencies = catalog.direct_dependencies(&path)?;
            print!("{}", presentation::render_dependencies(dependencies));
            Ok(())
        }
        Command::Dependents(path) => {
            let catalog = reader::read_catalog_from(Path::new("."))?;
            let dependents = catalog.direct_dependents(&path)?;
            print!("{}", presentation::render_dependents(&dependents));
            Ok(())
        }
    }
}

fn render_workspace_catalog(
    workspace: &workspace::Workspace,
) -> Result<render::RenderedCatalog, CatalogError> {
    let catalog = discovery::discover_catalog(workspace)?;
    render::render_catalog(&catalog)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Command {
    Write,
    Check,
    Validate,
    List,
    ListKind(model::ArtifactKind),
    ListCategory(String),
    Show(String),
    ShowPackage(String),
    Deps(String),
    Dependents(String),
}

impl Command {
    fn parse(input: Option<String>) -> Result<Self, CatalogError> {
        let args = env::args().skip(1).collect::<Vec<_>>();
        Self::parse_args(input, &args)
    }

    fn parse_args(input: Option<String>, args: &[String]) -> Result<Self, CatalogError> {
        match input.as_deref() {
            Some("write") => expect_no_args(args, Self::Write),
            Some("check") => expect_no_args(args, Self::Check),
            Some("validate") => expect_no_args(args, Self::Validate),
            Some("list") => expect_no_args(args, Self::List),
            Some("list-kind") => {
                let value = required_argument(args, "list-kind", "kind")?;
                Ok(Self::ListKind(query::parse_artifact_kind(value)?))
            }
            Some("list-category") => {
                let value = required_argument(args, "list-category", "category")?;
                Ok(Self::ListCategory(value.to_owned()))
            }
            Some("show") => {
                let value = required_argument(args, "show", "path")?;
                Ok(Self::Show(value.to_owned()))
            }
            Some("show-package") => {
                let value = required_argument(args, "show-package", "cargo package")?;
                Ok(Self::ShowPackage(value.to_owned()))
            }
            Some("deps") => {
                let value = required_argument(args, "deps", "path")?;
                Ok(Self::Deps(value.to_owned()))
            }
            Some("dependents") => {
                let value = required_argument(args, "dependents", "path")?;
                Ok(Self::Dependents(value.to_owned()))
            }
            Some(other) => Err(CatalogError::new(format!(
                "unknown command `{other}`: expected `write`, `check`, `validate`, `list`, `list-kind`, `list-category`, `show`, `show-package`, `deps`, or `dependents`"
            ))),
            None => Err(CatalogError::new(
                "missing command: expected `write`, `check`, `validate`, `list`, `list-kind`, `list-category`, `show`, `show-package`, `deps`, or `dependents`".to_owned(),
            )),
        }
    }
}

fn expect_no_args(args: &[String], command: Command) -> Result<Command, CatalogError> {
    if args.len() == 1 {
        Ok(command)
    } else {
        Err(CatalogError::new(format!(
            "`{}` does not accept additional arguments",
            args.first().map(String::as_str).unwrap_or("")
        )))
    }
}

fn required_argument<'a>(
    args: &'a [String],
    command: &str,
    argument: &str,
) -> Result<&'a str, CatalogError> {
    match args {
        [_, value] => Ok(value),
        [_] => Err(CatalogError::new(format!(
            "`{command}` requires {argument}"
        ))),
        _ => Err(CatalogError::new(format!(
            "`{command}` expects exactly one {argument}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_parsing_is_bounded_to_write_and_check() {
        assert_eq!(
            Command::parse_args(Some("write".to_owned()), &["write".to_owned()]).expect("write"),
            Command::Write
        );
        assert_eq!(
            Command::parse_args(Some("check".to_owned()), &["check".to_owned()]).expect("check"),
            Command::Check
        );
        assert_eq!(
            Command::parse_args(Some("validate".to_owned()), &["validate".to_owned()])
                .expect("validate"),
            Command::Validate
        );
        assert_eq!(
            Command::parse_args(Some("list".to_owned()), &["list".to_owned()]).expect("list"),
            Command::List
        );
        assert_eq!(
            Command::parse_args(
                Some("show".to_owned()),
                &["show".to_owned(), "packages/networking/http".to_owned()]
            )
            .expect("show"),
            Command::Show("packages/networking/http".to_owned())
        );
        assert!(Command::parse_args(Some("serve".to_owned()), &["serve".to_owned()]).is_err());
        assert!(Command::parse_args(None, &[]).is_err());
        assert!(
            Command::parse_args(Some("list-kind".to_owned()), &["list-kind".to_owned()]).is_err()
        );
        assert!(Command::parse_args(
            Some("list-kind".to_owned()),
            &["list-kind".to_owned(), "resource".to_owned()]
        )
        .unwrap_err()
        .to_string()
        .contains("unsupported artifact kind: resource"));
    }
}
