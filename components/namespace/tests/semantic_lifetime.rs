use std::sync::{Arc, Mutex};

use fabric_component::{
    Component, ComponentId, ComponentRegistry, ComponentRuntime, ComponentRuntimeModule,
    ComponentRuntimeService, ComponentRuntimeStatus,
};
use fabric_component_namespace::{Namespace, NamespaceError, NamespaceModule, NamespaceName};
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    Instance, InstanceId, Module, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};

#[derive(Clone)]
struct StaticComponentRuntimeService {
    instance_id: InstanceId,
}

impl ComponentRuntimeService for StaticComponentRuntimeService {
    fn instance_id(&self) -> InstanceId {
        self.instance_id.clone()
    }

    fn current_instance_id(&self) -> Result<InstanceId, fabric_component::ComponentError> {
        Ok(self.instance_id())
    }

    fn current_status(&self) -> ComponentRuntimeStatus {
        ComponentRuntimeStatus::new(
            self.instance_id(),
            None,
            fabric_component::ComponentRuntimeLifecycle::Stopped,
            Health::Unavailable,
        )
    }

    fn current_lifecycle(&self) -> fabric_component::ComponentRuntimeLifecycle {
        fabric_component::ComponentRuntimeLifecycle::Ready
    }

    fn current_health(&self) -> Health {
        Health::Healthy
    }
}

#[derive(Clone)]
struct NamespaceCapture {
    runtime: Arc<ComponentRuntime>,
    registry: Arc<ComponentRegistry>,
    namespace: Arc<Namespace>,
}

type Capture = Arc<Mutex<Option<NamespaceCapture>>>;

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

#[derive(Clone)]
struct NamespaceCaptureModule {
    module_id: ModuleId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registry_requirement: ContractRequirement<ComponentRegistry>,
    namespace_requirement: ContractRequirement<Namespace>,
    capture: Capture,
}

impl NamespaceCaptureModule {
    fn new(capture: Capture) -> Self {
        Self {
            module_id: ModuleId::new("fabric.component.namespace.capture").expect("module id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry_requirement: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            namespace_requirement: ContractRequirement::provisional(
                fabric_component_namespace::namespace_contract_id(),
            ),
            capture,
        }
    }
}

impl ModuleRuntime for NamespaceCaptureModule {
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
            self.namespace_requirement.id().clone(),
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
        let namespace = bindings
            .resolve(&self.namespace_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.lock().expect("capture lock") = Some(NamespaceCapture {
            runtime,
            registry,
            namespace,
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

fn started_namespace(instance_id: &str) -> (Instance, NamespaceModule, NamespaceCapture) {
    let capture = Arc::new(Mutex::new(None));
    let namespace_module = NamespaceModule::new();
    let block =
        BlockBuilder::new(BlockId::new("fabric.component.namespace.block").expect("block id"))
            .register_module(ComponentRuntimeModule::new())
            .register_module(SharedNamespaceDefinition::new(
                namespace_module.shared_clone(),
            ))
            .register_module(NamespaceCaptureModule::new(Arc::clone(&capture)))
            .build();
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.component.namespace.composition").expect("composition id"),
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
        .expect("captured namespace rails");
    (instance, namespace_module, capture)
}

fn bind_component(runtime: &ComponentRuntime, instance_id: &str, component_id: &str) -> Component {
    let expected = InstanceId::new(instance_id).expect("instance id");
    assert_eq!(runtime.instance_id(), expected);
    Component::bind(
        ComponentId::new(component_id).expect("component id"),
        &ComponentRuntime::new(Arc::new(StaticComponentRuntimeService {
            instance_id: expected,
        })),
    )
}

fn participate(rails: &NamespaceCapture, component_id: &str) -> Component {
    let component = bind_component(
        &rails.runtime,
        rails.runtime.instance_id().as_str(),
        component_id,
    );
    let participation = rails
        .registry
        .register(component.clone(), Health::Healthy)
        .expect("register component")
        .participation()
        .clone();
    rails
        .registry
        .activate(&participation)
        .expect("activate component");
    component
}

#[test]
fn namespace_claims_survive_stop_and_restart() {
    let (_instance, mut namespace_module, rails) = started_namespace("fabric.main");
    let owner = participate(&rails, "photos");
    let name = NamespaceName::new("photos").expect("namespace name");
    let claim = rails
        .namespace
        .claim(owner.clone(), name.clone())
        .expect("claim namespace");

    namespace_module.stop();

    let error = rails
        .namespace
        .allocation(&name)
        .expect_err("stopped namespace must be unavailable");
    assert!(matches!(error, NamespaceError::Unavailable));

    namespace_module.start().expect("restart namespace module");

    let restored = rails
        .namespace
        .allocation(&name)
        .expect("claim must survive restart");
    assert_eq!(restored, claim);
}

#[test]
fn namespace_claim_owner_is_stable_component_identity_not_participation() {
    let (mut instance, _namespace_module, rails) = started_namespace("fabric.main");
    let stable_owner = participate(&rails, "photos");
    let name = NamespaceName::new("photos").expect("namespace name");

    let claim = rails
        .namespace
        .claim(stable_owner.clone(), name.clone())
        .expect("claim namespace");
    let first_participation = rails
        .registry
        .component(stable_owner.component_id())
        .expect("owner status")
        .participation()
        .clone();
    rails
        .registry
        .unregister(&first_participation)
        .expect("unregister owner");
    let same_component_new_handle = participate(&rails, "photos");

    let restored = rails
        .namespace
        .allocation(&name)
        .expect("lookup allocation");
    assert_eq!(restored, claim);
    assert_eq!(restored.owner(), &same_component_new_handle);
    instance.stop();
}
