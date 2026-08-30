use std::future::Future;
use std::pin::Pin;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::task::{Context, Poll, Wake, Waker};

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    Instance, InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
    ResolvedContract,
};

use crate::support::provided_component_contract::{
    ProvidedContractModule, provided_component_contract_key,
};
use fabric_component::{
    Component, ComponentAggregateBlocker, ComponentCommunication, ComponentContract,
    ComponentControlRail, ComponentControlSnapshot, ComponentDesiredState,
    ComponentEffectiveAvailability, ComponentError, ComponentId, ComponentMaterializer,
    ComponentReadinessPolicy, ComponentReadinessRail, ComponentReconstructionOutcome,
    ComponentReconstructionRail, ComponentReconstructionResult, ComponentRegistry,
    ComponentRequirementKind, ComponentRequirementRail, ComponentRuntime,
    ComponentRuntimeDefinition, ComponentRuntimeLifecycle, ComponentRuntimeModule,
    ComponentRuntimeScope, ComponentScope, InvocationContext, InvocationOrigin, InvocationRail,
    OperationDescriptor, OperationId, OperationKey, OperationRail, OperationRegistrar,
    OperationTypeId, ParticipationState, ProvidedComponentContract, SurfaceId, SurfaceRegistry,
};
use fabric_component_gateway::{
    Gateway, GatewayError, GatewayModule, GatewayRequest, GatewayResponse, GatewayStepId,
    GatewayStepPhase, GatewayStepRegistrar,
};
use fabric_component_namespace::NamespaceModule;
use fabric_component_namespace::{Namespace, NamespaceName};
use fabric_component_publication::PublicationContract;
use fabric_component_publication::PublicationModule;

const NOTES_CONTRACT_ID: &str = "fabric.component.shell_closure.notes";
const ALPHA_REQUIRED_CONTRACT_ID: &str = "fabric.component.shell_closure.alpha.required";
const GAMMA_OPTIONAL_CONTRACT_ID: &str = "fabric.component.shell_closure.gamma.optional";

const PREPARING_OPERATION_ID: &str = "fabric.test.shell_closure.preparing";
const PREPARING_GUARD_ID: &str = "fabric.test.shell_closure.preparing.guard";

const ALPHA_OPERATION_ID: &str = "fabric.test.shell_closure.alpha";
const BETA_OPERATION_ID: &str = "fabric.test.shell_closure.beta";
const GAMMA_OPERATION_ID: &str = "fabric.test.shell_closure.gamma";
const GUARD_STEP_ID: &str = "fabric.test.shell_closure.guard";

const ALPHA_COMPONENT_ID: &str = "component.alpha";
const BETA_COMPONENT_ID: &str = "component.beta";
const GAMMA_COMPONENT_ID: &str = "component.gamma";
const GUARD_COMPONENT_ID: &str = "component.guard";

#[derive(Clone, Debug, PartialEq, Eq)]
struct EchoInput(&'static str);

#[derive(Clone, Debug, PartialEq, Eq)]
struct EchoOutput(&'static str);

#[derive(Clone, Debug, PartialEq, Eq)]
struct AlphaInput(&'static str);

#[derive(Clone, Debug, PartialEq, Eq)]
struct AlphaOutput {
    context: InvocationContext,
    reply: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BetaInput(&'static str);

#[derive(Clone, Debug, PartialEq, Eq)]
struct BetaOutput {
    gateway_context: InvocationContext,
    continued_context: InvocationContext,
    alpha_context: InvocationContext,
    alpha_reply: String,
    alpha_operation_reply: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GammaInput(&'static str);

#[derive(Clone, Debug, PartialEq, Eq)]
struct GammaOutput {
    context: InvocationContext,
    reply: &'static str,
}

#[derive(Clone)]
struct NotesContract;

impl NotesContract {
    fn reply(&self, context: &InvocationContext, message: &str) -> String {
        format!("{}:{message}", context.invocation_id())
    }
}

fn notes_contract_key() -> fabric_core::ContractKey<ProvidedComponentContract<NotesContract>> {
    provided_component_contract_key(NOTES_CONTRACT_ID)
}

fn alpha_required_contract_key() -> fabric_core::ContractKey<ProvidedComponentContract<()>> {
    provided_component_contract_key(ALPHA_REQUIRED_CONTRACT_ID)
}

fn gamma_optional_contract_key() -> fabric_core::ContractKey<ProvidedComponentContract<()>> {
    provided_component_contract_key(GAMMA_OPTIONAL_CONTRACT_ID)
}

#[derive(Clone)]
struct Rails {
    runtime: Arc<ComponentRuntime>,
    registry: Arc<ComponentRegistry>,
    materializer: Arc<ComponentMaterializer>,
    communication: Arc<ComponentCommunication>,
    control: Arc<ComponentControlRail>,
    readiness: Arc<ComponentReadinessRail>,
    reconstruction: Arc<ComponentReconstructionRail>,
    requirements: Arc<ComponentRequirementRail>,
    invocation: Arc<InvocationRail>,
    operations: Arc<OperationRail>,
    registrar: Arc<OperationRegistrar>,
    gateway: Arc<Gateway>,
    steps: Arc<GatewayStepRegistrar>,
    surfaces: Arc<SurfaceRegistry>,
    namespace: Arc<Namespace>,
    publications: Arc<PublicationContract>,
    resolved_notes: ResolvedContract<ProvidedComponentContract<NotesContract>>,
    resolved_alpha_required: ResolvedContract<ProvidedComponentContract<()>>,
    resolved_gamma_optional: ResolvedContract<ProvidedComponentContract<()>>,
}

type Capture = Arc<Mutex<Option<Rails>>>;

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    runtime: ContractRequirement<ComponentRuntime>,
    registry: ContractRequirement<ComponentRegistry>,
    materializer: ContractRequirement<ComponentMaterializer>,
    communication: ContractRequirement<ComponentCommunication>,
    control: ContractRequirement<ComponentControlRail>,
    readiness: ContractRequirement<ComponentReadinessRail>,
    reconstruction: ContractRequirement<ComponentReconstructionRail>,
    requirements: ContractRequirement<ComponentRequirementRail>,
    invocation: ContractRequirement<InvocationRail>,
    operations: ContractRequirement<OperationRail>,
    registrar: ContractRequirement<OperationRegistrar>,
    gateway: ContractRequirement<Gateway>,
    steps: ContractRequirement<GatewayStepRegistrar>,
    surfaces: ContractRequirement<SurfaceRegistry>,
    namespace: ContractRequirement<Namespace>,
    publications: ContractRequirement<PublicationContract>,
    notes: ContractRequirement<ProvidedComponentContract<NotesContract>>,
    alpha_required: ContractRequirement<ProvidedComponentContract<()>>,
    gamma_optional: ContractRequirement<ProvidedComponentContract<()>>,
    capture: Capture,
}

impl CaptureModule {
    fn new(capture: Capture) -> Self {
        Self {
            module_id: ModuleId::new("runtime.shell_closure.capture").expect("module id"),
            runtime: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            materializer: ContractRequirement::provisional(
                fabric_component::component_materializer_contract_id(),
            ),
            communication: ContractRequirement::provisional(
                fabric_component::component_communication_contract_id(),
            ),
            control: ContractRequirement::provisional(
                fabric_component::component_control_contract_id(),
            ),
            readiness: ContractRequirement::provisional(
                fabric_component::component_readiness_contract_id(),
            ),
            reconstruction: ContractRequirement::provisional(
                fabric_component::component_reconstruction_contract_id(),
            ),
            requirements: ContractRequirement::provisional(
                fabric_component::component_requirement_contract_id(),
            ),
            invocation: ContractRequirement::provisional(fabric_component::invocation_contract_id()),
            operations: ContractRequirement::provisional(
                fabric_component::operation_rail_contract_id(),
            ),
            registrar: ContractRequirement::provisional(
                fabric_component::operation_registrar_contract_id(),
            ),
            gateway: ContractRequirement::provisional(
                fabric_component_gateway::gateway_contract_id(),
            ),
            steps: ContractRequirement::provisional(
                fabric_component_gateway::gateway_step_registrar_contract_id(),
            ),
            surfaces: ContractRequirement::provisional(fabric_component::surface_contract_id()),
            namespace: ContractRequirement::provisional(
                fabric_component_namespace::namespace_contract_id(),
            ),
            publications: ContractRequirement::provisional(
                fabric_component_publication::publication_contract_id(),
            ),
            notes: ContractRequirement::provisional(notes_contract_key().id().clone()),
            alpha_required: ContractRequirement::provisional(
                alpha_required_contract_key().id().clone(),
            ),
            gamma_optional: ContractRequirement::provisional(
                gamma_optional_contract_key().id().clone(),
            ),
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
            self.materializer.id().clone(),
            self.communication.id().clone(),
            self.control.id().clone(),
            self.readiness.id().clone(),
            self.reconstruction.id().clone(),
            self.requirements.id().clone(),
            self.invocation.id().clone(),
            self.operations.id().clone(),
            self.registrar.id().clone(),
            self.gateway.id().clone(),
            self.steps.id().clone(),
            self.surfaces.id().clone(),
            self.namespace.id().clone(),
            self.publications.id().clone(),
            self.notes.id().clone(),
            self.alpha_required.id().clone(),
            self.gamma_optional.id().clone(),
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
            materializer: bindings.resolve(&self.materializer).map_err(module_error)?,
            communication: bindings
                .resolve(&self.communication)
                .map_err(module_error)?,
            control: bindings.resolve(&self.control).map_err(module_error)?,
            readiness: bindings.resolve(&self.readiness).map_err(module_error)?,
            reconstruction: bindings
                .resolve(&self.reconstruction)
                .map_err(module_error)?,
            requirements: bindings.resolve(&self.requirements).map_err(module_error)?,
            invocation: bindings.resolve(&self.invocation).map_err(module_error)?,
            operations: bindings.resolve(&self.operations).map_err(module_error)?,
            registrar: bindings.resolve(&self.registrar).map_err(module_error)?,
            gateway: bindings.resolve(&self.gateway).map_err(module_error)?,
            steps: bindings.resolve(&self.steps).map_err(module_error)?,
            surfaces: bindings.resolve(&self.surfaces).map_err(module_error)?,
            namespace: bindings.resolve(&self.namespace).map_err(module_error)?,
            publications: bindings.resolve(&self.publications).map_err(module_error)?,
            resolved_notes: bindings
                .resolve_with_provider(&self.notes)
                .map_err(module_error)?,
            resolved_alpha_required: bindings
                .resolve_with_provider(&self.alpha_required)
                .map_err(module_error)?,
            resolved_gamma_optional: bindings
                .resolve_with_provider(&self.gamma_optional)
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

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    struct ThreadWaker {
        ready: Mutex<bool>,
        wake: Condvar,
    }

    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            let mut ready = self.ready.lock().expect("waker lock");
            *ready = true;
            self.wake.notify_one();
        }
    }

    let waker = Arc::new(ThreadWaker {
        ready: Mutex::new(true),
        wake: Condvar::new(),
    });
    let waker_ref = Waker::from(Arc::clone(&waker));
    let mut context = Context::from_waker(&waker_ref);
    let mut future = Pin::from(Box::new(future));
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        let mut ready = waker.ready.lock().expect("waker lock");
        while !*ready {
            ready = waker.wake.wait(ready).expect("waker wait");
        }
        *ready = false;
    }
}

fn component(runtime_contract: &ComponentRuntime, component_id: &str) -> Component {
    Component::bind(
        ComponentId::new(component_id).expect("component id"),
        runtime_contract,
    )
}

fn alpha_operation() -> OperationKey<AlphaInput, AlphaOutput> {
    OperationKey::new(
        OperationId::new(ALPHA_OPERATION_ID).expect("alpha operation id"),
        OperationTypeId::new("fabric.test.shell_closure.alpha.input").expect("input type id"),
        OperationTypeId::new("fabric.test.shell_closure.alpha.output").expect("output type id"),
    )
}

fn beta_operation() -> OperationKey<BetaInput, BetaOutput> {
    OperationKey::new(
        OperationId::new(BETA_OPERATION_ID).expect("beta operation id"),
        OperationTypeId::new("fabric.test.shell_closure.beta.input").expect("input type id"),
        OperationTypeId::new("fabric.test.shell_closure.beta.output").expect("output type id"),
    )
}

fn gamma_operation() -> OperationKey<GammaInput, GammaOutput> {
    OperationKey::new(
        OperationId::new(GAMMA_OPERATION_ID).expect("gamma operation id"),
        OperationTypeId::new("fabric.test.shell_closure.gamma.input").expect("input type id"),
        OperationTypeId::new("fabric.test.shell_closure.gamma.output").expect("output type id"),
    )
}

fn preparing_operation() -> OperationKey<EchoInput, EchoOutput> {
    OperationKey::new(
        OperationId::new(PREPARING_OPERATION_ID).expect("preparing operation id"),
        OperationTypeId::new("fabric.test.shell_closure.preparing.input").expect("input type id"),
        OperationTypeId::new("fabric.test.shell_closure.preparing.output").expect("output type id"),
    )
}

fn fixture(
    instance_id: &str,
    required_components: &[&str],
    runtime_definitions: impl IntoIterator<Item = ComponentRuntimeDefinition>,
    control_snapshot: Option<ComponentControlSnapshot>,
) -> (Instance, Rails) {
    let capture = Arc::new(Mutex::new(None));
    let policy = ComponentReadinessPolicy::new(
        required_components
            .iter()
            .map(|component_id| ComponentId::new(*component_id).expect("required component id")),
    )
    .expect("policy");
    let runtime_module = match control_snapshot {
        Some(snapshot) => ComponentRuntimeModule::with_configuration_and_control_snapshot(
            policy,
            runtime_definitions,
            snapshot,
        )
        .expect("component runtime with snapshot"),
        None => ComponentRuntimeModule::with_configuration(policy, runtime_definitions)
            .expect("component runtime"),
    };
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.shell_closure").expect("id"))
            .register_block(
                BlockBuilder::new(BlockId::new("runtime.shell_closure.block").expect("block"))
                    .register_module(runtime_module)
                    .register_module(ProvidedContractModule::new(
                        "runtime.shell_closure.notes.provider",
                        ALPHA_COMPONENT_ID,
                        NOTES_CONTRACT_ID,
                        Arc::new(NotesContract),
                    ))
                    .register_module(ProvidedContractModule::new(
                        "runtime.shell_closure.alpha.required.provider",
                        ALPHA_COMPONENT_ID,
                        ALPHA_REQUIRED_CONTRACT_ID,
                        Arc::new(()),
                    ))
                    .register_module(ProvidedContractModule::new(
                        "runtime.shell_closure.gamma.optional.provider",
                        GAMMA_COMPONENT_ID,
                        GAMMA_OPTIONAL_CONTRACT_ID,
                        Arc::new(()),
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
        .materialize(InstanceId::new(instance_id).expect("instance id"))
        .expect("materialize instance");
    instance.start().expect("start composition");
    let rails = capture.lock().expect("capture lock").take().expect("rails");
    (instance, rails)
}

fn report_outcome<'a>(
    outcomes: &'a [ComponentReconstructionOutcome],
    component_id: &str,
) -> &'a ComponentReconstructionOutcome {
    outcomes
        .iter()
        .find(|outcome| outcome.component_id().as_str() == component_id)
        .expect("outcome")
}

fn descriptor_ids(descriptor: &OperationDescriptor) -> (&str, &str, &str, &str) {
    (
        descriptor.definition().id().as_str(),
        descriptor.definition().input_type().as_str(),
        descriptor.definition().output_type().as_str(),
        descriptor.owner().component_id().as_str(),
    )
}

#[test]
fn preparing_visibility_and_active_interface_freeze_hold_in_heterogeneous_fixture() {
    let captured_scope: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let runtime_definitions = [ComponentRuntimeDefinition::new(
        ComponentId::new("component.captured").expect("component id"),
        {
            let captured_scope = Arc::clone(&captured_scope);
            move |scope: &ComponentRuntimeScope| {
                let component_scope = scope.component_scope();
                assert_eq!(component_scope.participation(), scope.participation());
                assert!(matches!(
                    component_scope.begin_invocation(),
                    Err(ComponentError::ComponentUnavailable(_))
                ));
                assert!(matches!(
                    component_scope.update_health(Health::Degraded),
                    Err(ComponentError::ComponentParticipationNotActive(_))
                ));
                *captured_scope.lock().expect("captured scope") = Some(component_scope);
                Ok(Health::Healthy)
            }
        },
    )];
    let (mut composition, rails) = fixture(
        "runtime.shell_closure.preparing",
        &[],
        runtime_definitions,
        None,
    );

    let operation_owner = component(&rails.runtime, "component.operation");
    let guard_owner = component(&rails.runtime, "component.guard");
    let operation_participation = rails
        .registry
        .register(operation_owner.clone(), Health::Healthy)
        .expect("prepare operation owner")
        .participation()
        .clone();
    let guard_participation = rails
        .registry
        .register(guard_owner.clone(), Health::Healthy)
        .expect("prepare guard owner")
        .participation()
        .clone();
    let events = Arc::new(Mutex::new(Vec::new()));
    rails
        .registrar
        .register_with_context(operation_participation.clone(), preparing_operation(), {
            let events = Arc::clone(&events);
            move |context, input| {
                let events = Arc::clone(&events);
                async move {
                    events.lock().expect("events").push(format!(
                        "operation:{}:{}",
                        context.invocation_id(),
                        input.0
                    ));
                    Ok(EchoOutput(input.0))
                }
            }
        })
        .expect("stage operation");
    rails
        .steps
        .register(
            guard_participation.clone(),
            GatewayStepId::new(PREPARING_GUARD_ID).expect("step id"),
            GatewayStepPhase::Guard,
            |_| true,
            {
                let events = Arc::clone(&events);
                move |_| {
                    events.lock().expect("events").push("guard".to_owned());
                    Ok(())
                }
            },
        )
        .expect("stage guard");

    assert!(rails.operations.operations().is_empty());
    match block_on(rails.gateway.invoke(GatewayRequest::new(
        preparing_operation(),
        EchoInput("blocked"),
    ))) {
        Err(GatewayError::UnknownGatewayOperation(operation_id)) => {
            assert_eq!(operation_id, preparing_operation().id().clone());
        }
        Err(GatewayError::Component(ComponentError::ComponentUnavailable(component_id))) => {
            assert_eq!(component_id, *operation_owner.component_id());
        }
        other => panic!(
            "pre-activation gateway invoke must remain invisible or fail closed, got {other:?}"
        ),
    }
    match rails.operations.operation(preparing_operation().id()) {
        Err(ComponentError::UnknownOperation(operation_id)) => {
            assert_eq!(operation_id, preparing_operation().id().clone());
        }
        Ok(_) => {}
        other => panic!("pre-activation operation visibility changed unexpectedly: {other:?}"),
    }
    assert!(events.lock().expect("events").is_empty());

    let captured_status = rails
        .materializer
        .materialize(&ComponentId::new("component.captured").expect("component id"))
        .expect("materialize captured");
    let retained_scope = captured_scope
        .lock()
        .expect("captured scope")
        .clone()
        .expect("retained scope");
    let captured_invocation = retained_scope.begin_invocation().expect("captured begin");
    assert_eq!(
        retained_scope.participation(),
        captured_status.participation()
    );
    assert_eq!(
        captured_invocation.context().root_origin(),
        &InvocationOrigin::Component(component(&rails.runtime, "component.captured"))
    );

    rails
        .registry
        .activate(&operation_participation)
        .expect("activate operation owner");
    assert!(matches!(
        block_on(rails.gateway.invoke(GatewayRequest::new(
            preparing_operation(),
            EchoInput("still-blocked"),
        ))),
        Err(GatewayError::Component(ComponentError::ComponentUnavailable(component_id)))
            if component_id == *guard_owner.component_id()
    ));
    assert!(events.lock().expect("events").is_empty());

    rails
        .registry
        .activate(&guard_participation)
        .expect("activate guard owner");
    let response: GatewayResponse<EchoOutput> = block_on(rails.gateway.invoke(
        GatewayRequest::new(preparing_operation(), EchoInput("ready")),
    ))
    .expect("gateway invoke");
    assert_eq!(response.into_output(), EchoOutput("ready"));
    let recorded_events = events.lock().expect("events").clone();
    assert_eq!(recorded_events.len(), 2);
    assert_eq!(recorded_events[0], "guard");
    assert!(recorded_events[1].ends_with(":ready"));
    assert_eq!(rails.operations.operations().len(), 1);

    assert!(matches!(
        rails.registrar.register(
            operation_participation,
            OperationKey::new(
                OperationId::new("fabric.test.shell_closure.preparing.extra")
                    .expect("operation id"),
                OperationTypeId::new("fabric.test.shell_closure.preparing.extra.input")
                    .expect("input type id"),
                OperationTypeId::new("fabric.test.shell_closure.preparing.extra.output")
                    .expect("output type id"),
            ),
            |input: EchoInput| async move { Ok(EchoOutput(input.0)) },
        ),
        Err(ComponentError::OperationOwnerNotPreparing { .. })
    ));
    assert!(matches!(
        rails.steps.register(
            guard_participation,
            GatewayStepId::new("fabric.test.shell_closure.preparing.extra").expect("step id"),
            GatewayStepPhase::Guard,
            |_| true,
            |_| Ok(()),
        ),
        Err(GatewayError::GatewayStepOwnerNotPreparing(_, _))
    ));

    composition.stop();
}

#[test]
fn heterogeneous_shell_closure_proves_component_only_foundation() {
    let alpha_scope: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let beta_scope: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let gamma_scope: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let guard_scope: Arc<Mutex<Option<ComponentScope>>> = Arc::new(Mutex::new(None));
    let beta_notes_handle: Arc<Mutex<Option<ComponentContract<NotesContract>>>> =
        Arc::new(Mutex::new(None));
    let alpha_contexts = Arc::new(Mutex::new(Vec::<InvocationContext>::new()));
    let beta_contexts = Arc::new(Mutex::new(Vec::<InvocationContext>::new()));
    let guard_events = Arc::new(Mutex::new(Vec::<InvocationContext>::new()));
    let gamma_recipe_runs = Arc::new(AtomicUsize::new(0));
    let runtime_capture = Arc::new(Mutex::new(None::<Rails>));

    let build_runtime_definitions = |runtime_capture: Arc<Mutex<Option<Rails>>>| {
        vec![
            ComponentRuntimeDefinition::new(
                ComponentId::new(ALPHA_COMPONENT_ID).expect("component id"),
                {
                    let alpha_scope = Arc::clone(&alpha_scope);
                    let alpha_contexts = Arc::clone(&alpha_contexts);
                    move |scope: &ComponentRuntimeScope| {
                        *alpha_scope.lock().expect("alpha scope") = Some(scope.component_scope());
                        scope.operation_with_context(alpha_operation(), {
                            let alpha_contexts = Arc::clone(&alpha_contexts);
                            move |context, input| {
                                let alpha_contexts = Arc::clone(&alpha_contexts);
                                async move {
                                    alpha_contexts
                                        .lock()
                                        .expect("alpha contexts")
                                        .push(context.clone());
                                    Ok(AlphaOutput {
                                        context: context.clone(),
                                        reply: format!("alpha:{}", input.0),
                                    })
                                }
                            }
                        })?;
                        Ok(Health::Healthy)
                    }
                },
            ),
            ComponentRuntimeDefinition::new(
                ComponentId::new(BETA_COMPONENT_ID).expect("component id"),
                {
                    let beta_scope = Arc::clone(&beta_scope);
                    let beta_notes_handle = Arc::clone(&beta_notes_handle);
                    let beta_contexts = Arc::clone(&beta_contexts);
                    let runtime_capture = Arc::clone(&runtime_capture);
                    move |scope: &ComponentRuntimeScope| {
                        let component_scope = scope.component_scope();
                        let rails = runtime_capture
                            .lock()
                            .expect("runtime capture")
                            .clone()
                            .expect("captured rails");
                        let beta_component = component(&rails.runtime, BETA_COMPONENT_ID);
                        let alpha_component = component(&rails.runtime, ALPHA_COMPONENT_ID);
                        let notes = rails
                            .communication
                            .bind_resolved(
                                beta_component,
                                notes_contract_key().id().clone(),
                                ComponentRequirementKind::Required,
                                rails.resolved_notes.clone(),
                            )
                            .expect("bind notes");
                        assert_eq!(notes.provider(), &alpha_component);
                        *beta_notes_handle.lock().expect("beta notes") = Some(notes.clone());
                        *beta_scope.lock().expect("beta scope") = Some(component_scope.clone());
                        scope.operation_with_context(beta_operation(), {
                            let component_scope = component_scope.clone();
                            let notes = notes.clone();
                            let beta_contexts = Arc::clone(&beta_contexts);
                            move |gateway_context, input| {
                                let component_scope = component_scope.clone();
                                let notes = notes.clone();
                                let beta_contexts = Arc::clone(&beta_contexts);
                                async move {
                                    beta_contexts
                                        .lock()
                                        .expect("beta contexts")
                                        .push(gateway_context.clone());
                                    let continued =
                                        component_scope.continue_invocation(&gateway_context)?;
                                    let alpha_notes =
                                        continued.call(&notes, |context, contract| {
                                            AlphaOutput {
                                                context: context.clone(),
                                                reply: contract.reply(context, input.0),
                                            }
                                        })?;
                                    let alpha_operation = continued
                                        .invoke(&alpha_operation(), AlphaInput(input.0))
                                        .await?;
                                    Ok(BetaOutput {
                                        gateway_context,
                                        continued_context: continued.context().clone(),
                                        alpha_context: alpha_notes.context,
                                        alpha_reply: alpha_notes.reply,
                                        alpha_operation_reply: alpha_operation.reply,
                                    })
                                }
                            }
                        })?;
                        Ok(Health::Healthy)
                    }
                },
            ),
            ComponentRuntimeDefinition::new(
                ComponentId::new(GAMMA_COMPONENT_ID).expect("component id"),
                {
                    let gamma_scope = Arc::clone(&gamma_scope);
                    let gamma_recipe_runs = Arc::clone(&gamma_recipe_runs);
                    move |scope: &ComponentRuntimeScope| {
                        gamma_recipe_runs.fetch_add(1, Ordering::SeqCst);
                        *gamma_scope.lock().expect("gamma scope") = Some(scope.component_scope());
                        scope.operation_with_context(
                            gamma_operation(),
                            |context, input| async move {
                                Ok(GammaOutput {
                                    context,
                                    reply: input.0,
                                })
                            },
                        )?;
                        Ok(Health::Healthy)
                    }
                },
            ),
            ComponentRuntimeDefinition::new(
                ComponentId::new(GUARD_COMPONENT_ID).expect("component id"),
                {
                    let guard_scope = Arc::clone(&guard_scope);
                    let guard_events = Arc::clone(&guard_events);
                    let runtime_capture = Arc::clone(&runtime_capture);
                    move |scope: &ComponentRuntimeScope| {
                        let rails = runtime_capture
                            .lock()
                            .expect("runtime capture")
                            .clone()
                            .expect("captured rails");
                        *guard_scope.lock().expect("guard scope") = Some(scope.component_scope());
                        rails
                            .steps
                            .register(
                                scope.participation().clone(),
                                GatewayStepId::new(GUARD_STEP_ID).expect("step id"),
                                GatewayStepPhase::Guard,
                                |call| call.operation_id().as_str() == BETA_OPERATION_ID,
                                {
                                    let guard_events = Arc::clone(&guard_events);
                                    move |call| {
                                        guard_events
                                            .lock()
                                            .expect("guard events")
                                            .push(call.context().clone());
                                        Ok(())
                                    }
                                },
                            )
                            .expect("register guard step");
                        Ok(Health::Healthy)
                    }
                },
            ),
        ]
    };

    let (mut composition_one, rails_one) = fixture(
        "runtime.shell_closure.home",
        &[BETA_COMPONENT_ID],
        build_runtime_definitions(Arc::clone(&runtime_capture)),
        None,
    );
    *runtime_capture.lock().expect("runtime capture") = Some(rails_one.clone());

    let alpha_component = component(&rails_one.runtime, ALPHA_COMPONENT_ID);
    let beta_component = component(&rails_one.runtime, BETA_COMPONENT_ID);
    let gamma_component = component(&rails_one.runtime, GAMMA_COMPONENT_ID);
    let guard_component = component(&rails_one.runtime, GUARD_COMPONENT_ID);

    rails_one
        .requirements
        .register_resolved(
            beta_component.clone(),
            alpha_required_contract_key().id().clone(),
            ComponentRequirementKind::Required,
            rails_one.resolved_alpha_required.clone(),
        )
        .expect("beta requires alpha");
    rails_one
        .requirements
        .register_resolved(
            beta_component.clone(),
            gamma_optional_contract_key().id().clone(),
            ComponentRequirementKind::Optional,
            rails_one.resolved_gamma_optional.clone(),
        )
        .expect("beta optional gamma");

    for component in [
        alpha_component.clone(),
        beta_component.clone(),
        gamma_component.clone(),
        guard_component.clone(),
    ] {
        rails_one
            .control
            .enable(component)
            .expect("enable component");
    }

    let first_report = rails_one.reconstruction.reconstruct().expect("reconstruct");
    let second_report = rails_one
        .reconstruction
        .reconstruct()
        .expect("reconstruct again");
    assert_eq!(
        first_report
            .outcomes()
            .iter()
            .map(|outcome| outcome.component_id().as_str())
            .collect::<Vec<_>>(),
        vec![
            ALPHA_COMPONENT_ID,
            BETA_COMPONENT_ID,
            GAMMA_COMPONENT_ID,
            GUARD_COMPONENT_ID,
        ]
    );
    for component_id in [
        ALPHA_COMPONENT_ID,
        BETA_COMPONENT_ID,
        GAMMA_COMPONENT_ID,
        GUARD_COMPONENT_ID,
    ] {
        assert!(matches!(
            report_outcome(first_report.outcomes(), component_id).result(),
            ComponentReconstructionResult::Materialized(status)
                if status.state() == ParticipationState::Active
        ));
        assert_eq!(
            report_outcome(second_report.outcomes(), component_id).result(),
            &ComponentReconstructionResult::AlreadyConverged
        );
    }
    assert_eq!(gamma_recipe_runs.load(Ordering::SeqCst), 1);

    let runtime_one_id = rails_one
        .runtime
        .current_status()
        .generation()
        .expect("runtime one generation");
    let alpha_status = rails_one
        .registry
        .component(alpha_component.component_id())
        .expect("alpha status")
        .clone();
    let beta_status = rails_one
        .registry
        .component(beta_component.component_id())
        .expect("beta status")
        .clone();
    let _gamma_status = rails_one
        .registry
        .component(gamma_component.component_id())
        .expect("gamma status")
        .clone();
    let _guard_status = rails_one
        .registry
        .component(guard_component.component_id())
        .expect("guard status")
        .clone();

    assert_eq!(
        rails_one
            .materializer
            .known_component_ids()
            .iter()
            .map(|component_id| component_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            ALPHA_COMPONENT_ID,
            BETA_COMPONENT_ID,
            GAMMA_COMPONENT_ID,
            GUARD_COMPONENT_ID,
        ]
    );
    assert_eq!(
        rails_one
            .readiness
            .aggregate_readiness()
            .status()
            .lifecycle(),
        ComponentRuntimeLifecycle::Ready
    );
    assert_eq!(
        rails_one.readiness.aggregate_readiness().status().health(),
        Health::Healthy
    );

    let descriptors = rails_one.operations.operations();
    assert_eq!(
        descriptors.iter().map(descriptor_ids).collect::<Vec<_>>(),
        vec![
            (
                ALPHA_OPERATION_ID,
                "fabric.test.shell_closure.alpha.input",
                "fabric.test.shell_closure.alpha.output",
                ALPHA_COMPONENT_ID,
            ),
            (
                BETA_OPERATION_ID,
                "fabric.test.shell_closure.beta.input",
                "fabric.test.shell_closure.beta.output",
                BETA_COMPONENT_ID,
            ),
            (
                GAMMA_OPERATION_ID,
                "fabric.test.shell_closure.gamma.input",
                "fabric.test.shell_closure.gamma.output",
                GAMMA_COMPONENT_ID,
            ),
        ]
    );

    let alpha_surface = rails_one
        .surfaces
        .register(
            alpha_component.clone(),
            SurfaceId::new("surface.alpha").expect("surface id"),
        )
        .expect("surface");
    let alpha_claim = rails_one
        .namespace
        .claim(
            alpha_component.clone(),
            NamespaceName::new("alpha-public").expect("name"),
        )
        .expect("claim");
    let alpha_publication = rails_one
        .publications
        .publish(alpha_claim.clone(), alpha_surface.clone())
        .expect("publish");
    assert_eq!(
        rails_one
            .gateway
            .entry(&NamespaceName::new("alpha-public").expect("name"))
            .expect("gateway entry")
            .publication(),
        &alpha_publication
    );

    let gateway_response: GatewayResponse<BetaOutput> = block_on(
        rails_one
            .gateway
            .invoke(GatewayRequest::new(beta_operation(), BetaInput("gateway"))),
    )
    .expect("gateway beta");
    let gateway_output = gateway_response.into_output();
    assert_eq!(
        gateway_output.gateway_context.root_origin(),
        &InvocationOrigin::External
    );
    assert_eq!(
        gateway_output.continued_context.root_origin(),
        &InvocationOrigin::External
    );
    assert_eq!(
        gateway_output.alpha_context.root_origin(),
        &InvocationOrigin::External
    );
    assert_eq!(
        gateway_output.gateway_context.generation(),
        gateway_output.continued_context.generation()
    );
    assert_eq!(
        gateway_output.gateway_context.invocation_id(),
        gateway_output.alpha_context.invocation_id()
    );
    assert_eq!(gateway_output.alpha_operation_reply, "alpha:gateway");
    assert_eq!(guard_events.lock().expect("guard events").len(), 1);

    let gamma_scope = gamma_scope
        .lock()
        .expect("gamma scope")
        .clone()
        .expect("gamma scope");
    let gamma_invocation = gamma_scope.begin_invocation().expect("gamma begin");
    let gamma_output = block_on(gamma_invocation.invoke(&gamma_operation(), GammaInput("gamma")))
        .expect("gamma operation");
    let gamma_expected_origin = InvocationOrigin::Component(gamma_component.clone());
    assert_eq!(
        gamma_invocation.context().root_origin(),
        &gamma_expected_origin
    );
    assert_eq!(gamma_output.context.root_origin(), &gamma_expected_origin);
    assert_eq!(
        gamma_invocation.context().invocation_id(),
        gamma_output.context.invocation_id()
    );

    assert_eq!(
        rails_one
            .requirements
            .effective_availability(beta_component.component_id())
            .expect("beta availability"),
        ComponentEffectiveAvailability::Available
    );

    let old_alpha_scope = alpha_scope
        .lock()
        .expect("alpha scope")
        .clone()
        .expect("alpha scope");
    let old_alpha_invocation = old_alpha_scope.begin_invocation().expect("alpha root");

    old_alpha_scope
        .update_health(Health::Degraded)
        .expect("degrade alpha");
    assert_eq!(
        rails_one
            .requirements
            .effective_health(beta_component.component_id())
            .expect("beta health")
            .health(),
        Health::Degraded
    );
    assert_eq!(
        rails_one
            .readiness
            .aggregate_readiness()
            .status()
            .lifecycle(),
        ComponentRuntimeLifecycle::Degraded
    );
    assert_eq!(
        rails_one.readiness.aggregate_readiness().status().health(),
        Health::Degraded
    );
    assert_eq!(beta_status.health(), Health::Healthy);

    old_alpha_scope
        .update_health(Health::Unavailable)
        .expect("unavailable alpha");
    assert_eq!(
        rails_one
            .requirements
            .effective_health(beta_component.component_id())
            .expect("beta health unavailable")
            .health(),
        Health::Unavailable
    );
    assert_eq!(
        rails_one
            .readiness
            .aggregate_readiness()
            .status()
            .lifecycle(),
        ComponentRuntimeLifecycle::Degraded
    );
    assert_eq!(
        rails_one.readiness.aggregate_readiness().status().health(),
        Health::Unavailable
    );
    assert!(matches!(
        rails_one.readiness.aggregate_readiness().blockers(),
        [ComponentAggregateBlocker::RequiredComponentUnavailable { component_id, .. }]
            if component_id == beta_component.component_id()
    ));
    assert_eq!(rails_one.operations.operations(), descriptors);
    assert!(matches!(
        block_on(rails_one.gateway.invoke(GatewayRequest::new(
            beta_operation(),
            BetaInput("blocked"),
        ))),
        Err(GatewayError::Component(ComponentError::ComponentUnavailable(component_id)))
            if component_id == *alpha_component.component_id()
                || component_id == *beta_component.component_id()
    ));

    let gamma_still_works =
        block_on(gamma_invocation.invoke(&gamma_operation(), GammaInput("still-healthy")))
            .expect("gamma survives aggregate failure");
    assert_eq!(gamma_still_works.reply, "still-healthy");

    old_alpha_scope
        .update_health(Health::Healthy)
        .expect("recover alpha");
    assert_eq!(
        rails_one
            .readiness
            .aggregate_readiness()
            .status()
            .lifecycle(),
        ComponentRuntimeLifecycle::Ready
    );
    assert_eq!(
        rails_one.readiness.aggregate_readiness().status().health(),
        Health::Healthy
    );
    assert_eq!(
        rails_one
            .registry
            .component(alpha_component.component_id())
            .expect("alpha after recovery")
            .participation(),
        alpha_status.participation()
    );
    assert_eq!(rails_one.operations.operations(), descriptors);

    rails_one
        .control
        .disable(alpha_component.clone())
        .expect("disable alpha");
    assert_eq!(
        rails_one
            .registry
            .component(alpha_component.component_id())
            .expect("still active before reconstruct")
            .participation(),
        alpha_status.participation()
    );
    let disable_report = rails_one
        .reconstruction
        .reconstruct()
        .expect("disable reconstruct");
    assert!(matches!(
        report_outcome(disable_report.outcomes(), ALPHA_COMPONENT_ID).result(),
        ComponentReconstructionResult::Dematerialized(status)
            if status.participation() == alpha_status.participation()
    ));
    assert_eq!(
        rails_one.operations.operation(alpha_operation().id()),
        Err(ComponentError::UnknownOperation(
            alpha_operation().id().clone()
        ))
    );
    assert_eq!(
        rails_one
            .publications
            .publication(alpha_publication.claim().name())
            .expect("publication survives")
            .claim()
            .owner(),
        &alpha_component
    );
    assert_eq!(
        rails_one.readiness.aggregate_readiness().status().health(),
        Health::Unavailable
    );
    assert!(matches!(
        rails_one
            .requirements
            .effective_availability(beta_component.component_id())
            .expect("beta unavailable without alpha"),
        ComponentEffectiveAvailability::RequiredDependencyUnavailable(_)
    ));

    rails_one
        .control
        .enable(alpha_component.clone())
        .expect("re-enable alpha");
    assert_eq!(
        rails_one.registry.component(alpha_component.component_id()),
        Err(ComponentError::UnknownComponent(
            alpha_component.component_id().clone()
        ))
    );
    let reenable_report = rails_one
        .reconstruction
        .reconstruct()
        .expect("re-enable reconstruct");
    let reenabled_alpha =
        match report_outcome(reenable_report.outcomes(), ALPHA_COMPONENT_ID).result() {
            ComponentReconstructionResult::Materialized(status) => status.clone(),
            other => panic!("expected alpha rematerialization, got {other:?}"),
        };
    assert_ne!(
        reenabled_alpha.participation(),
        alpha_status.participation()
    );
    assert_eq!(reenabled_alpha.component(), &alpha_component);
    assert_eq!(
        rails_one
            .requirements
            .requirements(beta_component.component_id())
            .len(),
        3
    );
    assert_eq!(
        descriptor_ids(
            &rails_one
                .operations
                .operation(alpha_operation().id())
                .expect("alpha descriptor restored")
        ),
        (
            ALPHA_OPERATION_ID,
            "fabric.test.shell_closure.alpha.input",
            "fabric.test.shell_closure.alpha.output",
            ALPHA_COMPONENT_ID,
        )
    );
    assert_eq!(
        rails_one.readiness.aggregate_readiness().status().health(),
        Health::Healthy
    );

    assert!(matches!(
        old_alpha_scope.begin_invocation(),
        Err(ComponentError::InvocationOriginNotParticipating(_))
            | Err(ComponentError::ComponentUnavailable(_))
    ));
    assert!(matches!(
        old_alpha_scope.update_health(Health::Degraded),
        Err(ComponentError::UnknownComponent(_))
            | Err(ComponentError::StaleComponentParticipation(_))
    ));
    assert!(matches!(
        old_alpha_scope.continue_invocation(old_alpha_invocation.context()),
        Err(ComponentError::InvocationOriginNotParticipating(_))
            | Err(ComponentError::ComponentUnavailable(_))
    ));

    let snapshot = rails_one.control.snapshot();
    assert_eq!(
        snapshot
            .entries()
            .iter()
            .map(|entry| (entry.component_id().as_str(), entry.desired()))
            .collect::<Vec<_>>(),
        vec![
            (ALPHA_COMPONENT_ID, ComponentDesiredState::Enabled),
            (BETA_COMPONENT_ID, ComponentDesiredState::Enabled),
            (GAMMA_COMPONENT_ID, ComponentDesiredState::Enabled),
            (GUARD_COMPONENT_ID, ComponentDesiredState::Enabled),
        ]
    );

    let runtime_capture_two = Arc::new(Mutex::new(None::<Rails>));
    let (mut composition_two, rails_two) = fixture(
        "runtime.shell_closure.home",
        &[BETA_COMPONENT_ID],
        build_runtime_definitions(Arc::clone(&runtime_capture_two)),
        Some(snapshot.clone()),
    );
    *runtime_capture_two.lock().expect("runtime capture two") = Some(rails_two.clone());
    let runtime_two_id = rails_two
        .runtime
        .current_status()
        .generation()
        .expect("runtime two generation");
    assert_ne!(runtime_one_id, runtime_two_id);
    assert_eq!(rails_two.control.snapshot(), snapshot);
    assert_eq!(rails_two.operations.operations(), Vec::new());
    assert_eq!(
        rails_two.readiness.aggregate_readiness().status().health(),
        Health::Unavailable
    );
    let rails_two_report = rails_two
        .reconstruction
        .reconstruct()
        .expect("r2 reconstruct");
    for component_id in [
        ALPHA_COMPONENT_ID,
        BETA_COMPONENT_ID,
        GAMMA_COMPONENT_ID,
        GUARD_COMPONENT_ID,
    ] {
        assert!(matches!(
            report_outcome(rails_two_report.outcomes(), component_id).result(),
            ComponentReconstructionResult::Materialized(status)
                if status.state() == ParticipationState::Active
        ));
    }
    assert_eq!(
        rails_two
            .operations
            .operations()
            .iter()
            .map(descriptor_ids)
            .collect::<Vec<_>>(),
        descriptors.iter().map(descriptor_ids).collect::<Vec<_>>()
    );
    assert_eq!(
        rails_two.readiness.aggregate_readiness().status().health(),
        Health::Healthy
    );
    assert_eq!(
        rails_two
            .registry
            .component(alpha_component.component_id())
            .expect("r2 alpha")
            .component(),
        &alpha_component
    );
    assert!(matches!(
        block_on(rails_two.operations.invoke_with_context(
            old_alpha_invocation.context().clone(),
            &alpha_operation(),
            AlphaInput("stale-runtime"),
        )),
        Err(ComponentError::InvocationContextGenerationMismatch { .. })
    ));
    assert!(matches!(
        old_alpha_scope.continue_invocation(
            &rails_two
                .invocation
                .begin_external()
                .expect("r2 gateway root"),
        ),
        Err(ComponentError::InvocationContextGenerationMismatch { .. })
    ));

    composition_one.stop();
    composition_two.stop();
}
