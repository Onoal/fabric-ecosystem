use std::sync::Arc;

use fabric_core::{ContractId, ContractKey};
use fabric_resource::ResourceId;
use fabric_resource_authority::{ActorRef, AuthorityScopeId};

use crate::error::SecretsError;
use crate::model::{
    AuthorizedSecretRef, MaterializedSecret, SecretMaterial, SecretRef, SecretVersion, StoredSecret,
};
use crate::native::module::SharedSecretsState;
#[cfg(test)]
use crate::native::persistence::PersistenceFailpoint;

const SECRETS_CONTRACT_ID: &str = "fabric.resource.secrets";

pub fn secrets_resource_id() -> ResourceId {
    ResourceId::new("secrets").expect("static secrets resource id")
}

pub fn secrets_contract_id() -> ContractId {
    ContractId::new(SECRETS_CONTRACT_ID).expect("static secrets contract id")
}

pub fn secrets_contract_key() -> ContractKey<SecretsContract> {
    ContractKey::provisional(secrets_contract_id())
}

#[derive(Clone)]
pub struct SecretsContract {
    state: Arc<SharedSecretsState>,
}

impl SecretsContract {
    pub(crate) fn new(state: Arc<SharedSecretsState>) -> Self {
        Self { state }
    }

    pub fn create(
        &self,
        actor: &ActorRef,
        scope_id: &AuthorityScopeId,
        value: SecretMaterial,
    ) -> Result<StoredSecret, SecretsError> {
        self.state.create(actor, scope_id, value)
    }

    pub fn materialize(
        &self,
        actor: &ActorRef,
        secret: &SecretRef,
    ) -> Result<MaterializedSecret, SecretsError> {
        self.state.materialize(actor, secret)
    }

    pub fn materialize_authorized(
        &self,
        authorized: &AuthorizedSecretRef,
    ) -> Result<MaterializedSecret, SecretsError> {
        self.state
            .materialize(authorized.actor(), authorized.secret())
    }

    pub fn rotate(
        &self,
        actor: &ActorRef,
        secret: &SecretRef,
        value: SecretMaterial,
    ) -> Result<SecretVersion, SecretsError> {
        self.state.rotate(actor, secret, value)
    }

    pub fn delete(&self, actor: &ActorRef, secret: &SecretRef) -> Result<(), SecretsError> {
        self.state.delete(actor, secret)
    }

    #[cfg(test)]
    pub(crate) fn inject_failpoint(
        &self,
        failpoint: PersistenceFailpoint,
    ) -> Result<(), SecretsError> {
        self.state.inject_failpoint(failpoint)
    }
}
