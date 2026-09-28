use std::path::PathBuf;

use fabric_composition_http_server::HttpServerCompositionConfig;
use fabric_package_observability_logging::ConsoleLogConfig;
use fabric_package_sqlite::SqliteDatabasePath;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalBackendCompositionConfig {
    pub http: HttpServerCompositionConfig,
    pub database_name: &'static str,
    pub database: SqliteDatabasePath,
    pub log_name: &'static str,
    pub logging: ConsoleLogConfig,
}

impl LocalBackendCompositionConfig {
    pub fn local(
        transport_name: &'static str,
        database_name: &'static str,
        database_path: impl Into<PathBuf>,
        log_name: &'static str,
    ) -> Self {
        Self {
            http: HttpServerCompositionConfig::local(transport_name),
            database_name,
            database: SqliteDatabasePath::File(database_path.into()),
            log_name,
            logging: ConsoleLogConfig::default(),
        }
    }

    pub fn in_memory(
        transport_name: &'static str,
        database_name: &'static str,
        log_name: &'static str,
    ) -> Self {
        Self {
            http: HttpServerCompositionConfig::local(transport_name),
            database_name,
            database: SqliteDatabasePath::InMemory,
            log_name,
            logging: ConsoleLogConfig::default(),
        }
    }
}
