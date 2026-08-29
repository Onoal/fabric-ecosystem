#![forbid(unsafe_code)]

mod config;
mod contract;
mod materialization;
mod model;
mod native;
#[cfg(test)]
mod tests;

pub use config::DatabaseConfig;
pub use contract::{
    DatabaseContract, DatabaseService, SqliteDatabaseCompatibility,
    SqliteDatabaseCompatibilityContract, database_contract_id, database_contract_key,
    database_resource_id, sqlite_database_compatibility_contract_id,
    sqlite_database_compatibility_contract_key,
};
pub use materialization::SqliteDatabaseMaterialization;
pub use model::{DatabaseBinding, DatabaseRef, PreparedDatabase, PreparedDatabaseState};
pub use native::{DatabaseAdapter, DatabaseAdapterSupport, NativeDatabase};
