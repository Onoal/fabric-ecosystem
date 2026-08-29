use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::native::schema::initialize_schema;
use crate::{
    PreparedService, Service, ServiceError, ServiceId, ServiceProtocol, ServiceRequirement,
    ServiceScope,
};

#[derive(Debug)]
pub(crate) struct ServiceDatabase {
    connection: Connection,
}

impl ServiceDatabase {
    pub(crate) fn open(path: &Path) -> Result<Self, ServiceError> {
        let mut connection = Connection::open(path).map_err(|error| {
            ServiceError::persistence(format!("open service database: {error}"))
        })?;
        initialize_schema(&mut connection)?;
        let database = Self { connection };
        database.check_integrity()?;
        Ok(database)
    }

    pub(crate) fn ensure_service(
        &mut self,
        scope: &ServiceScope,
        requirement: &ServiceRequirement,
    ) -> Result<PreparedService, ServiceError> {
        if let Some(service) = self.find_service(scope, requirement)? {
            return Ok(PreparedService {
                service,
                created: false,
            });
        }

        let service = Service::from_parts(
            ServiceId::for_scope(scope, requirement),
            scope.clone(),
            requirement.clone(),
        );
        self.connection
            .execute(
                "
                INSERT INTO services (
                    service_id,
                    service_scope,
                    service_name,
                    service_protocol
                ) VALUES (?1, ?2, ?3, ?4)
                ",
                params![
                    service.id.as_str(),
                    service.scope.as_str(),
                    service.requirement.name.as_str(),
                    service.requirement.protocol.as_str(),
                ],
            )
            .map_err(|error| ServiceError::persistence(format!("insert service: {error}")))?;
        Ok(PreparedService {
            service,
            created: true,
        })
    }

    pub(crate) fn cleanup_prepared_service(
        &mut self,
        prepared: &PreparedService,
    ) -> Result<(), ServiceError> {
        if !prepared.created {
            return Ok(());
        }
        self.connection
            .execute(
                "DELETE FROM services WHERE service_id = ?1",
                params![prepared.service.id.as_str()],
            )
            .map_err(|error| ServiceError::persistence(format!("delete service: {error}")))?;
        Ok(())
    }

    pub(crate) fn find_service(
        &self,
        scope: &ServiceScope,
        requirement: &ServiceRequirement,
    ) -> Result<Option<Service>, ServiceError> {
        self.connection
            .query_row(
                "
                SELECT service_id, service_scope, service_name, service_protocol
                FROM services
                WHERE service_scope = ?1 AND service_name = ?2
                ",
                params![scope.as_str(), requirement.name.as_str()],
                |row| {
                    let protocol = ServiceProtocol::parse(&row.get::<_, String>(3)?)
                        .map_err(to_sqlite_error)?;
                    if protocol != requirement.protocol {
                        return Err(to_sqlite_error(ServiceError::integrity(
                            "persisted service protocol does not match requested service identity",
                        )));
                    }
                    let id = ServiceId::parse(row.get::<_, String>(0)?).map_err(to_sqlite_error)?;
                    let scope =
                        ServiceScope::new(row.get::<_, String>(1)?).map_err(to_sqlite_error)?;
                    let requirement = ServiceRequirement {
                        name: crate::ServiceName::new(row.get::<_, String>(2)?)
                            .map_err(to_sqlite_error)?,
                        protocol,
                    };
                    Ok(Service::from_parts(id, scope, requirement))
                },
            )
            .optional()
            .map_err(|error| map_load_error("find service", error))
    }

    pub(crate) fn get_service(&self, service_id: &ServiceId) -> Result<Service, ServiceError> {
        self.connection
            .query_row(
                "
                SELECT service_id, service_scope, service_name, service_protocol
                FROM services
                WHERE service_id = ?1
                ",
                params![service_id.as_str()],
                |row| {
                    let id = ServiceId::parse(row.get::<_, String>(0)?).map_err(to_sqlite_error)?;
                    let scope =
                        ServiceScope::new(row.get::<_, String>(1)?).map_err(to_sqlite_error)?;
                    let requirement = ServiceRequirement {
                        name: crate::ServiceName::new(row.get::<_, String>(2)?)
                            .map_err(to_sqlite_error)?,
                        protocol: ServiceProtocol::parse(&row.get::<_, String>(3)?)
                            .map_err(to_sqlite_error)?,
                    };
                    Ok(Service::from_parts(id, scope, requirement))
                },
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => ServiceError::NotFound,
                other => map_load_error("get service", other),
            })
    }

    fn check_integrity(&self) -> Result<(), ServiceError> {
        let mut statement = self
            .connection
            .prepare("SELECT service_id FROM services")
            .map_err(|error| {
                ServiceError::persistence(format!("prepare integrity query: {error}"))
            })?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| ServiceError::persistence(format!("query service ids: {error}")))?;
        for id in ids {
            let id =
                id.map_err(|error| ServiceError::persistence(format!("read service id: {error}")))?;
            let _ = self.get_service(&ServiceId::parse(id)?)?;
        }
        Ok(())
    }
}

fn to_sqlite_error(error: ServiceError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::other(error.to_string())),
    )
}

fn map_load_error(operation: &str, error: rusqlite::Error) -> ServiceError {
    match error {
        rusqlite::Error::FromSqlConversionFailure(_, _, source) => {
            ServiceError::integrity(source.to_string())
        }
        other => ServiceError::persistence(format!("failed to {operation}: {other}")),
    }
}
