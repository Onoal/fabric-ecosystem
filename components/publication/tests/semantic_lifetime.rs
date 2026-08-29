use std::sync::{Arc, Mutex};

use fabric_component::{
    Component, ComponentId, ComponentRegistry, ComponentRuntime, ComponentRuntimeModule, SurfaceId,
    SurfaceRegistry,
};
use fabric_component_namespace::{Namespace, NamespaceModule, NamespaceName};
use fabric_component_publication::{PublicationContract, PublicationError, PublicationModule};
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    Instance, InstanceId, Module, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};

#[derive(Clone)]
struct PublicationCapture {
    runtime: Arc<ComponentRuntime>,
    registry: Arc<ComponentRegistry>,
    surfaces: Arc<SurfaceRegistry>,
    namespace: Arc<Namespace>,
    publications: Arc<PublicationContract>,
}

type Capture = Arc<Mutex<Option<PublicationCapture>>>;

struct SharedNamespaceDefinition {
    module: NamespaceModule,
}

impl SharedNamespaceDefinition {
    fn new(module: NamespaceModule) -> Self {
        Self { module }
    }
}

impl Module for SharedNamespaceDefinition {
    fn materialize(&self) -> Box<dyn ModuleRuntime> {
        Box::new(self.module.shared_clone())
    }
}

struct SharedPublicationDefinition {
    module: PublicationModule,
}

impl SharedPublicationDefinition {
    fn new(module: PublicationModule) -> Self {
        Self { module }
    }
}

impl Module for SharedPublicationDefinition {
    fn materialize(&self) -> Box<dyn ModuleRuntime> {
        Box::new(self.module.shared_clone())
    }
}

#[derive(Clone)]
struct PublicationCaptureModule {
    module_id: ModuleId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registry_requirement: ContractRequirement<ComponentRegistry>,
    surface_requirement: ContractRequirement<SurfaceRegistry>,
    namespace_requirement: ContractRequirement<Namespace>,
    publication_requirement: ContractRequirement<PublicationContract>,
    capture: Capture,
}

impl PublicationCaptureModule {
    fn new(capture: Capture) -> Self {
        Self {
            module_id: ModuleId::new("fabric.component.publication.capture").expect("module id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry_requirement: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            surface_requirement: ContractRequirement::provisional(
                fabric_component::surface_contract_id(),
            ),
            namespace_requirement: ContractRequirement::provisional(
                fabric_component_namespace::namespace_contract_id(),
            ),
            publication_requirement: ContractRequirement::provisional(
                fabric_component_publication::publication_contract_id(),
            ),
            capture,
        }
    }
}

impl ModuleRuntime for PublicationCaptureModule {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        Vec::new()
            .into_iter()
            .map(fabric_core::ProvidedContractDeclaration::provisional)
            .collect()
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![
            self.runtime_requirement.id().clone(),
            self.registry_requirement.id().clone(),
            self.surface_requirement.id().clone(),
            self.namespace_requirement.id().clone(),
            self.publication_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let runtime = bindings
            .resolve(&self.runtime_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let registry = bindings
            .resolve(&self.registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let surfaces = bindings
            .resolve(&self.surface_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let namespace = bindings
            .resolve(&self.namespace_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let publications = bindings
            .resolve(&self.publication_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.lock().expect("capture lock") = Some(PublicationCapture {
            runtime,
            registry,
            surfaces,
            namespace,
            publications,
        });
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn stop(&mut self) {}

    fn health(&self) -> Health {
        Health::Healthy
    }
}

fn started_publication(
    instance_id: &str,
) -> (
    Instance,
    NamespaceModule,
    PublicationModule,
    PublicationCapture,
) {
    let capture = Arc::new(Mutex::new(None));
    let namespace_module = NamespaceModule::new();
    let publication_module = PublicationModule::new();
    let block =
        BlockBuilder::new(BlockId::new("fabric.component.publication.block").expect("block id"))
            .register_module(ComponentRuntimeModule::new())
            .register_module(SharedNamespaceDefinition::new(
                namespace_module.shared_clone(),
            ))
            .register_module(SharedPublicationDefinition::new(
                publication_module.shared_clone(),
            ))
            .register_module(PublicationCaptureModule::new(Arc::clone(&capture)))
            .build();
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.component.publication.composition").expect("composition id"),
    )
    .register_block(block)
    .build()
    .expect("build composition");
    let mut instance = composition
        .materialize(InstanceId::new(instance_id).expect("instance id"))
        .expect("materialize instance");
    instance.start().expect("start instance");
    let capture = capture
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured publication rails");
    (instance, namespace_module, publication_module, capture)
}

fn participate(capture: &PublicationCapture, component_id: &str) -> Component {
    let component = Component::bind(
        ComponentId::new(component_id).expect("component id"),
        capture.runtime.as_ref(),
    );
    let participation = capture
        .registry
        .register(component.clone(), Health::Healthy)
        .expect("register component")
        .participation()
        .clone();
    capture
        .registry
        .activate(&participation)
        .expect("activate component");
    component
}

#[test]
fn publications_survive_publication_module_stop_and_restart() {
    let (mut instance, _namespace_module, mut publication_module, capture) =
        started_publication("fabric.main");
    let owner = participate(&capture, "photos");
    let claim = capture
        .namespace
        .claim(
            owner.clone(),
            NamespaceName::new("photos").expect("namespace name"),
        )
        .expect("claim namespace");
    let surface = capture
        .surfaces
        .register(owner, SurfaceId::new("photos.main").expect("surface id"))
        .expect("register surface");
    let publication = capture
        .publications
        .publish(claim.clone(), surface.clone())
        .expect("publish surface");
    assert_eq!(
        capture
            .publications
            .publication(claim.name())
            .expect("publication exists"),
        publication
    );

    publication_module.stop();
    assert!(matches!(
        capture.publications.publication(claim.name()),
        Err(PublicationError::Unavailable)
    ));

    publication_module
        .start()
        .expect("restart publication module");
    assert_eq!(
        capture
            .publications
            .publication(claim.name())
            .expect("publication survives restart"),
        publication
    );

    instance.stop();
}
