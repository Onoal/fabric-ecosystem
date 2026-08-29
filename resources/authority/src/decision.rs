use crate::error::AuthorityError;
use crate::model::{
    ActionId, ActorRef, AuthorityDecision, AuthorityRequest, AuthorityScopeId, ResourceRef,
};

pub trait AuthorityDecisionAdapter: Send + Sync {
    fn authorize(
        &self,
        input: AuthorityEvaluationInput<'_>,
    ) -> Result<AuthorityDecision, AuthorityError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorityGrant {
    ScopeControl {
        actor: ActorRef,
        scope_id: AuthorityScopeId,
    },
    Exact {
        actor: ActorRef,
        action: ActionId,
        resource: ResourceRef,
    },
}

pub struct AuthorityEvaluationInput<'a> {
    grants: &'a [AuthorityGrant],
    request: &'a AuthorityRequest,
}

impl<'a> AuthorityEvaluationInput<'a> {
    pub fn new(grants: &'a [AuthorityGrant], request: &'a AuthorityRequest) -> Self {
        Self { grants, request }
    }

    pub fn grants(&self) -> &'a [AuthorityGrant] {
        self.grants
    }

    pub fn request(&self) -> &'a AuthorityRequest {
        self.request
    }
}
