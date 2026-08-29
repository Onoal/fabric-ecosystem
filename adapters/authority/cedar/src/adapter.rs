use std::collections::HashSet;
use std::str::FromStr;

use cedar_policy::{
    Authorizer, Context, Decision, Entities, Entity, EntityUid, PolicySet, Request,
    RestrictedExpression,
};
use fabric_resource_authority::{
    ActionId, ActorRef, AuthorityDecision, AuthorityDecisionAdapter, AuthorityError,
    AuthorityEvaluationInput, AuthorityGrant, AuthorityScopeId, RequestContext, ResourceRef,
};

#[derive(Default)]
pub struct CedarAuthorityDecisionAdapter;

impl CedarAuthorityDecisionAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl AuthorityDecisionAdapter for CedarAuthorityDecisionAdapter {
    fn authorize(
        &self,
        input: AuthorityEvaluationInput<'_>,
    ) -> Result<AuthorityDecision, AuthorityError> {
        let policy_set = build_policy_set(input.grants())?;
        let entities = build_entities(input.request())?;
        let cedar_request = Request::new(
            actor_uid(&input.request().actor)?,
            action_uid(&input.request().action)?,
            resource_uid(&input.request().resource)?,
            cedar_context(&input.request().context)?,
            None,
        )
        .map_err(|error| AuthorityError::Integrity {
            message: format!("failed to build cedar request: {error}"),
        })?;
        let response = Authorizer::new().is_authorized(&cedar_request, &policy_set, &entities);
        Ok(match response.decision() {
            Decision::Allow => AuthorityDecision::Allow,
            Decision::Deny => AuthorityDecision::Deny,
        })
    }
}

#[cfg(test)]
pub(crate) fn context_attribute_reaches_cedar(value: &str) -> Result<bool, AuthorityError> {
    let actor = ActorRef::new("system:context-principal")?;
    let scope_id = AuthorityScopeId::parse("context-scope")?;
    let action = ActionId::new("context.action")?;
    let resource = ResourceRef::new(scope_id, "context", "resource")?;
    let request = fabric_resource_authority::AuthorityRequest {
        actor,
        action,
        resource,
        context: RequestContext {
            attributes: [("proof".to_owned(), value.to_owned())].into(),
        },
    };
    let policy_set = PolicySet::from_str(
        "permit(principal, action, resource) when { context[\"proof\"] == \"allow\" };",
    )
    .map_err(|error| AuthorityError::Integrity {
        message: format!("failed to build context proof policy: {error}"),
    })?;
    let cedar_request = Request::new(
        actor_uid(&request.actor)?,
        action_uid(&request.action)?,
        resource_uid(&request.resource)?,
        cedar_context(&request.context)?,
        None,
    )
    .map_err(|error| AuthorityError::Integrity {
        message: format!("failed to build context proof request: {error}"),
    })?;
    let entities = build_entities(&request)?;
    Ok(matches!(
        Authorizer::new()
            .is_authorized(&cedar_request, &policy_set, &entities)
            .decision(),
        Decision::Allow
    ))
}

fn cedar_context(request_context: &RequestContext) -> Result<Context, AuthorityError> {
    let pairs = request_context
        .attributes
        .iter()
        .map(|(key, value)| {
            let expression =
                RestrictedExpression::from_str(&json_string(value)).map_err(|error| {
                    AuthorityError::InvalidInput {
                        message: format!("invalid request context value for {key}: {error}"),
                    }
                })?;
            Ok((key.clone(), expression))
        })
        .collect::<Result<Vec<_>, AuthorityError>>()?;
    Context::from_pairs(pairs).map_err(|error| AuthorityError::InvalidInput {
        message: format!("invalid request context: {error}"),
    })
}

fn build_policy_set(grants: &[AuthorityGrant]) -> Result<PolicySet, AuthorityError> {
    let mut source = String::new();
    for grant in grants {
        match grant {
            AuthorityGrant::ScopeControl { actor, scope_id } => {
                source.push_str(&format!(
                    "permit (principal == {}, action, resource) when {{ resource in {} }};\n",
                    actor_literal(actor.as_str()),
                    scope_literal(scope_id.as_str())
                ));
            }
            AuthorityGrant::Exact {
                actor,
                action,
                resource,
            } => {
                source.push_str(&format!(
                    "permit (principal == {}, action == {}, resource == {});\n",
                    actor_literal(actor.as_str()),
                    action_literal(action.as_str()),
                    resource_literal(resource.scope_id.as_str(), &resource.kind, &resource.id)
                ));
            }
        }
    }
    PolicySet::from_str(&source).map_err(|error| AuthorityError::Integrity {
        message: format!("failed to build cedar policy set: {error}"),
    })
}

fn build_entities(
    request: &fabric_resource_authority::AuthorityRequest,
) -> Result<Entities, AuthorityError> {
    let actor = Entity::with_uid(actor_uid(&request.actor)?);
    let action = Entity::with_uid(action_uid(&request.action)?);
    let scope_uid = scope_uid(&request.resource.scope_id)?;
    let scope = Entity::with_uid(scope_uid.clone());
    let resource = Entity::new(
        resource_uid(&request.resource)?,
        Default::default(),
        HashSet::from([scope_uid]),
    )
    .map_err(|error| AuthorityError::Integrity {
        message: format!("failed to build cedar resource entity: {error}"),
    })?;
    Entities::from_entities([actor, action, scope, resource], None).map_err(|error| {
        AuthorityError::Integrity {
            message: format!("failed to build cedar entities: {error}"),
        }
    })
}

fn actor_uid(actor: &ActorRef) -> Result<EntityUid, AuthorityError> {
    parse_entity_uid(&format!("Fabric::Actor::{}", json_string(actor.as_str())))
}

fn action_uid(action_id: &ActionId) -> Result<EntityUid, AuthorityError> {
    parse_entity_uid(&format!(
        "Fabric::Action::{}",
        json_string(action_id.as_str())
    ))
}

fn scope_uid(scope_id: &AuthorityScopeId) -> Result<EntityUid, AuthorityError> {
    parse_entity_uid(&format!(
        "Fabric::Scope::{}",
        json_string(scope_id.as_str())
    ))
}

fn resource_uid(resource: &ResourceRef) -> Result<EntityUid, AuthorityError> {
    parse_entity_uid(&resource_literal(
        resource.scope_id.as_str(),
        &resource.kind,
        &resource.id,
    ))
}

fn resource_literal(scope_id: &str, kind: &str, id: &str) -> String {
    format!(
        "Fabric::Resource::{}",
        json_string(&format!("{scope_id}::{kind}::{id}"))
    )
}

fn scope_literal(scope_id: &str) -> String {
    format!("Fabric::Scope::{}", json_string(scope_id))
}

fn action_literal(action_id: &str) -> String {
    format!("Fabric::Action::{}", json_string(action_id))
}

fn actor_literal(actor_ref: &str) -> String {
    format!("Fabric::Actor::{}", json_string(actor_ref))
}

fn json_string(value: &str) -> String {
    format!("{value:?}")
}

fn parse_entity_uid(value: &str) -> Result<EntityUid, AuthorityError> {
    EntityUid::from_str(value).map_err(|error| AuthorityError::Integrity {
        message: format!("failed to parse cedar entity uid: {error}"),
    })
}
