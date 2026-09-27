use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use fabric_package_relational_database::{RelationalDatabaseError, RelationalDatabaseErrorKind};
use rusqlite::Connection;

#[derive(Default)]
pub(crate) struct SqliteDatabaseState {
    connection: Mutex<Option<Connection>>,
    live: AtomicBool,
    stopped: AtomicBool,
}

impl SqliteDatabaseState {
    pub(crate) fn with_connection<T>(
        &self,
        failure_kind: RelationalDatabaseErrorKind,
        operation: impl FnOnce(&Connection) -> Result<T, RelationalDatabaseError>,
    ) -> Result<T, RelationalDatabaseError> {
        if !self.live.load(Ordering::SeqCst) {
            return if self.stopped.load(Ordering::SeqCst) {
                Err(RelationalDatabaseError::stopped())
            } else {
                Err(RelationalDatabaseError::not_started())
            };
        }
        let guard = self.connection.lock().map_err(|_| {
            RelationalDatabaseError::new(failure_kind, "sqlite connection lock poisoned")
        })?;
        let connection = guard
            .as_ref()
            .ok_or_else(RelationalDatabaseError::not_started)?;
        operation(connection)
    }

    pub(crate) fn mark_started(
        &self,
        connection: Connection,
    ) -> Result<(), fabric::core::ModuleError> {
        *self.connection.lock().map_err(|_| {
            fabric::core::ModuleError::new("sqlite connection lock poisoned during start")
        })? = Some(connection);
        self.stopped.store(false, Ordering::SeqCst);
        self.live.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub(crate) fn stop(&self) -> Result<(), fabric::core::ModuleError> {
        self.live.store(false, Ordering::SeqCst);
        self.stopped.store(true, Ordering::SeqCst);
        let connection = self
            .connection
            .lock()
            .map_err(|_| {
                fabric::core::ModuleError::new("sqlite connection lock poisoned during stop")
            })?
            .take();
        if let Some(connection) = connection {
            connection.close().map_err(|(_, error)| {
                fabric::core::ModuleError::new(format!("sqlite close failed: {error}"))
            })?;
        }
        Ok(())
    }
}
