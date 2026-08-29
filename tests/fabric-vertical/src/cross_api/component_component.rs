use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fabric_component::{
    Component, ComponentCommunication, ComponentContract, ComponentError, ComponentId,
    ComponentParticipation, ComponentRegistry, ComponentRequirementKind, ComponentRuntime,
    ComponentRuntimeModule, ProvidedComponentContract,
};
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractId, ContractKey,
    ContractRequirement, Health, Instance, InstanceId, ModuleBindings, ModuleContract, ModuleError,
    ModuleId, ModuleRuntime, ResolvedContract,
};

const INSTANCE_ID: &str = "fabric.fabric.vertical.cross-api.components";
const NOTES_CONTRACT_ID: &str = "fabric.test.cross-api.component-notes";
const PROVIDER_COMPONENT_ID: &str = "component.provider";
const CONSUMER_COMPONENT_ID: &str = "component.consumer";
const INTRUDER_COMPONENT_ID: &str = "component.intruder";
const PROVIDER_MODULE_ID: &str = "fabric.test.cross-api.component-notes.provider";
const FORGED_PROVIDER_MODULE_ID: &str = "fabric.test.cross-api.component-notes.forged";

#[derive(Clone)]
struct Notes;

impl Notes {
    fn reply(&self, input: &str) -> String {
        format!("notes:{input}")
    }
}

fn notes_contract_key() -> ContractKey<ProvidedComponentContract<Notes>> {
    ContractKey::provisional(ContractId::new(NOTES_CONTRACT_ID).expect("contract id"))
}

#[derive(Clone)]
struct NotesProviderModule {
    module_id: ModuleId,
    provider_component_id: ComponentId,
    claimed_provider_module: ModuleId,
    instance_id: InstanceId,
}

impl NotesProviderModule {
    fn new(module_id: &str, provider_component_id: &str, claimed_provider_module: &str) -> Self {
        Self {
            module_id: ModuleId::new(module_id).expect("module id"),
            provider_component_id: ComponentId::new(provider_component_id).expect("component id"),
            claimed_provider_module: ModuleId::new(claimed_provider_module)
                .expect("claimed provider module id"),
            instance_id: InstanceId::new("fabric.component.runtime.unbound")
                .expect("unbound instance id"),
        }
    }

    fn provider_component(&self) -> Component {
        Component::for_instance(self.provider_component_id.clone(), self.instance_id.clone())
    }
}

impl ModuleRuntime for NotesProviderModule {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![notes_contract_key().id().clone()]
            .into_iter()
            .map(fabric_core::ProvidedContractDeclaration::provisional)
            .collect()
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        Vec::new()
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(vec![ModuleContract::new(
            &notes_contract_key(),
            Arc::new(ProvidedComponentContract::new(
                self.provider_component(),
                self.claimed_provider_module.clone(),
                Arc::new(Notes),
            )),
        )])
    }

    fn bind(&mut self, _bindings: &ModuleBindings) -> Result<(), ModuleError> {
        Ok(())
    }

    fn bind_instance_context(
        &mut self,
        context: &fabric_core::InstanceRuntimeContext,
    ) -> Result<(), ModuleError> {
        self.instance_id = context.instance_id().clone();
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

#[derive(Clone)]
struct Rails {
    runtime: Arc<ComponentRuntime>,
    registry: Arc<ComponentRegistry>,
    communication: Arc<ComponentCommunication>,
    resolved_notes: ResolvedContract<ProvidedComponentContract<Notes>>,
}

type Capture = Arc<Mutex<Option<Rails>>>;

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registry_requirement: ContractRequirement<ComponentRegistry>,
    communication_requirement: ContractRequirement<ComponentCommunication>,
    notes_requirement: ContractRequirement<ProvidedComponentContract<Notes>>,
    capture: Capture,
}

impl CaptureModule {
    fn new(capture: Capture) -> Self {
        Self {
            module_id: ModuleId::new("fabric.test.cross-api.component-capture").expect("module id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry_requirement: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            communication_requirement: ContractRequirement::provisional(
                fabric_component::component_communication_contract_id(),
            ),
            notes_requirement: ContractRequirement::provisional(notes_contract_key().id().clone()),
            capture,
        }
    }
}

impl ModuleRuntime for CaptureModule {
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
            self.communication_requirement.id().clone(),
            self.notes_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        *self.capture.lock().expect("capture lock") = Some(Rails {
            runtime: bindings
                .resolve(&self.runtime_requirement)
                .map_err(module_error)?,
            registry: bindings
                .resolve(&self.registry_requirement)
                .map_err(module_error)?,
            communication: bindings
                .resolve(&self.communication_requirement)
                .map_err(module_error)?,
            resolved_notes: bindings
                .resolve_with_provider(&self.notes_requirement)
                .map_err(module_error)?,
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

fn module_error(error: impl std::fmt::Display) -> ModuleError {
    ModuleError::new(error.to_string())
}

struct Fixture {
    instance: Instance,
    rails: Rails,
}

impl Fixture {
    fn component(&self, component_id: &str) -> Component {
        Component::bind(
            ComponentId::new(component_id).expect("component id"),
            self.rails.runtime.as_ref(),
        )
    }

    fn participate(&self, component: Component) -> ComponentParticipation {
        let status = self
            .rails
            .registry
            .register(component, Health::Healthy)
            .expect("register component");
        self.rails
            .registry
            .activate(status.participation())
            .expect("activate component")
            .participation()
            .clone()
    }

    fn bind_notes(&self, consumer: Component) -> ComponentContract<Notes> {
        self.rails
            .communication
            .bind_resolved(
                consumer,
                notes_contract_key().id().clone(),
                ComponentRequirementKind::Required,
                self.rails.resolved_notes.clone(),
            )
            .expect("bind notes")
    }
}

fn fixture(provider: NotesProviderModule) -> Fixture {
    let capture = Arc::new(Mutex::new(None));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.fabric.vertical.cross-api.components").expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("components").expect("block id"))
            .register_module(ComponentRuntimeModule::new())
            .register_module(provider)
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");

    let mut instance = composition
        .materialize(InstanceId::new(INSTANCE_ID).expect("instance id"))
        .expect("materialize composition");
    instance.start().expect("start composition");
    let rails = capture
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured rails");
    Fixture { instance, rails }
}

#[test]
fn explicit_typed_component_contract_uses_core_resolved_provider_provenance() {
    let mut fixture = fixture(NotesProviderModule::new(
        PROVIDER_MODULE_ID,
        PROVIDER_COMPONENT_ID,
        PROVIDER_MODULE_ID,
    ));
    let provider = fixture.component(PROVIDER_COMPONENT_ID);
    let consumer = fixture.component(CONSUMER_COMPONENT_ID);
    fixture.participate(provider.clone());
    let consumer_participation = fixture.participate(consumer.clone());
    let notes = fixture.bind_notes(consumer.clone());

    assert_eq!(
        fixture.rails.resolved_notes.provider().as_str(),
        PROVIDER_MODULE_ID
    );
    assert_eq!(notes.provider(), &provider);
    assert_eq!(notes.consumer(), &consumer);
    assert_eq!(
        notes.requirement().provider_module().as_str(),
        PROVIDER_MODULE_ID
    );
    assert_eq!(
        notes
            .call(&consumer_participation, |notes| notes.reply("hello"))
            .expect("component contract call"),
        "notes:hello"
    );

    fixture.instance.stop();
}

#[test]
fn contract_handle_is_scoped_and_component_ids_do_not_grant_sibling_access() {
    let mut fixture = fixture(NotesProviderModule::new(
        PROVIDER_MODULE_ID,
        PROVIDER_COMPONENT_ID,
        PROVIDER_MODULE_ID,
    ));
    let provider = fixture.component(PROVIDER_COMPONENT_ID);
    let consumer = fixture.component(CONSUMER_COMPONENT_ID);
    let intruder = fixture.component(INTRUDER_COMPONENT_ID);
    fixture.participate(provider);
    fixture.participate(consumer.clone());
    let intruder_participation = fixture.participate(intruder);
    let notes = fixture.bind_notes(consumer);

    assert_eq!(
        notes.call(&intruder_participation, |notes| notes.reply("blocked")),
        Err(ComponentError::ComponentContractConsumerMismatch {
            expected_component_id: ComponentId::new(CONSUMER_COMPONENT_ID).expect("component id"),
            actual_component_id: ComponentId::new(INTRUDER_COMPONENT_ID).expect("component id"),
        })
    );

    fixture.instance.stop();
}

#[test]
fn mismatched_provider_provenance_is_rejected_deterministically() {
    let mut fixture = fixture(NotesProviderModule::new(
        PROVIDER_MODULE_ID,
        PROVIDER_COMPONENT_ID,
        FORGED_PROVIDER_MODULE_ID,
    ));
    let consumer = fixture.component(CONSUMER_COMPONENT_ID);

    let error = match fixture.rails.communication.bind_resolved(
        consumer,
        notes_contract_key().id().clone(),
        ComponentRequirementKind::Required,
        fixture.rails.resolved_notes.clone(),
    ) {
        Ok(_) => panic!("forged provider provenance must fail"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        ComponentError::ComponentContractProviderProvenanceMismatch {
            contract_id: ContractId::new(NOTES_CONTRACT_ID).expect("contract id"),
            expected_provider_module: ModuleId::new(PROVIDER_MODULE_ID)
                .expect("provider module id"),
            actual_provider_module: ModuleId::new(FORGED_PROVIDER_MODULE_ID)
                .expect("forged provider module id"),
        }
    );

    fixture.instance.stop();
}

#[test]
fn component_scope_and_registry_stay_free_of_ambient_lookup_apis() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root");
    assert!(
        !repo_root.join("components/component").exists(),
        "generic fabric-component kernel source must stay external to fabric-packages"
    );
}
