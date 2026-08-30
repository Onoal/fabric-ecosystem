use std::sync::{Arc, Mutex};

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    Instance, InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
    ResolvedContract,
};

use crate::support::provided_component_contract::{
    ProvidedContractModule, provided_component_contract_key,
};
use fabric_component::{
    Component, ComponentCommunication, ComponentContract, ComponentError, ComponentId,
    ComponentParticipation, ComponentRegistry, ComponentRequirementKind, ComponentRequirementRail,
    ComponentRuntime, ComponentRuntimeModule, InvocationContext, InvocationOrigin, InvocationRail,
    OperationId, OperationKey, OperationRegistrar, ProvidedComponentContract,
};
use fabric_component_gateway::{
    Gateway, GatewayError, GatewayModule, GatewayRequest, GatewayStepId, GatewayStepPhase,
    GatewayStepRegistrar,
};
use fabric_component_namespace::NamespaceModule;
use fabric_component_publication::PublicationModule;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Input(String);

#[derive(Clone, Debug, PartialEq, Eq)]
struct Output {
    invocation: InvocationContext,
    reply: String,
}

#[derive(Clone)]
struct Notes;

impl Notes {
    fn reply(&self, context: &InvocationContext, input: &str) -> String {
        format!("{}:{input}", context.invocation_id())
    }
}

struct Rails {
    runtime: Arc<ComponentRuntime>,
    registry: Arc<ComponentRegistry>,
    invocation: Arc<InvocationRail>,
    communication: Arc<ComponentCommunication>,
    registrar: Arc<OperationRegistrar>,
    gateway: Arc<Gateway>,
    requirements: Arc<ComponentRequirementRail>,
    steps: Arc<GatewayStepRegistrar>,
    resolved_notes: ResolvedContract<ProvidedComponentContract<Notes>>,
}

type Capture = Arc<Mutex<Option<Rails>>>;

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    runtime: ContractRequirement<ComponentRuntime>,
    registry: ContractRequirement<ComponentRegistry>,
    invocation: ContractRequirement<InvocationRail>,
    communication: ContractRequirement<ComponentCommunication>,
    registrar: ContractRequirement<OperationRegistrar>,
    gateway: ContractRequirement<Gateway>,
    requirements: ContractRequirement<ComponentRequirementRail>,
    steps: ContractRequirement<GatewayStepRegistrar>,
    notes: ContractRequirement<ProvidedComponentContract<Notes>>,
    capture: Capture,
}

impl CaptureModule {
    fn new(capture: Capture) -> Self {
        Self {
            module_id: ModuleId::new("runtime.invocation.capture").expect("module id"),
            runtime: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            invocation: ContractRequirement::provisional(fabric_component::invocation_contract_id()),
            communication: ContractRequirement::provisional(
                fabric_component::component_communication_contract_id(),
            ),
            registrar: ContractRequirement::provisional(
                fabric_component::operation_registrar_contract_id(),
            ),
            gateway: ContractRequirement::provisional(
                fabric_component_gateway::gateway_contract_id(),
            ),
            requirements: ContractRequirement::provisional(
                fabric_component::component_requirement_contract_id(),
            ),
            steps: ContractRequirement::provisional(
                fabric_component_gateway::gateway_step_registrar_contract_id(),
            ),
            notes: ContractRequirement::provisional(notes_contract_key().id().clone()),
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
            self.runtime.id().clone(),
            self.registry.id().clone(),
            self.invocation.id().clone(),
            self.communication.id().clone(),
            self.registrar.id().clone(),
            self.gateway.id().clone(),
            self.requirements.id().clone(),
            self.steps.id().clone(),
            self.notes.id().clone(),
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
            runtime: bindings.resolve(&self.runtime).map_err(module_error)?,
            registry: bindings.resolve(&self.registry).map_err(module_error)?,
            invocation: bindings.resolve(&self.invocation).map_err(module_error)?,
            communication: bindings
                .resolve(&self.communication)
                .map_err(module_error)?,
            registrar: bindings.resolve(&self.registrar).map_err(module_error)?,
            gateway: bindings.resolve(&self.gateway).map_err(module_error)?,
            requirements: bindings.resolve(&self.requirements).map_err(module_error)?,
            steps: bindings.resolve(&self.steps).map_err(module_error)?,
            resolved_notes: bindings
                .resolve_with_provider(&self.notes)
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

fn fixture() -> (Instance, Rails) {
    let capture = Arc::new(Mutex::new(None));
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.invocation").expect("id"))
            .register_block(
                BlockBuilder::new(BlockId::new("runtime.invocation.block").expect("id"))
                    .register_module(ComponentRuntimeModule::new())
                    .register_module(ProvidedContractModule::new(
                        "runtime.invocation.notes.provider",
                        "component.provider",
                        "runtime.invocation.notes",
                        Arc::new(Notes),
                    ))
                    .register_module(NamespaceModule::new())
                    .register_module(PublicationModule::new())
                    .register_module(GatewayModule::new())
                    .register_module(CaptureModule::new(Arc::clone(&capture)))
                    .build(),
            )
            .build()
            .expect("composition");
    let mut instance = composition
        .materialize(InstanceId::new("runtime.invocation").expect("instance id"))
        .expect("materialize composition");
    instance.start().expect("start");
    let rails = capture.lock().expect("capture lock").take().expect("rails");
    (instance, rails)
}

fn component(rails: &Rails, id: &str) -> Component {
    Component::bind(ComponentId::new(id).expect("id"), rails.runtime.as_ref())
}

fn participate(rails: &Rails, component: Component, health: Health) -> ComponentParticipation {
    let participation = rails
        .registry
        .register(component, health)
        .expect("participate")
        .participation()
        .clone();
    rails
        .registry
        .activate(&participation)
        .expect("activate participation");
    participation
}

fn prepare(rails: &Rails, component: Component, health: Health) -> ComponentParticipation {
    rails
        .registry
        .register(component, health)
        .expect("prepare")
        .participation()
        .clone()
}

fn operation() -> OperationKey<Input, Output> {
    OperationKey::new(
        OperationId::new("context.proof").expect("id"),
        fabric_component::OperationTypeId::new("fabric.test.invocation.context_proof.input")
            .expect("input type id"),
        fabric_component::OperationTypeId::new("fabric.test.invocation.context_proof.output")
            .expect("output type id"),
    )
}

fn notes_contract_key() -> fabric_core::ContractKey<ProvidedComponentContract<Notes>> {
    provided_component_contract_key("runtime.invocation.notes")
}

fn bind_notes(rails: &Rails, consumer: Component, provider: Component) -> ComponentContract<Notes> {
    let notes = rails
        .communication
        .bind_resolved(
            consumer.clone(),
            notes_contract_key().id().clone(),
            ComponentRequirementKind::Required,
            rails.resolved_notes.clone(),
        )
        .expect("bind");
    assert_eq!(
        rails
            .requirements
            .requirements(consumer.component_id())
            .len(),
        1
    );
    assert_eq!(notes.provider(), &provider);
    notes
}

#[test]
fn gateway_context_reaches_operation_and_contract_provider_without_changing_root() {
    let (mut instance, rails) = fixture();
    let provider = component(&rails, "component.provider");
    let caller = component(&rails, "component.caller");
    participate(&rails, provider.clone(), Health::Healthy);
    let caller = prepare(&rails, caller.clone(), Health::Healthy);
    let notes = bind_notes(&rails, caller.component().clone(), provider);
    let operation_owner = caller.clone();
    rails
        .registrar
        .register_with_context(caller.clone(), operation(), move |context, input| {
            let notes = notes.clone();
            let caller = operation_owner.clone();
            async move {
                let reply =
                    notes.call_with_context(&caller, &context, |provider_context, notes| {
                        assert_eq!(provider_context, &context);
                        notes.reply(provider_context, &input.0)
                    })?;
                Ok(Output {
                    invocation: context,
                    reply,
                })
            }
        })
        .expect("operation");
    rails.registry.activate(&caller).expect("activate caller");

    let output = block_on(
        rails
            .gateway
            .invoke(GatewayRequest::new(operation(), Input("hello".into()))),
    )
    .expect("gateway")
    .into_output();
    assert!(matches!(
        output.invocation.root_origin(),
        InvocationOrigin::External
    ));
    assert_eq!(
        output.invocation.instance_id(),
        &rails.runtime.instance_id()
    );
    assert_eq!(
        output.reply,
        format!("{}:hello", output.invocation.invocation_id())
    );
    instance.stop();
}

#[test]
fn gateway_roots_keep_runtime_generation_and_increment_invocation_ids() {
    let (mut instance, rails) = fixture();
    let first = rails.invocation.begin_external().expect("first");
    let second = rails.invocation.begin_external().expect("second");
    assert_eq!(first.generation(), second.generation());
    assert_ne!(first.invocation_id(), second.invocation_id());
    instance.stop();
}

#[test]
fn participating_component_can_begin_internal_root_and_absent_component_cannot() {
    let (mut instance, rails) = fixture();
    let caller = component(&rails, "component.caller");
    let stale = prepare(&rails, caller.clone(), Health::Healthy);
    rails
        .registry
        .unregister(&stale)
        .expect("unregister stale caller");
    assert!(matches!(
        rails.invocation.begin_component(stale),
        Err(ComponentError::InvocationOriginNotParticipating(_))
    ));
    let caller_participation = participate(&rails, caller.clone(), Health::Healthy);
    let context = rails
        .invocation
        .begin_component(caller_participation)
        .expect("context");
    assert_eq!(context.root_origin(), &InvocationOrigin::Component(caller));
    instance.stop();
}

#[test]
fn component_root_admission_interprets_unavailable_but_keeps_degraded_usable() {
    let (mut instance, rails) = fixture();
    let component = component(&rails, "component.available.root");
    let participation = participate(&rails, component, Health::Healthy);
    assert!(
        rails
            .invocation
            .begin_component(participation.clone())
            .is_ok()
    );
    rails
        .registry
        .update_health(&participation, Health::Unavailable)
        .expect("unavailable");
    assert_eq!(
        rails.invocation.begin_component(participation.clone()),
        Err(ComponentError::ComponentUnavailable(
            ComponentId::new("component.available.root").expect("component id"),
        ))
    );
    rails
        .registry
        .update_health(&participation, Health::Degraded)
        .expect("degraded");
    assert!(rails.invocation.begin_component(participation).is_ok());
    instance.stop();
}

#[test]
fn stale_context_preserves_identity_but_does_not_bypass_participation_or_instance_isolation() {
    let (mut instance, rails) = fixture();
    let provider = component(&rails, "component.provider");
    let caller = component(&rails, "component.caller");
    let provider_participation = participate(&rails, provider.clone(), Health::Healthy);
    let caller_participation = participate(&rails, caller.clone(), Health::Healthy);
    let notes: ComponentContract<Notes> = bind_notes(&rails, caller.clone(), provider.clone());
    let context = rails
        .invocation
        .begin_component(caller_participation.clone())
        .expect("context");
    rails
        .registry
        .unregister(&caller_participation)
        .expect("leave caller");
    assert!(matches!(
        notes.call_with_context(&caller_participation, &context, |_, notes| notes
            .reply(&context, "no")),
        Err(ComponentError::ComponentContractCallerNotParticipating(_))
    ));
    participate(&rails, caller.clone(), Health::Healthy);
    rails
        .registry
        .unregister(&provider_participation)
        .expect("leave provider");
    assert!(matches!(
        notes.call_with_context(
            rails
                .registry
                .component(caller.component_id())
                .expect("caller status")
                .participation(),
            &context,
            |_, notes| notes.reply(&context, "no"),
        ),
        Err(ComponentError::ComponentContractProviderNotParticipating(_))
    ));
    instance.stop();
}

#[test]
fn foreign_context_cannot_cross_instance() {
    let (mut instance, rails) = fixture();
    let foreign = {
        let capture = Arc::new(Mutex::new(None));
        let composition =
            CompositionBuilder::new(CompositionId::new("runtime.foreign").expect("id"))
                .register_block(
                    BlockBuilder::new(BlockId::new("runtime.foreign.block").expect("id"))
                        .register_module(ComponentRuntimeModule::new())
                        .register_module(ProvidedContractModule::new(
                            "runtime.foreign.notes.provider",
                            "component.provider",
                            "runtime.invocation.notes",
                            Arc::new(Notes),
                        ))
                        .register_module(NamespaceModule::new())
                        .register_module(PublicationModule::new())
                        .register_module(GatewayModule::new())
                        .register_module(CaptureModule::new(Arc::clone(&capture)))
                        .build(),
                )
                .build()
                .expect("composition");
        let mut foreign_instance = composition
            .materialize(InstanceId::new("runtime.foreign").expect("instance id"))
            .expect("materialize composition");
        foreign_instance.start().expect("start");
        let foreign_rails = capture.lock().expect("capture lock").take().expect("rails");
        let context = foreign_rails.invocation.begin_external().expect("foreign");
        foreign_instance.stop();
        context
    };
    let provider = component(&rails, "component.provider");
    let caller = component(&rails, "component.caller");
    participate(&rails, provider.clone(), Health::Healthy);
    participate(&rails, caller.clone(), Health::Healthy);
    let notes = bind_notes(&rails, caller.clone(), provider);
    assert!(matches!(
        notes.call_with_context(
            rails
                .registry
                .component(caller.component_id())
                .expect("caller")
                .participation(),
            &foreign,
            |_, notes| notes.reply(&foreign, "no"),
        ),
        Err(ComponentError::InvocationContextInstanceMismatch { .. })
    ));
    instance.stop();
}

#[test]
fn gateway_runs_deterministic_component_steps_and_cleans_up_departed_owner() {
    let (mut instance, rails) = fixture();
    let operation_owner = component(&rails, "component.operation");
    let enrich_owner = component(&rails, "component.enrich");
    let guard_owner = component(&rails, "component.guard");
    let operation_participation = prepare(&rails, operation_owner.clone(), Health::Healthy);
    let enrich_participation = prepare(&rails, enrich_owner.clone(), Health::Healthy);
    let guard_participation = prepare(&rails, guard_owner.clone(), Health::Healthy);
    let events = Arc::new(Mutex::new(Vec::new()));
    let operation_events = Arc::clone(&events);
    rails
        .registrar
        .register_with_context(
            operation_participation.clone(),
            operation(),
            move |context, input| {
                let events = Arc::clone(&operation_events);
                async move {
                    events.lock().expect("events").push("dispatch");
                    Ok(Output {
                        invocation: context,
                        reply: input.0,
                    })
                }
            },
        )
        .expect("operation");
    let enrich_events = Arc::clone(&events);
    rails
        .steps
        .register(
            enrich_participation.clone(),
            GatewayStepId::new("step.enrich").expect("id"),
            GatewayStepPhase::Enrich,
            |_| true,
            move |call| {
                assert!(matches!(
                    call.context().root_origin(),
                    InvocationOrigin::External
                ));
                enrich_events.lock().expect("events").push("enrich");
                Ok(())
            },
        )
        .expect("enrich");
    let guard_events = Arc::clone(&events);
    rails
        .steps
        .register(
            guard_participation.clone(),
            GatewayStepId::new("step.guard").expect("id"),
            GatewayStepPhase::Guard,
            |call| call.operation_id().as_str() == "context.proof",
            move |_| {
                guard_events.lock().expect("events").push("guard");
                Ok(())
            },
        )
        .expect("guard");
    rails
        .registry
        .activate(&operation_participation)
        .expect("activate operation owner");
    rails
        .registry
        .activate(&enrich_participation)
        .expect("activate enrich owner");
    rails
        .registry
        .activate(&guard_participation)
        .expect("activate guard owner");
    let _ = block_on(
        rails
            .gateway
            .invoke(GatewayRequest::new(operation(), Input("ok".into()))),
    )
    .expect("gateway");
    assert_eq!(
        *events.lock().expect("events"),
        vec!["enrich", "guard", "dispatch"]
    );
    rails
        .registry
        .update_health(&enrich_participation, Health::Unavailable)
        .expect("enrich unavailable");
    events.lock().expect("events").clear();
    assert_eq!(
        block_on(
            rails
                .gateway
                .invoke(GatewayRequest::new(operation(), Input("blocked".into()))),
        ),
        Err(GatewayError::Component(
            ComponentError::ComponentUnavailable(
                ComponentId::new("component.enrich").expect("component id"),
            )
        ))
    );
    assert!(events.lock().expect("events").is_empty());
    rails
        .registry
        .update_health(&enrich_participation, Health::Degraded)
        .expect("enrich degraded");
    let _ = block_on(
        rails
            .gateway
            .invoke(GatewayRequest::new(operation(), Input("degraded".into()))),
    )
    .expect("gateway with degraded step");
    rails
        .registry
        .unregister(&enrich_participation)
        .expect("leave");
    events.lock().expect("events").clear();
    let _ = block_on(
        rails
            .gateway
            .invoke(GatewayRequest::new(operation(), Input("ok".into()))),
    )
    .expect("gateway");
    assert_eq!(*events.lock().expect("events"), vec!["guard", "dispatch"]);
    let rejoined = prepare(&rails, enrich_owner.clone(), Health::Healthy);
    assert!(
        rails
            .steps
            .register(
                rejoined.clone(),
                GatewayStepId::new("step.enrich").expect("id"),
                GatewayStepPhase::Enrich,
                |_| true,
                |_| Ok(())
            )
            .is_ok()
    );
    rails
        .registry
        .activate(&rejoined)
        .expect("activate rejoined enrich owner");
    instance.stop();
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = pin!(future);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}
