use std::sync::Arc;

use fabric_core::{ContractId, ContractKey};
use fabric_resource::ResourceId;

use crate::error::AuthorityError;
use crate::model::{
    ActionId, ActorRef, AuthorityDecision, AuthorityRequest, AuthorityScopeId, ResourceRef,
};
#[cfg(feature = "test-support")]
use crate::native::module::NativeAuthorityFailurePoint;
use crate::native::module::SharedAuthorityState;

const AUTHORITY_CONTRACT_ID: &str = "fabric.resource.authority";

pub fn authority_resource_id() -> ResourceId {
    ResourceId::new("authority").expect("static authority resource id")
}

pub fn authority_contract_id() -> ContractId {
    ContractId::new(AUTHORITY_CONTRACT_ID).expect("static authority contract id")
}

pub fn authority_contract_key() -> ContractKey<AuthorityContract> {
    ContractKey::provisional(authority_contract_id())
}

#[derive(Clone)]
pub struct AuthorityContract {
    state: Arc<SharedAuthorityState>,
}

impl AuthorityContract {
    pub(crate) fn new(state: Arc<SharedAuthorityState>) -> Self {
        Self { state }
    }

    pub fn create_scope(&self) -> Result<AuthorityScopeId, AuthorityError> {
        self.state.create_scope()
    }

    pub fn grant_action(
        &self,
        actor: &ActorRef,
        action_id: &ActionId,
        resource: &ResourceRef,
    ) -> Result<(), AuthorityError> {
        self.state.grant_action(actor, action_id, resource)
    }

    pub fn grant_scope_control(
        &self,
        actor: &ActorRef,
        scope_id: &AuthorityScopeId,
    ) -> Result<(), AuthorityError> {
        self.state.grant_scope_control(actor, scope_id)
    }

    pub fn authorize(
        &self,
        request: &AuthorityRequest,
    ) -> Result<AuthorityDecision, AuthorityError> {
        self.state.authorize(request)
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn inject_transient_failure(&self, point: NativeAuthorityFailurePoint) {
        self.state.inject_transient_failure(point);
    }
}
