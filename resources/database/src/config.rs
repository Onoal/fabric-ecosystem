use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatabaseConfig {
    pub root: PathBuf,
}
