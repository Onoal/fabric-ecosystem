use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fabric_core::ContractRequirement;
use fabric_core::{Health, Module, ModuleBindings, ModuleContract, ModuleId, ModuleRuntime};
use fabric_resource_registry::{
    ResourceDescriptor, ResourceRegistry, resource_registry_contract_id,
};

use crate::contract::{AuthorityContract, authority_contract_key, authority_resource_id};
use crate::decision::AuthorityDecisionAdapter;
use crate::error::AuthorityError;
use crate::model::{
    ActionId, ActorRef, AuthorityDecision, AuthorityRequest, AuthorityScopeId, ResourceRef,
};
use crate::native::persistence::AuthorityDatabase;

#[derive(Clone, Debug)]
pub struct NativeAuthorityConfig {
    pub database_path: PathBuf,
}

pub struct NativeAuthority {
    module_id: ModuleId,
    resource_registry_requirement: ContractRequirement<ResourceRegistry>,
    resource_registry: Option<ResourceRegistry>,
    config: NativeAuthorityConfig,
    decision_adapter: Arc<dyn AuthorityDecisionAdapter>,
    shared: Arc<SharedAuthorityState>,
}

#[cfg(feature = "test-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeAuthorityFailurePoint {
    CreateScope,
    GrantScopeControl,
}

pub struct SharedAuthorityState {
    inner: Mutex<AuthorityModuleState>,
    decision_adapter: Arc<dyn AuthorityDecisionAdapter>,
}

struct AuthorityModuleState {
    health: Health,
    runtime: Option<AuthorityDatabase>,
    started: bool,
    #[cfg(feature = "test-support")]
    transient_failure: Option<NativeAuthorityFailurePoint>,
}

impl NativeAuthority {
    pub fn with_decision_adapter(
        config: NativeAuthorityConfig,
        decision_adapter: Arc<dyn AuthorityDecisionAdapter>,
    ) -> Self {
        Self {
            module_id: ModuleId::new("fabric.resource.authority.native")
                .expect("static native authority module id"),
            resource_registry_requirement: ContractRequirement::provisional(
                resource_registry_contract_id(),
            ),
            resource_registry: None,
            config,
            decision_adapter: Arc::clone(&decision_adapter),
            shared: Arc::new(SharedAuthorityState {
                inner: Mutex::new(AuthorityModuleState {
                    health: Health::Unavailable,
                    runtime: None,
                    started: false,
                    #[cfg(feature = "test-support")]
                    transient_failure: None,
                }),
                decision_adapter,
            }),
        }
    }
}

impl ModuleRuntime for NativeAuthority {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![authority_contract_key().declaration()]
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.resource_registry_requirement.declaration().clone()]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, fabric_core::ModuleError> {
        Ok(vec![ModuleContract::new(
            &authority_contract_key(),
            Arc::new(AuthorityContract::new(Arc::clone(&self.shared))),
        )])
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), fabric_core::ModuleError> {
        self.resource_registry = bindings
            .resolve_optional(&self.resource_registry_requirement)
            .map_err(|error| fabric_core::ModuleError::new(error.to_string()))?
            .as_deref()
            .cloned();
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), fabric_core::ModuleError> {
        let runtime =
            AuthorityDatabase::open(&self.config.database_path).map_err(to_module_error)?;
        let mut state = self.shared.inner.lock().expect("authority state lock");
        state.runtime = Some(runtime);
        state.started = false;
        state.health = Health::Unavailable;
        Ok(())
    }

    fn start(&mut self) -> Result<(), fabric_core::ModuleError> {
        if let Some(shell) = &self.resource_registry {
            shell
                .register(self, ResourceDescriptor::new(authority_resource_id()))
                .map_err(|error| fabric_core::ModuleError::new(error.to_string()))?;
        }
        let mut state = self.shared.inner.lock().expect("authority state lock");
        if state.runtime.is_none() {
            return Err(fabric_core::ModuleError::new(
                "native authority runtime is not initialized",
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
        let mut state = self.shared.inner.lock().expect("authority state lock");
        state.runtime = None;
        state.started = false;
        state.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.shared
            .inner
            .lock()
            .expect("authority state lock")
            .health
    }
}

impl Module for NativeAuthority {
    fn materialize(&self) -> Box<dyn ModuleRuntime> {
        Box::new(Self::with_decision_adapter(
            self.config.clone(),
            Arc::clone(&self.decision_adapter),
        ))
    }
}

impl SharedAuthorityState {
    pub(crate) fn create_scope(&self) -> Result<AuthorityScopeId, AuthorityError> {
        #[cfg(feature = "test-support")]
        self.fail_if_requested(NativeAuthorityFailurePoint::CreateScope)?;
        self.with_runtime_mut(|runtime| runtime.create_scope())
    }

    pub(crate) fn grant_action(
        &self,
        actor: &ActorRef,
        action_id: &ActionId,
        resource: &ResourceRef,
    ) -> Result<(), AuthorityError> {
        self.with_runtime_mut(|runtime| runtime.grant_action(actor, action_id, resource))
    }

    pub(crate) fn grant_scope_control(
        &self,
        actor: &ActorRef,
        scope_id: &AuthorityScopeId,
    ) -> Result<(), AuthorityError> {
        #[cfg(feature = "test-support")]
        self.fail_if_requested(NativeAuthorityFailurePoint::GrantScopeControl)?;
        self.with_runtime_mut(|runtime| runtime.grant_scope_control(actor, scope_id))
    }

    pub(crate) fn authorize(
        &self,
        request: &AuthorityRequest,
    ) -> Result<AuthorityDecision, AuthorityError> {
        self.with_runtime(|runtime| runtime.authorize(request, self.decision_adapter.as_ref()))
    }

    fn with_runtime<T>(
        &self,
        action: impl FnOnce(&AuthorityDatabase) -> Result<T, AuthorityError>,
    ) -> Result<T, AuthorityError> {
        let state = self.inner.lock().expect("authority state lock");
        if !state.started {
            return Err(AuthorityError::Unavailable);
        }
        let runtime = state.runtime.as_ref().ok_or(AuthorityError::Unavailable)?;
        action(runtime)
    }

    fn with_runtime_mut<T>(
        &self,
        action: impl FnOnce(&mut AuthorityDatabase) -> Result<T, AuthorityError>,
    ) -> Result<T, AuthorityError> {
        let mut state = self.inner.lock().expect("authority state lock");
        if !state.started {
            return Err(AuthorityError::Unavailable);
        }
        let runtime = state.runtime.as_mut().ok_or(AuthorityError::Unavailable)?;
        action(runtime)
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn inject_transient_failure(&self, point: NativeAuthorityFailurePoint) {
        self.inner
            .lock()
            .expect("authority state lock")
            .transient_failure = Some(point);
    }

    #[cfg(feature = "test-support")]
    fn fail_if_requested(&self, point: NativeAuthorityFailurePoint) -> Result<(), AuthorityError> {
        let mut state = self.inner.lock().expect("authority state lock");
        if state.transient_failure == Some(point) {
            state.transient_failure = None;
            return Err(AuthorityError::Persistence {
                message: format!("injected transient authority failure at {point:?}"),
            });
        }
        Ok(())
    }
}

fn to_module_error(error: AuthorityError) -> fabric_core::ModuleError {
    fabric_core::ModuleError::new(error.to_string())
}
