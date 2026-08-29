use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fabric_component::{
    Component, ComponentId, ComponentParticipation, ComponentRegistry, ComponentRuntime, Surface,
    SurfaceRegistry, component_registry_contract_id, component_runtime_contract_id,
    surface_contract_id,
};
use fabric_component_namespace::{Namespace, NamespaceName, namespace_contract_id};
use fabric_core::{
    ContractRequirement, Health, Module, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};

use crate::{
    Publication, PublicationContract, PublicationError, PublicationService,
    publication_contract_key,
};

pub struct PublicationModule {
    module_id: ModuleId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registry_requirement: ContractRequirement<ComponentRegistry>,
    namespace_requirement: ContractRequirement<Namespace>,
    surface_requirement: ContractRequirement<SurfaceRegistry>,
    shared: Arc<SharedPublicationState>,
}

struct SharedPublicationState {
    inner: Mutex<PublicationModuleState>,
}

struct PublicationModuleState {
    health: Health,
    runtime: Option<ComponentRuntime>,
    registry: Option<ComponentRegistry>,
    namespace: Option<Namespace>,
    surfaces: Option<SurfaceRegistry>,
    component: Option<Component>,
    participation: Option<ComponentParticipation>,
    started: bool,
    publications: BTreeMap<NamespaceName, Publication>,
}

impl PublicationModule {
    pub fn new() -> Self {
        Self {
            module_id: ModuleId::new("fabric.component.publication.module")
                .expect("static publication module id"),
            runtime_requirement: ContractRequirement::provisional(component_runtime_contract_id()),
            registry_requirement: ContractRequirement::provisional(component_registry_contract_id()),
            namespace_requirement: ContractRequirement::provisional(namespace_contract_id()),
            surface_requirement: ContractRequirement::provisional(surface_contract_id()),
            shared: Arc::new(SharedPublicationState {
                inner: Mutex::new(PublicationModuleState {
                    health: Health::Unavailable,
                    runtime: None,
                    registry: None,
                    namespace: None,
                    surfaces: None,
                    component: None,
                    participation: None,
                    started: false,
                    publications: BTreeMap::new(),
                }),
            }),
        }
    }

    pub fn shared_clone(&self) -> Self {
        Self {
            module_id: self.module_id.clone(),
            runtime_requirement: self.runtime_requirement.clone(),
            registry_requirement: self.registry_requirement.clone(),
            namespace_requirement: self.namespace_requirement.clone(),
            surface_requirement: self.surface_requirement.clone(),
            shared: Arc::clone(&self.shared),
        }
    }
}

impl Default for PublicationModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleRuntime for PublicationModule {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![publication_contract_key().declaration()]
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![
            self.runtime_requirement.declaration().clone(),
            self.registry_requirement.declaration().clone(),
            self.namespace_requirement.declaration().clone(),
            self.surface_requirement.declaration().clone(),
        ]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        let publication: Arc<dyn PublicationService> = self.shared.clone();
        Ok(vec![ModuleContract::new(
            &publication_contract_key(),
            Arc::new(PublicationContract::new(publication)),
        )])
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let runtime = bindings
            .resolve(&self.runtime_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let registry = bindings
            .resolve(&self.registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let namespace = bindings
            .resolve(&self.namespace_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let surfaces = bindings
            .resolve(&self.surface_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let component_id =
            ComponentId::new("publication").map_err(|error| ModuleError::new(error.to_string()))?;
        let component = Component::bind(component_id, runtime.as_ref());

        let mut state = self.shared.inner.lock().expect("publication state lock");
        state.runtime = Some(runtime.as_ref().clone());
        state.registry = Some(registry.as_ref().clone());
        state.namespace = Some(namespace.as_ref().clone());
        state.surfaces = Some(surfaces.as_ref().clone());
        state.component = Some(component);
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        let mut state = self.shared.inner.lock().expect("publication state lock");
        state.health = Health::Unavailable;
        state.started = false;
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        let (registry, component) = {
            let state = self.shared.inner.lock().expect("publication state lock");
            let registry = state
                .registry
                .clone()
                .ok_or_else(|| ModuleError::new("publication registry dependency is not bound"))?;
            let component = state
                .component
                .clone()
                .ok_or_else(|| ModuleError::new("publication component is not bound"))?;
            (registry, component)
        };

        let status = registry
            .register(component, Health::Healthy)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let status = registry
            .activate(status.participation())
            .map_err(|error| ModuleError::new(error.to_string()))?;

        let mut state = self.shared.inner.lock().expect("publication state lock");
        state.participation = Some(status.participation().clone());
        state.started = true;
        state.health = status.health();
        Ok(())
    }

    fn stop(&mut self) {
        let (registry, participation) = {
            let mut state = self.shared.inner.lock().expect("publication state lock");
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
            .expect("publication state lock")
            .health
    }
}

impl Module for PublicationModule {
    fn materialize(&self) -> Box<dyn ModuleRuntime> {
        Box::new(Self::new())
    }
}

impl PublicationService for SharedPublicationState {
    fn publish(
        &self,
        claim: fabric_component_namespace::NamespaceClaim,
        surface: Surface,
    ) -> Result<Publication, PublicationError> {
        let mut state = self.inner.lock().expect("publication state lock");
        if !state.started {
            return Err(PublicationError::Unavailable);
        }
        if state.publications.contains_key(claim.name()) {
            return Err(PublicationError::NamespaceNameAlreadyPublished(
                claim.name().clone(),
            ));
        }
        let namespace = state
            .namespace
            .as_ref()
            .ok_or(PublicationError::Unavailable)?;
        let surfaces = state
            .surfaces
            .as_ref()
            .ok_or(PublicationError::Unavailable)?;
        let stored_claim = namespace.allocation(claim.name())?;
        if stored_claim != claim {
            return Err(PublicationError::NamespaceClaimMismatch(
                claim.name().clone(),
            ));
        }
        let stored_surface = surfaces
            .surface(surface.surface_id())
            .map_err(PublicationError::from)?;
        if stored_surface != surface {
            return Err(PublicationError::UnknownSurface(
                surface.surface_id().clone(),
            ));
        }
        let publication = Publication::new(claim.clone(), surface);
        state
            .publications
            .insert(claim.name().clone(), publication.clone());
        Ok(publication)
    }

    fn publication(&self, name: &NamespaceName) -> Result<Publication, PublicationError> {
        let state = self.inner.lock().expect("publication state lock");
        if !state.started {
            return Err(PublicationError::Unavailable);
        }
        state
            .publications
            .get(name)
            .cloned()
            .ok_or_else(|| PublicationError::UnknownPublication(name.clone()))
    }

    fn publications(&self) -> Vec<Publication> {
        self.inner
            .lock()
            .expect("publication state lock")
            .publications
            .values()
            .cloned()
            .collect()
    }
}
