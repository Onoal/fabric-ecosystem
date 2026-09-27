use fabric_package_relational_database::{RelationalDatabaseError, RelationalDatabaseErrorKind};

pub(crate) fn sqlite_error(
    kind: RelationalDatabaseErrorKind,
    error: rusqlite::Error,
) -> RelationalDatabaseError {
    RelationalDatabaseError::new(kind, error.to_string())
}
