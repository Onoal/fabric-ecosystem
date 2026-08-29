use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fabric_core::{
    ContractRequirement, Health, Module, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};
use fabric_resource_authority::{
    ActorRef, AuthorityContract, AuthorityScopeId, authority_contract_id,
};
use fabric_resource_registry::{
    ResourceDescriptor, ResourceRegistry, resource_registry_contract_id,
};

use crate::contract::{SecretsContract, secrets_contract_key, secrets_resource_id};
use crate::error::SecretsError;
use crate::model::{MaterializedSecret, SecretMaterial, SecretRef, SecretVersion, StoredSecret};
#[cfg(test)]
use crate::native::persistence::PersistenceFailpoint;
use crate::native::persistence::SecretsDatabase;

#[derive(Clone, Debug)]
pub struct NativeSecretsConfig {
    pub database_path: PathBuf,
}

pub struct NativeSecrets {
    module_id: ModuleId,
    authority_requirement: ContractRequirement<AuthorityContract>,
    resource_registry_requirement: ContractRequirement<ResourceRegistry>,
    resource_registry: Option<ResourceRegistry>,
    config: NativeSecretsConfig,
    shared: Arc<SharedSecretsState>,
}

pub struct SharedSecretsState {
    inner: Mutex<SecretsModuleState>,
}

struct SecretsModuleState {
    health: Health,
    runtime: Option<SecretsDatabase>,
    authority: Option<AuthorityContract>,
    started: bool,
}

impl NativeSecrets {
    pub fn new(config: NativeSecretsConfig) -> Self {
        Self {
            module_id: ModuleId::new("fabric.resource.secrets.native")
                .expect("static native secrets module id"),
            authority_requirement: ContractRequirement::provisional(authority_contract_id()),
            resource_registry_requirement: ContractRequirement::provisional(
                resource_registry_contract_id(),
            ),
            resource_registry: None,
            config,
            shared: Arc::new(SharedSecretsState {
                inner: Mutex::new(SecretsModuleState {
                    health: Health::Unavailable,
                    runtime: None,
                    authority: None,
                    started: false,
                }),
            }),
        }
    }
}

impl ModuleRuntime for NativeSecrets {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![secrets_contract_key().declaration()]
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.authority_requirement.declaration().clone()]
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.resource_registry_requirement.declaration().clone()]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(vec![ModuleContract::new(
            &secrets_contract_key(),
            Arc::new(SecretsContract::new(Arc::clone(&self.shared))),
        )])
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let authority = bindings
            .resolve(&self.authority_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        self.resource_registry = bindings
            .resolve_optional(&self.resource_registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?
            .as_deref()
            .cloned();
        self.shared
            .inner
            .lock()
            .expect("secrets state lock")
            .authority = Some(authority.as_ref().clone());
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        let runtime = SecretsDatabase::open(&self.config.database_path).map_err(to_module_error)?;
        let mut state = self.shared.inner.lock().expect("secrets state lock");
        state.runtime = Some(runtime);
        state.started = false;
        state.health = Health::Unavailable;
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        if let Some(shell) = &self.resource_registry {
            shell
                .register(self, ResourceDescriptor::new(secrets_resource_id()))
                .map_err(|error| ModuleError::new(error.to_string()))?;
        }
        let mut state = self.shared.inner.lock().expect("secrets state lock");
        if state.runtime.is_none() || state.authority.is_none() {
            return Err(ModuleError::new(
                "native secrets dependencies are not ready",
            ));
        }
        state.started = true;
        state.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(shell) = &self.resource_registry {
            let _ = shell.unregister(&self.module_id);
        }
        let mut state = self.shared.inner.lock().expect("secrets state lock");
        state.runtime = None;
        state.started = false;
        state.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.shared.inner.lock().expect("secrets state lock").health
    }
}

impl Module for NativeSecrets {
    fn materialize(&self) -> Box<dyn ModuleRuntime> {
        Box::new(Self::new(self.config.clone()))
    }
}

impl SharedSecretsState {
    pub(crate) fn create(
        &self,
        actor: &ActorRef,
        scope_id: &AuthorityScopeId,
        value: SecretMaterial,
    ) -> Result<StoredSecret, SecretsError> {
        self.with_runtime_mut(|runtime, authority| {
            runtime.create(authority, actor, scope_id, value)
        })
    }

    pub(crate) fn materialize(
        &self,
        actor: &ActorRef,
        secret: &SecretRef,
    ) -> Result<MaterializedSecret, SecretsError> {
        self.with_runtime(|runtime, authority| runtime.materialize(authority, actor, secret))
    }

    pub(crate) fn rotate(
        &self,
        actor: &ActorRef,
        secret: &SecretRef,
        value: SecretMaterial,
    ) -> Result<SecretVersion, SecretsError> {
        self.with_runtime_mut(|runtime, authority| runtime.rotate(authority, actor, secret, value))
    }

    pub(crate) fn delete(&self, actor: &ActorRef, secret: &SecretRef) -> Result<(), SecretsError> {
        self.with_runtime_mut(|runtime, authority| runtime.delete(authority, actor, secret))
    }

    #[cfg(test)]
    pub(crate) fn inject_failpoint(
        &self,
        failpoint: PersistenceFailpoint,
    ) -> Result<(), SecretsError> {
        self.with_runtime_mut(|runtime, _authority| {
            runtime.inject_failpoint(failpoint);
            Ok(())
        })
    }

    fn with_runtime<T>(
        &self,
        action: impl FnOnce(&SecretsDatabase, &AuthorityContract) -> Result<T, SecretsError>,
    ) -> Result<T, SecretsError> {
        let state = self.inner.lock().expect("secrets state lock");
        if !state.started {
            return Err(SecretsError::Unavailable);
        }
        let authority = state.authority.as_ref().ok_or(SecretsError::Unavailable)?;
        let runtime = state.runtime.as_ref().ok_or(SecretsError::Unavailable)?;
        action(runtime, authority)
    }

    fn with_runtime_mut<T>(
        &self,
        action: impl FnOnce(&mut SecretsDatabase, &AuthorityContract) -> Result<T, SecretsError>,
    ) -> Result<T, SecretsError> {
        let mut state = self.inner.lock().expect("secrets state lock");
        if !state.started {
            return Err(SecretsError::Unavailable);
        }
        let authority = state
            .authority
            .as_ref()
            .ok_or(SecretsError::Unavailable)?
            .clone();
        let runtime = state.runtime.as_mut().ok_or(SecretsError::Unavailable)?;
        action(runtime, &authority)
    }
}

fn to_module_error(error: SecretsError) -> ModuleError {
    ModuleError::new(error.to_string())
}
