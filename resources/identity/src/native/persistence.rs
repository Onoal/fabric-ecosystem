use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::IdentityError;
use crate::model::{Principal, PrincipalId};
use crate::native::schema::initialize_schema;

#[derive(Debug)]
pub(crate) struct IdentityDatabase {
    connection: Connection,
}

impl IdentityDatabase {
    pub(crate) fn open(path: &Path) -> Result<Self, IdentityError> {
        let mut connection = Connection::open(path).map_err(|error| {
            IdentityError::persistence(format!("failed to open identity database: {error}"))
        })?;
        initialize_schema(&mut connection)?;
        Ok(Self { connection })
    }

    pub(crate) fn create_principal(&mut self) -> Result<Principal, IdentityError> {
        let principal = Principal {
            id: PrincipalId::new(hex::encode(rand::random::<[u8; 16]>())),
        };
        self.connection
            .execute(
                "INSERT INTO principals (principal_id) VALUES (?1)",
                params![principal.id.as_str()],
            )
            .map_err(|error| {
                IdentityError::persistence(format!("failed to insert principal: {error}"))
            })?;
        Ok(principal)
    }

    pub(crate) fn get_principal(
        &self,
        principal_id: &PrincipalId,
    ) -> Result<Option<Principal>, IdentityError> {
        self.connection
            .query_row(
                "SELECT principal_id FROM principals WHERE principal_id = ?1",
                params![principal_id.as_str()],
                |row| {
                    Ok(Principal {
                        id: PrincipalId::parse(row.get::<_, String>(0)?).map_err(to_sql_error)?,
                    })
                },
            )
            .optional()
            .map_err(|error| {
                IdentityError::persistence(format!("failed to load principal: {error}"))
            })
    }
}

fn to_sql_error(error: IdentityError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::other(error.to_string())),
    )
}
