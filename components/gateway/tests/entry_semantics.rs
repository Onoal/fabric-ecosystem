use std::sync::{Arc, Mutex};

use fabric_component::{
    Component, ComponentId, ComponentRegistry, ComponentRuntime, ComponentRuntimeModule, SurfaceId,
    SurfaceRegistry,
};
use fabric_component_gateway::{Gateway, GatewayEntry, GatewayModule};
use fabric_component_namespace::{Namespace, NamespaceModule, NamespaceName};
use fabric_component_publication::{PublicationContract, PublicationModule};
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    Instance, InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
};

type CapturedGatewayRails = Arc<Mutex<Option<GatewayRailsCapture>>>;

#[derive(Clone)]
struct GatewayRailsCapture {
    instance_id: InstanceId,
    runtime: Arc<ComponentRuntime>,
    registry: Arc<ComponentRegistry>,
    gateway: Arc<Gateway>,
    surfaces: Arc<SurfaceRegistry>,
    namespace: Arc<Namespace>,
    publications: Arc<PublicationContract>,
}

#[derive(Clone)]
struct GatewayRailsCaptureModule {
    module_id: ModuleId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registry_requirement: ContractRequirement<ComponentRegistry>,
    gateway_requirement: ContractRequirement<Gateway>,
    surface_requirement: ContractRequirement<SurfaceRegistry>,
    namespace_requirement: ContractRequirement<Namespace>,
    publication_requirement: ContractRequirement<PublicationContract>,
    capture: CapturedGatewayRails,
}

impl GatewayRailsCaptureModule {
    fn new(capture: CapturedGatewayRails) -> Self {
        Self {
            module_id: ModuleId::new("gateway.entry.capture").expect("module id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry_requirement: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            gateway_requirement: ContractRequirement::provisional(
                fabric_component_gateway::gateway_contract_id(),
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

impl ModuleRuntime for GatewayRailsCaptureModule {
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
            self.gateway_requirement.id().clone(),
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
        let gateway = bindings
            .resolve(&self.gateway_requirement)
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
        *self.capture.lock().expect("gateway rails capture lock") = Some(GatewayRailsCapture {
            instance_id: runtime.instance_id(),
            runtime,
            registry,
            gateway,
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

fn materialize_instance(instance_id: &str, composition: fabric_core::Composition) -> Instance {
    composition
        .materialize(InstanceId::new(instance_id).expect("instance id"))
        .expect("materialize composition")
}

fn fixture() -> (Instance, GatewayRailsCapture) {
    let capture = Arc::new(Mutex::new(None));
    let composition = CompositionBuilder::new(
        CompositionId::new("gateway.entry.semantics").expect("composition"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("component.core").expect("block"))
            .register_module(ComponentRuntimeModule::new())
            .register_module(NamespaceModule::new())
            .register_module(PublicationModule::new())
            .register_module(GatewayModule::new())
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("gateway.capture").expect("block"))
            .register_module(GatewayRailsCaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");

    let mut instance = materialize_instance("component.alpha", composition);
    instance.start().expect("start");
    let captured = capture
        .lock()
        .expect("gateway rails capture lock")
        .clone()
        .expect("captured gateway rails");
    (instance, captured)
}

fn participate_component(rails: &GatewayRailsCapture, component_id: &str) -> Component {
    let component = Component::bind(
        ComponentId::new(component_id).expect("component id"),
        rails.runtime.as_ref(),
    );
    let status = rails
        .registry
        .register(component.clone(), Health::Healthy)
        .expect("register component");
    rails
        .registry
        .activate(status.participation())
        .expect("activate component");
    component
}

fn publish_entry(
    rails: &GatewayRailsCapture,
    owner: Component,
    surface_id: &str,
    name: &str,
) -> GatewayEntry {
    let surface = rails
        .surfaces
        .register(
            owner.clone(),
            SurfaceId::new(surface_id).expect("surface id"),
        )
        .expect("register surface");
    let claim = rails
        .namespace
        .claim(owner, NamespaceName::new(name).expect("namespace name"))
        .expect("claim name");
    let publication = rails
        .publications
        .publish(claim, surface.clone())
        .expect("publish");
    let entry = rails
        .gateway
        .entry(&NamespaceName::new(name).expect("namespace name"))
        .expect("gateway entry");
    assert_eq!(entry.publication(), &publication);
    assert_eq!(entry.surface(), &surface);
    entry
}

#[test]
fn gateway_resolves_published_entries() {
    let (instance, rails) = fixture();
    let component = participate_component(&rails, "component.notes");
    let entry = publish_entry(&rails, component, "notes.main", "notes");

    assert_eq!(
        entry.publication().claim().owner().instance_id(),
        &rails.instance_id
    );
    assert_eq!(
        entry.publication().claim().owner().instance_id(),
        instance.instance_id()
    );
    assert_eq!(entry.name().as_str(), "notes");
}

#[test]
fn gateway_lists_published_entries_in_name_order() {
    let (_instance, rails) = fixture();
    let component = participate_component(&rails, "component.apps");

    publish_entry(&rails, component.clone(), "zeta.main", "zeta");
    publish_entry(&rails, component, "alpha.main", "alpha");

    let names: Vec<String> = rails
        .gateway
        .entries()
        .into_iter()
        .map(|entry| entry.name().as_str().to_owned())
        .collect();

    assert_eq!(names, vec!["alpha".to_owned(), "zeta".to_owned()]);
}
