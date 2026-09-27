use std::path::PathBuf;

use fabric::prelude::*;
use fabric_package_relational_database::RelationalDatabase;

use crate::adapter::{SqliteRelationalDatabase, SqliteRelationalDatabaseConfig};
use crate::config::SqliteDatabasePath;

pub fn sqlite_database(
    name: &'static str,
    path: impl Into<PathBuf>,
) -> impl IntoFabricContribution {
    sqlite_database_with(name, SqliteDatabasePath::File(path.into()))
}

pub fn in_memory_sqlite_database(name: &'static str) -> impl IntoFabricContribution {
    sqlite_database_with(name, SqliteDatabasePath::InMemory)
}

pub fn sqlite_database_with(
    name: &'static str,
    path: SqliteDatabasePath,
) -> impl IntoFabricContribution {
    let selected = RelationalDatabase::select(name).expect("valid relational database name");
    FabricContribution::new().resource(
        selected
            .using(SqliteRelationalDatabase::new(
                SqliteRelationalDatabaseConfig { path },
            ))
            .expect("SqliteRelationalDatabase supports RelationalDatabase"),
    )
}
