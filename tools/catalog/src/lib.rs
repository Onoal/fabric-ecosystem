mod discovery;
mod error;
mod model;
mod output;
mod render;
mod workspace;

use std::env;

pub use error::CatalogError;

pub fn run_from_env() -> Result<(), CatalogError> {
    let command = Command::parse(env::args().nth(1))?;
    run(command)
}

pub fn check_committed_catalog() -> Result<(), CatalogError> {
    let workspace = workspace::Workspace::discover()?;
    let rendered = render_workspace_catalog(&workspace)?;
    output::check_catalog(&workspace.root, &rendered)
}

fn run(command: Command) -> Result<(), CatalogError> {
    let workspace = workspace::Workspace::discover()?;
    let rendered = render_workspace_catalog(&workspace)?;
    match command {
        Command::Write => output::write_catalog(&workspace.root, &rendered),
        Command::Check => output::check_catalog(&workspace.root, &rendered),
    }
}

fn render_workspace_catalog(
    workspace: &workspace::Workspace,
) -> Result<render::RenderedCatalog, CatalogError> {
    let catalog = discovery::discover_catalog(workspace)?;
    render::render_catalog(&catalog)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Write,
    Check,
}

impl Command {
    fn parse(input: Option<String>) -> Result<Self, CatalogError> {
        match input.as_deref() {
            Some("write") => Ok(Self::Write),
            Some("check") => Ok(Self::Check),
            Some(other) => Err(CatalogError::new(format!(
                "unknown command `{other}`: expected `write` or `check`"
            ))),
            None => Err(CatalogError::new(
                "missing command: expected `write` or `check`".to_owned(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_parsing_is_bounded_to_write_and_check() {
        assert_eq!(
            Command::parse(Some("write".to_owned())).expect("write"),
            Command::Write
        );
        assert_eq!(
            Command::parse(Some("check".to_owned())).expect("check"),
            Command::Check
        );
        assert!(Command::parse(Some("serve".to_owned())).is_err());
        assert!(Command::parse(None).is_err());
    }
}
