use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteDatabaseMaterialization {
    path: PathBuf,
}

impl SqliteDatabaseMaterialization {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn sqlite_file_path(&self) -> &Path {
        self.path.as_path()
    }
}
