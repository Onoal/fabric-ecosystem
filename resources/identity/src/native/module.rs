use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fabric_core::ContractRequirement;
use fabric_core::{Health, Module, ModuleContract, ModuleId, ModuleRuntime};
use fabric_resource_registry::{
    ResourceDescriptor, ResourceRegistry, resource_registry_contract_id,
};

use crate::contract::{IdentityContract, identity_contract_key, identity_resource_id};
use crate::error::IdentityError;
use crate::model::{Principal, PrincipalId};
use crate::native::persistence::IdentityDatabase;

#[derive(Clone, Debug)]
pub struct NativeIdentityConfig {
    pub database_path: PathBuf,
}

pub struct NativeIdentity {
    module_id: ModuleId,
    resource_registry_requirement: ContractRequirement<ResourceRegistry>,
    resource_registry: Option<ResourceRegistry>,
    config: NativeIdentityConfig,
    shared: Arc<SharedIdentityState>,
}

pub struct SharedIdentityState {
    inner: Mutex<IdentityModuleState>,
}

struct IdentityModuleState {
    health: Health,
    runtime: Option<IdentityDatabase>,
    started: bool,
}

impl NativeIdentity {
    pub fn new(config: NativeIdentityConfig) -> Self {
        Self {
            module_id: ModuleId::new("fabric.resource.identity.native")
                .expect("static native identity module id"),
            resource_registry_requirement: ContractRequirement::provisional(
                resource_registry_contract_id(),
            ),
            resource_registry: None,
            config,
            shared: Arc::new(SharedIdentityState {
                inner: Mutex::new(IdentityModuleState {
                    health: Health::Unavailable,
                    runtime: None,
                    started: false,
                }),
            }),
        }
    }
}

impl ModuleRuntime for NativeIdentity {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![identity_contract_key().declaration()]
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.resource_registry_requirement.declaration().clone()]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, fabric_core::ModuleError> {
        Ok(vec![ModuleContract::new(
            &identity_contract_key(),
            Arc::new(IdentityContract::new(Arc::clone(&self.shared))),
        )])
    }

    fn bind(
        &mut self,
        bindings: &fabric_core::ModuleBindings,
    ) -> Result<(), fabric_core::ModuleError> {
        self.resource_registry = bindings
            .resolve_optional(&self.resource_registry_requirement)
            .map_err(|error| fabric_core::ModuleError::new(error.to_string()))?
            .as_deref()
            .cloned();
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), fabric_core::ModuleError> {
        let runtime =
            IdentityDatabase::open(&self.config.database_path).map_err(to_module_error)?;
        let mut state = self.shared.inner.lock().expect("identity state lock");
        state.runtime = Some(runtime);
        state.started = false;
        state.health = Health::Unavailable;
        Ok(())
    }

    fn start(&mut self) -> Result<(), fabric_core::ModuleError> {
        if let Some(shell) = &self.resource_registry {
            shell
                .register(self, ResourceDescriptor::new(identity_resource_id()))
                .map_err(|error| fabric_core::ModuleError::new(error.to_string()))?;
        }
        let mut state = self.shared.inner.lock().expect("identity state lock");
        if state.runtime.is_none() {
            return Err(fabric_core::ModuleError::new(
                "native identity runtime is not initialized",
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
        let mut state = self.shared.inner.lock().expect("identity state lock");
        state.runtime = None;
        state.started = false;
        state.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.shared
            .inner
            .lock()
            .expect("identity state lock")
            .health
    }
}

impl Module for NativeIdentity {
    fn materialize(&self) -> Box<dyn ModuleRuntime> {
        Box::new(Self::new(self.config.clone()))
    }
}

impl SharedIdentityState {
    pub(crate) fn create_principal(&self) -> Result<Principal, IdentityError> {
        self.with_runtime_mut(IdentityDatabase::create_principal)
    }

    pub(crate) fn get_principal(
        &self,
        principal_id: &PrincipalId,
    ) -> Result<Option<Principal>, IdentityError> {
        self.with_runtime(|runtime| runtime.get_principal(principal_id))
    }

    fn with_runtime<T>(
        &self,
        action: impl FnOnce(&IdentityDatabase) -> Result<T, IdentityError>,
    ) -> Result<T, IdentityError> {
        let state = self.inner.lock().expect("identity state lock");
        if !state.started {
            return Err(IdentityError::Unavailable);
        }
        let Some(runtime) = state.runtime.as_ref() else {
            return Err(IdentityError::Unavailable);
        };
        action(runtime)
    }

    fn with_runtime_mut<T>(
        &self,
        action: impl FnOnce(&mut IdentityDatabase) -> Result<T, IdentityError>,
    ) -> Result<T, IdentityError> {
        let mut state = self.inner.lock().expect("identity state lock");
        if !state.started {
            return Err(IdentityError::Unavailable);
        }
        let Some(runtime) = state.runtime.as_mut() else {
            return Err(IdentityError::Unavailable);
        };
        action(runtime)
    }
}

fn to_module_error(error: IdentityError) -> fabric_core::ModuleError {
    fabric_core::ModuleError::new(error.to_string())
}
