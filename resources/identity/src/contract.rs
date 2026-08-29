use std::sync::Arc;

use fabric_core::{ContractId, ContractKey};
use fabric_resource::ResourceId;

use crate::error::IdentityError;
use crate::model::{Principal, PrincipalId};
use crate::native::module::SharedIdentityState;

const IDENTITY_CONTRACT_ID: &str = "fabric.resource.identity";

pub fn identity_resource_id() -> ResourceId {
    ResourceId::new("identity").expect("static identity resource id")
}

pub fn identity_contract_id() -> ContractId {
    ContractId::new(IDENTITY_CONTRACT_ID).expect("static identity contract id")
}

pub fn identity_contract_key() -> ContractKey<IdentityContract> {
    ContractKey::provisional(identity_contract_id())
}

#[derive(Clone)]
pub struct IdentityContract {
    state: Arc<SharedIdentityState>,
}

impl IdentityContract {
    pub(crate) fn new(state: Arc<SharedIdentityState>) -> Self {
        Self { state }
    }

    pub fn create_principal(&self) -> Result<Principal, IdentityError> {
        self.state.create_principal()
    }

    pub fn get_principal(
        &self,
        principal_id: &PrincipalId,
    ) -> Result<Option<Principal>, IdentityError> {
        self.state.get_principal(principal_id)
    }
}
