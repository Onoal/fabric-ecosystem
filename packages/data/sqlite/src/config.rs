use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SqliteDatabasePath {
    File(PathBuf),
    InMemory,
}
