use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

use crate::decision::{AuthorityDecisionAdapter, AuthorityEvaluationInput, AuthorityGrant};
use crate::error::AuthorityError;
use crate::model::{
    ActionId, ActorRef, AuthorityDecision, AuthorityRequest, AuthorityScopeId, ResourceRef,
};
use crate::native::schema::initialize_schema;

#[derive(Debug)]
pub(crate) struct AuthorityDatabase {
    connection: Connection,
}

impl AuthorityDatabase {
    pub(crate) fn open(path: &Path) -> Result<Self, AuthorityError> {
        let mut connection = Connection::open(path).map_err(|error| {
            AuthorityError::persistence(format!("failed to open authority database: {error}"))
        })?;
        initialize_schema(&mut connection)?;
        Ok(Self { connection })
    }

    pub(crate) fn create_scope(&mut self) -> Result<AuthorityScopeId, AuthorityError> {
        let scope_id = AuthorityScopeId::new(format!("scope:{}", uuid_fragment()));
        self.connection
            .execute(
                "INSERT INTO scopes (scope_id) VALUES (?1)",
                params![scope_id.as_str()],
            )
            .map_err(|error| {
                AuthorityError::persistence(format!("failed to insert scope: {error}"))
            })?;
        Ok(scope_id)
    }

    pub(crate) fn grant_action(
        &mut self,
        actor: &ActorRef,
        action_id: &ActionId,
        resource: &ResourceRef,
    ) -> Result<(), AuthorityError> {
        self.ensure_scope_exists(&resource.scope_id)?;
        self.connection
            .execute(
                "INSERT OR IGNORE INTO grants (
                    grant_kind, actor_ref, scope_id, action_id, resource_kind, resource_id
                ) VALUES ('exact', ?1, ?2, ?3, ?4, ?5)",
                params![
                    actor.as_str(),
                    resource.scope_id.as_str(),
                    action_id.as_str(),
                    resource.kind,
                    resource.id
                ],
            )
            .map_err(|error| {
                AuthorityError::persistence(format!("failed to persist grant: {error}"))
            })?;
        Ok(())
    }

    pub(crate) fn grant_scope_control(
        &mut self,
        actor: &ActorRef,
        scope_id: &AuthorityScopeId,
    ) -> Result<(), AuthorityError> {
        self.ensure_scope_exists(scope_id)?;
        self.connection
            .execute(
                "INSERT OR IGNORE INTO grants (
                    grant_kind, actor_ref, scope_id, action_id, resource_kind, resource_id
                ) VALUES ('scope_control', ?1, ?2, NULL, NULL, NULL)",
                params![actor.as_str(), scope_id.as_str()],
            )
            .map_err(|error| {
                AuthorityError::persistence(format!(
                    "failed to persist scope control grant: {error}"
                ))
            })?;
        Ok(())
    }

    pub(crate) fn authorize(
        &self,
        request: &AuthorityRequest,
        decision_adapter: &dyn AuthorityDecisionAdapter,
    ) -> Result<AuthorityDecision, AuthorityError> {
        self.ensure_scope_exists(&request.resource.scope_id)?;
        let grants = self.load_grants()?;
        decision_adapter.authorize(AuthorityEvaluationInput::new(&grants, request))
    }

    fn ensure_scope_exists(&self, scope_id: &AuthorityScopeId) -> Result<(), AuthorityError> {
        let exists = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM scopes WHERE scope_id = ?1)",
                params![scope_id.as_str()],
                |row| row.get::<_, bool>(0),
            )
            .optional()
            .map_err(|error| {
                AuthorityError::persistence(format!("failed to resolve scope: {error}"))
            })?
            .unwrap_or(false);
        if !exists {
            return Err(AuthorityError::integrity("authority scope does not exist"));
        }
        Ok(())
    }

    fn load_grants(&self) -> Result<Vec<AuthorityGrant>, AuthorityError> {
        let mut statement = self
            .connection
            .prepare(
                "
                SELECT grant_kind, actor_ref, scope_id, action_id, resource_kind, resource_id
                FROM grants
                ORDER BY rowid
                ",
            )
            .map_err(|error| {
                AuthorityError::persistence(format!("failed to prepare grant query: {error}"))
            })?;
        let rows = statement
            .query_map([], |row| {
                let grant_kind = row.get::<_, String>(0)?;
                let actor_ref = row.get::<_, String>(1)?;
                let scope_id = row.get::<_, String>(2)?;
                let action_id = row.get::<_, Option<String>>(3)?;
                let resource_kind = row.get::<_, Option<String>>(4)?;
                let resource_id = row.get::<_, Option<String>>(5)?;
                match grant_kind.as_str() {
                    "scope_control" => Ok(AuthorityGrant::ScopeControl {
                        actor: parse_actor_ref(&actor_ref)?,
                        scope_id: parse_scope_id(&scope_id)?,
                    }),
                    "exact" => {
                        let scope_id = parse_scope_id(&scope_id)?;
                        let action = parse_action_id(&action_id.ok_or_else(|| {
                            rusqlite::Error::InvalidColumnType(
                                3,
                                "action_id".to_owned(),
                                rusqlite::types::Type::Null,
                            )
                        })?)?;
                        let resource_kind = resource_kind.ok_or_else(|| {
                            rusqlite::Error::InvalidColumnType(
                                4,
                                "resource_kind".to_owned(),
                                rusqlite::types::Type::Null,
                            )
                        })?;
                        let resource_id = resource_id.ok_or_else(|| {
                            rusqlite::Error::InvalidColumnType(
                                5,
                                "resource_id".to_owned(),
                                rusqlite::types::Type::Null,
                            )
                        })?;
                        Ok(AuthorityGrant::Exact {
                            actor: parse_actor_ref(&actor_ref)?,
                            action,
                            resource: ResourceRef::new(scope_id, resource_kind, resource_id)
                                .map_err(to_invalid_query)?,
                        })
                    }
                    _ => Err(rusqlite::Error::InvalidQuery),
                }
            })
            .map_err(|error| {
                AuthorityError::persistence(format!("failed to query grants: {error}"))
            })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|error| {
            AuthorityError::persistence(format!("failed to collect grants: {error}"))
        })
    }
}

fn uuid_fragment() -> String {
    hex::encode(rand::random::<[u8; 16]>())
}

fn parse_actor_ref(value: &str) -> Result<ActorRef, rusqlite::Error> {
    ActorRef::new(value).map_err(to_invalid_query)
}

fn parse_scope_id(value: &str) -> Result<AuthorityScopeId, rusqlite::Error> {
    AuthorityScopeId::parse(value).map_err(to_invalid_query)
}

fn parse_action_id(value: &str) -> Result<ActionId, rusqlite::Error> {
    ActionId::new(value).map_err(to_invalid_query)
}

fn to_invalid_query(error: AuthorityError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}
