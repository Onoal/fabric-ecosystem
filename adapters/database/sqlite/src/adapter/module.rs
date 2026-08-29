use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use fabric_core::{Health, ModuleError};
use fabric_resource::{ResourceError, ResourceInstanceId};
use fabric_resource_database::{
    DatabaseAdapter, DatabaseConfig, DatabaseRef, DatabaseService, PreparedDatabase,
    PreparedDatabaseState, SqliteDatabaseCompatibility, SqliteDatabaseMaterialization,
};
use fabric_resource_registry::{
    ResourceInspection, ResourceInspectionEntry, ResourceRegistryError,
};

const SQLITE_FILE_NAME: &str = "database.sqlite";
const SQLITE_BUSY_TIMEOUT_MS: u64 = 1_000;

pub struct SqliteDatabaseAdapter {
    root: Mutex<Option<PathBuf>>,
    #[cfg(test)]
    faults: Mutex<SqliteDatabaseFaults>,
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct SqliteDatabaseFaults {
    pub(crate) fail_after_initialize_once: bool,
}

impl SqliteDatabaseAdapter {
    pub fn new() -> Self {
        Self {
            root: Mutex::new(None),
            #[cfg(test)]
            faults: Mutex::new(SqliteDatabaseFaults::default()),
        }
    }

    fn configured_root(&self) -> Result<PathBuf, ResourceError> {
        self.root
            .lock()
            .expect("sqlite root")
            .clone()
            .ok_or_else(|| ResourceError::InvalidInput {
                message: "database configuration has not been consumed".to_owned(),
            })
    }

    fn database_path(&self, resource_id: &ResourceInstanceId) -> Result<PathBuf, ResourceError> {
        Ok(self
            .configured_root()?
            .join(resource_id.as_str())
            .join(SQLITE_FILE_NAME))
    }
}

impl Default for SqliteDatabaseAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl DatabaseAdapter for SqliteDatabaseAdapter {
    fn initialize(&self, config: &DatabaseConfig) -> Result<(), ModuleError> {
        fs::create_dir_all(&config.root).map_err(|error| ModuleError::new(error.to_string()))
    }

    fn consume_configuration(&self, config: &DatabaseConfig) -> Result<(), ResourceRegistryError> {
        let mut root = self.root.lock().expect("sqlite root");
        if let Some(existing) = root.as_ref() {
            if existing != &config.root {
                return Err(ResourceRegistryError::ConfigurationRejected {
                    message: "database configuration change is not supported after initialization"
                        .to_owned(),
                });
            }
            return Ok(());
        }
        *root = Some(config.root.clone());
        Ok(())
    }

    fn inspect(&self) -> Result<ResourceInspection, ResourceRegistryError> {
        let configured = self.root.lock().expect("sqlite root").is_some();
        ResourceInspection::new(vec![
            ResourceInspectionEntry::public("configured", configured.to_string())?,
            ResourceInspectionEntry::public("adapter", "sqlite")?,
            ResourceInspectionEntry::public("resource", "database")?,
        ])
    }

    fn health(&self) -> Health {
        Health::Healthy
    }
}

impl DatabaseService for SqliteDatabaseAdapter {
    fn prepare(
        &self,
        resource_id: &ResourceInstanceId,
    ) -> Result<PreparedDatabaseState, ResourceError> {
        let path = self.database_path(resource_id)?;
        let created = !path.exists();
        if let Err(error) = initialize_database(&path) {
            if created {
                cleanup_file(&path);
            }
            return Err(error);
        }
        #[cfg(test)]
        {
            let mut faults = self.faults.lock().expect("sqlite faults");
            if faults.fail_after_initialize_once {
                faults.fail_after_initialize_once = false;
                if created {
                    cleanup_file(&path);
                }
                return Err(ResourceError::PrepareFailed {
                    message: "injected database prepare failure".to_owned(),
                });
            }
        }
        Ok(PreparedDatabaseState { created })
    }

    fn cleanup(&self, prepared: &PreparedDatabase) {
        if prepared.created
            && let Ok(path) = self.database_path(&prepared.resource_id)
        {
            cleanup_file(&path);
        }
    }

    fn verify(&self, resource_id: &ResourceInstanceId) -> Result<(), ResourceError> {
        let path = self.database_path(resource_id)?;
        verify_database(&path)
    }
}

impl SqliteDatabaseCompatibility for SqliteDatabaseAdapter {
    fn materialize_sqlite(
        &self,
        reference: &DatabaseRef,
    ) -> Result<SqliteDatabaseMaterialization, ResourceError> {
        let path = self.database_path(reference.resource_id())?;
        verify_database(&path)?;
        Ok(SqliteDatabaseMaterialization::new(path))
    }
}

fn initialize_database(path: &Path) -> Result<(), ResourceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_prepare)?;
    }
    let connection = open_sqlite(path).map_err(sql_prepare)?;
    configure_sqlite(&connection)
}

fn verify_database(path: &Path) -> Result<(), ResourceError> {
    if !path.is_file() {
        return Err(ResourceError::Integrity {
            message: "database backing is missing".to_owned(),
        });
    }
    let connection = open_sqlite(path).map_err(sql_integrity)?;
    configure_sqlite(&connection)
}

fn sqlite_busy_timeout() -> std::time::Duration {
    std::time::Duration::from_millis(SQLITE_BUSY_TIMEOUT_MS)
}

fn configure_sqlite(connection: &Connection) -> Result<(), ResourceError> {
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(sql_prepare)?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(sql_prepare)?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(sql_prepare)?;
    connection
        .busy_timeout(sqlite_busy_timeout())
        .map_err(sql_prepare)
}

fn cleanup_file(path: &Path) {
    let _ = fs::remove_file(path);
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
}

fn io_prepare(error: std::io::Error) -> ResourceError {
    ResourceError::PrepareFailed {
        message: error.to_string(),
    }
}

fn sql_prepare(error: rusqlite::Error) -> ResourceError {
    ResourceError::PrepareFailed {
        message: error.to_string(),
    }
}

fn sql_integrity(error: rusqlite::Error) -> ResourceError {
    ResourceError::Integrity {
        message: error.to_string(),
    }
}

fn open_sqlite(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open(path)
}
