use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fabric_component::{
    Component, ComponentId, ComponentParticipation, ComponentRegistry, ComponentRuntime,
    component_registry_contract_id, component_runtime_contract_id,
};
use fabric_core::{
    ContractRequirement, Health, Module, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};

use crate::{
    Namespace, NamespaceClaim, NamespaceError, NamespaceName, NamespaceService,
    namespace_contract_key,
};

pub struct NamespaceModule {
    module_id: ModuleId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registry_requirement: ContractRequirement<ComponentRegistry>,
    shared: Arc<SharedNamespaceState>,
}

struct SharedNamespaceState {
    inner: Mutex<NamespaceModuleState>,
}

struct NamespaceModuleState {
    health: Health,
    runtime: Option<ComponentRuntime>,
    registry: Option<ComponentRegistry>,
    component: Option<Component>,
    participation: Option<ComponentParticipation>,
    started: bool,
    names: BTreeMap<NamespaceName, NamespaceClaim>,
}

impl NamespaceModule {
    pub fn new() -> Self {
        Self {
            module_id: ModuleId::new("fabric.component.namespace.module")
                .expect("static namespace module id"),
            runtime_requirement: ContractRequirement::provisional(component_runtime_contract_id()),
            registry_requirement: ContractRequirement::provisional(component_registry_contract_id()),
            shared: Arc::new(SharedNamespaceState {
                inner: Mutex::new(NamespaceModuleState {
                    health: Health::Unavailable,
                    runtime: None,
                    registry: None,
                    component: None,
                    participation: None,
                    started: false,
                    names: BTreeMap::new(),
                }),
            }),
        }
    }

    pub fn shared_clone(&self) -> Self {
        Self {
            module_id: self.module_id.clone(),
            runtime_requirement: self.runtime_requirement.clone(),
            registry_requirement: self.registry_requirement.clone(),
            shared: Arc::clone(&self.shared),
        }
    }
}

impl Default for NamespaceModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleRuntime for NamespaceModule {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![namespace_contract_key().declaration()]
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![
            self.runtime_requirement.declaration().clone(),
            self.registry_requirement.declaration().clone(),
        ]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        let namespace: Arc<dyn NamespaceService> = self.shared.clone();
        Ok(vec![ModuleContract::new(
            &namespace_contract_key(),
            Arc::new(Namespace::new(namespace)),
        )])
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let runtime = bindings
            .resolve(&self.runtime_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let registry = bindings
            .resolve(&self.registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let component_id =
            ComponentId::new("namespace").map_err(|error| ModuleError::new(error.to_string()))?;
        let component = Component::bind(component_id, runtime.as_ref());

        let mut state = self.shared.inner.lock().expect("namespace state lock");
        state.runtime = Some(runtime.as_ref().clone());
        state.registry = Some(registry.as_ref().clone());
        state.component = Some(component);
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        let mut state = self.shared.inner.lock().expect("namespace state lock");
        state.health = Health::Unavailable;
        state.started = false;
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        let (registry, component) = {
            let state = self.shared.inner.lock().expect("namespace state lock");
            let registry = state
                .registry
                .clone()
                .ok_or_else(|| ModuleError::new("namespace registry dependency is not bound"))?;
            let component = state
                .component
                .clone()
                .ok_or_else(|| ModuleError::new("namespace component is not bound"))?;
            (registry, component)
        };

        let status = registry
            .register(component, Health::Healthy)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let status = registry
            .activate(status.participation())
            .map_err(|error| ModuleError::new(error.to_string()))?;

        let mut state = self.shared.inner.lock().expect("namespace state lock");
        state.participation = Some(status.participation().clone());
        state.started = true;
        state.health = status.health();
        Ok(())
    }

    fn stop(&mut self) {
        let (registry, participation) = {
            let mut state = self.shared.inner.lock().expect("namespace state lock");
            state.started = false;
            state.health = Health::Unavailable;
            (state.registry.clone(), state.participation.take())
        };
        if let (Some(registry), Some(participation)) = (registry, participation) {
            let _ = registry.unregister(&participation);
        }
    }

    fn health(&self) -> Health {
        self.shared
            .inner
            .lock()
            .expect("namespace state lock")
            .health
    }
}

impl Module for NamespaceModule {
    fn materialize(&self) -> Box<dyn ModuleRuntime> {
        Box::new(Self::new())
    }
}

impl NamespaceService for SharedNamespaceState {
    fn claim(
        &self,
        owner: Component,
        name: NamespaceName,
    ) -> Result<NamespaceClaim, NamespaceError> {
        let mut state = self.inner.lock().expect("namespace state lock");
        if !state.started {
            return Err(NamespaceError::Unavailable);
        }
        let runtime = state.runtime.as_ref().ok_or(NamespaceError::Unavailable)?;
        if owner.instance_id() != &runtime.instance_id() {
            return Err(NamespaceError::NamespaceNameOwnerInstanceMismatch {
                name,
                owner_instance_id: owner.instance_id().clone(),
                instance_id: runtime.instance_id(),
            });
        }
        let registry = state.registry.as_ref().ok_or(NamespaceError::Unavailable)?;
        registry
            .component(owner.component_id())
            .map_err(NamespaceError::from)?;
        if state.names.contains_key(&name) {
            return Err(NamespaceError::NamespaceNameAlreadyAllocated(name));
        }
        let claim = NamespaceClaim::new(owner, name.clone());
        state.names.insert(name, claim.clone());
        Ok(claim)
    }

    fn allocation(&self, name: &NamespaceName) -> Result<NamespaceClaim, NamespaceError> {
        let state = self.inner.lock().expect("namespace state lock");
        if !state.started {
            return Err(NamespaceError::Unavailable);
        }
        state
            .names
            .get(name)
            .cloned()
            .ok_or_else(|| NamespaceError::UnknownNamespaceName(name.clone()))
    }
}
