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
    Component, ComponentControlRail, ComponentDesiredState, ComponentError, ComponentId,
    ComponentMaterializer, ComponentReadinessPolicy, ComponentReadinessRail, ComponentRegistry,
    ComponentRequirementKind, ComponentRequirementRail, ComponentRuntime,
    ComponentRuntimeDefinition, ComponentRuntimeFailurePhase, ComponentRuntimeLifecycle,
    ComponentRuntimeModule, ComponentRuntimeScope, InvocationRail, OperationId, OperationKey,
    OperationRail, ProvidedComponentContract, SurfaceId, SurfaceRegistry,
};
use fabric_component_gateway::{Gateway, GatewayModule};
use fabric_component_namespace::{Namespace, NamespaceModule, NamespaceName};
use fabric_component_publication::PublicationContract;
use fabric_component_publication::PublicationModule;

const OPERATION_ID: &str = "fabric.component.runtime.host.echo";
const REQUIREMENT_CONTRACT_ID: &str = "fabric.component.runtime.host.requirement";

#[derive(Clone, Debug, PartialEq, Eq)]
struct EchoInput(&'static str);

#[derive(Clone, Debug, PartialEq, Eq)]
struct EchoOutput(&'static str);

struct Rails {
    runtime: Arc<ComponentRuntime>,
    registry: Arc<ComponentRegistry>,
    materializer: Arc<ComponentMaterializer>,
    control: Arc<ComponentControlRail>,
    readiness: Arc<ComponentReadinessRail>,
    requirements: Arc<ComponentRequirementRail>,
    invocation: Arc<InvocationRail>,
    operations: Arc<OperationRail>,
    gateway: Arc<Gateway>,
    surfaces: Arc<SurfaceRegistry>,
    namespace: Arc<Namespace>,
    publications: Arc<PublicationContract>,
    resolved_requirement: ResolvedContract<ProvidedComponentContract<()>>,
}

type Capture = Arc<Mutex<Option<Rails>>>;

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    runtime: ContractRequirement<ComponentRuntime>,
    registry: ContractRequirement<ComponentRegistry>,
    materializer: ContractRequirement<ComponentMaterializer>,
    control: ContractRequirement<ComponentControlRail>,
    readiness: ContractRequirement<ComponentReadinessRail>,
    requirements: ContractRequirement<ComponentRequirementRail>,
    invocation: ContractRequirement<InvocationRail>,
    operations: ContractRequirement<OperationRail>,
    gateway: ContractRequirement<Gateway>,
    surfaces: ContractRequirement<SurfaceRegistry>,
    namespace: ContractRequirement<Namespace>,
    publications: ContractRequirement<PublicationContract>,
    resolved_requirement: ContractRequirement<ProvidedComponentContract<()>>,
    capture: Capture,
}

impl CaptureModule {
    fn new(capture: Capture) -> Self {
        Self {
            module_id: ModuleId::new("runtime.runtime.host.capture").expect("module id"),
            runtime: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            materializer: ContractRequirement::provisional(
                fabric_component::component_materializer_contract_id(),
            ),
            control: ContractRequirement::provisional(
                fabric_component::component_control_contract_id(),
            ),
            readiness: ContractRequirement::provisional(
                fabric_component::component_readiness_contract_id(),
            ),
            requirements: ContractRequirement::provisional(
                fabric_component::component_requirement_contract_id(),
            ),
            invocation: ContractRequirement::provisional(fabric_component::invocation_contract_id()),
            operations: ContractRequirement::provisional(
                fabric_component::operation_rail_contract_id(),
            ),
            gateway: ContractRequirement::provisional(
                fabric_component_gateway::gateway_contract_id(),
            ),
            surfaces: ContractRequirement::provisional(fabric_component::surface_contract_id()),
            namespace: ContractRequirement::provisional(
                fabric_component_namespace::namespace_contract_id(),
            ),
            publications: ContractRequirement::provisional(
                fabric_component_publication::publication_contract_id(),
            ),
            resolved_requirement: ContractRequirement::provisional(
                requirement_contract_key().id().clone(),
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
            self.control.id().clone(),
            self.readiness.id().clone(),
            self.requirements.id().clone(),
            self.invocation.id().clone(),
            self.operations.id().clone(),
            self.gateway.id().clone(),
            self.surfaces.id().clone(),
            self.namespace.id().clone(),
            self.publications.id().clone(),
            self.resolved_requirement.id().clone(),
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
            control: bindings.resolve(&self.control).map_err(module_error)?,
            readiness: bindings.resolve(&self.readiness).map_err(module_error)?,
            requirements: bindings.resolve(&self.requirements).map_err(module_error)?,
            invocation: bindings.resolve(&self.invocation).map_err(module_error)?,
            operations: bindings.resolve(&self.operations).map_err(module_error)?,
            gateway: bindings.resolve(&self.gateway).map_err(module_error)?,
            surfaces: bindings.resolve(&self.surfaces).map_err(module_error)?,
            namespace: bindings.resolve(&self.namespace).map_err(module_error)?,
            publications: bindings.resolve(&self.publications).map_err(module_error)?,
            resolved_requirement: bindings
                .resolve_with_provider(&self.resolved_requirement)
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

fn definition(
    component_id: &str,
    prepare: impl Fn(&ComponentRuntimeScope) -> Result<Health, ComponentError> + Send + Sync + 'static,
) -> ComponentRuntimeDefinition {
    ComponentRuntimeDefinition::new(
        ComponentId::new(component_id).expect("component id"),
        prepare,
    )
}

fn requirement_contract_key() -> fabric_core::ContractKey<ProvidedComponentContract<()>> {
    provided_component_contract_key(REQUIREMENT_CONTRACT_ID)
}

fn operation_key() -> OperationKey<EchoInput, EchoOutput> {
    OperationKey::new(
        OperationId::new(OPERATION_ID).expect("operation id"),
        fabric_component::OperationTypeId::new("fabric.test.runtime_host.echo.input")
            .expect("input type id"),
        fabric_component::OperationTypeId::new("fabric.test.runtime_host.echo.output")
            .expect("output type id"),
    )
}

fn fixture(
    required_components: &[&str],
    runtime_definitions: impl IntoIterator<Item = ComponentRuntimeDefinition>,
) -> (Instance, Rails) {
    fixture_for_instance(
        "runtime.runtime.host",
        required_components,
        runtime_definitions,
    )
}

fn fixture_for_instance(
    instance_id: &str,
    required_components: &[&str],
    runtime_definitions: impl IntoIterator<Item = ComponentRuntimeDefinition>,
) -> (Instance, Rails) {
    let capture = Arc::new(Mutex::new(None));
    let policy = ComponentReadinessPolicy::new(
        required_components
            .iter()
            .map(|component_id| ComponentId::new(*component_id).expect("required component id")),
    )
    .expect("policy");
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.runtime.host").expect("composition"))
            .register_block(
                BlockBuilder::new(BlockId::new("runtime.runtime.host.block").expect("block"))
                    .register_module(
                        ComponentRuntimeModule::with_configuration(policy, runtime_definitions)
                            .expect("component runtime module"),
                    )
                    .register_module(ProvidedContractModule::new(
                        "runtime.runtime.host.requirement.provider",
                        "component.b",
                        REQUIREMENT_CONTRACT_ID,
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
    instance.start().expect("start instance");
    let rails = capture.lock().expect("capture lock").take().expect("rails");
    (instance, rails)
}

#[test]
fn known_and_unknown_runtime_definitions_are_reported_explicitly() {
    let (mut instance, rails) = fixture(
        &[],
        [
            definition("component.b", |_| Ok(Health::Healthy)),
            definition("component.a", |_| Ok(Health::Healthy)),
        ],
    );

    assert_eq!(
        rails.materializer.known_component_ids(),
        vec![
            ComponentId::new("component.a").expect("component id"),
            ComponentId::new("component.b").expect("component id"),
        ]
    );
    assert_eq!(
        rails
            .materializer
            .materialize(&ComponentId::new("component.unknown").expect("component id")),
        Err(ComponentError::UnknownComponentRuntimeDefinition(
            ComponentId::new("component.unknown").expect("component id"),
        ))
    );
    instance.stop();
}

#[test]
fn same_runtime_definition_id_binds_to_each_instance() {
    let (mut composition_a, rails_a) = fixture_for_instance(
        "runtime.runtime.host.a",
        &[],
        [definition("component.shared", |_| Ok(Health::Healthy))],
    );
    let (mut composition_b, rails_b) = fixture_for_instance(
        "runtime.runtime.host.b",
        &[],
        [definition("component.shared", |_| Ok(Health::Healthy))],
    );

    let status_a = rails_a
        .materializer
        .materialize(&ComponentId::new("component.shared").expect("component id"))
        .expect("materialize a");
    let status_b = rails_b
        .materializer
        .materialize(&ComponentId::new("component.shared").expect("component id"))
        .expect("materialize b");

    assert_eq!(
        status_a.component().instance_id(),
        &InstanceId::new("runtime.runtime.host.a").expect("instance id"),
    );
    assert_eq!(
        status_b.component().instance_id(),
        &InstanceId::new("runtime.runtime.host.b").expect("instance id"),
    );
    assert_eq!(
        status_a.component().component_id(),
        status_b.component().component_id()
    );
    assert_ne!(
        status_a.component().instance_id(),
        status_b.component().instance_id()
    );

    composition_a.stop();
    composition_b.stop();
}

#[test]
fn configuration_rejects_duplicate_runtime_definition_ids() {
    let duplicate = ComponentRuntimeModule::with_configuration(
        ComponentReadinessPolicy::empty(),
        [
            definition("component.a", |_| Ok(Health::Healthy)),
            definition("component.a", |_| Ok(Health::Healthy)),
        ],
    );
    assert_eq!(
        duplicate.err().expect("duplicate definitions rejected"),
        ComponentError::DuplicateComponentRuntimeDefinition(
            ComponentId::new("component.a").expect("component id")
        )
    );
}

#[test]
fn materialize_activates_coherent_runtime_and_generic_operations() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mut composition, rails) = fixture(
        &["component.a"],
        [definition("component.a", {
            let events = Arc::clone(&events);
            move |scope| {
                let dispatch_events = Arc::clone(&events);
                scope.operation(operation_key(), move |input: EchoInput| {
                    let events = Arc::clone(&dispatch_events);
                    async move {
                        events.lock().expect("events").push(input.0);
                        Ok(EchoOutput(input.0))
                    }
                })?;
                Ok(Health::Healthy)
            }
        })],
    );

    let status = rails
        .materializer
        .materialize(&ComponentId::new("component.a").expect("component id"))
        .expect("materialize component");
    assert!(status.is_active());
    assert_eq!(status.health(), Health::Healthy);
    assert_eq!(
        rails.readiness.aggregate_readiness().status().lifecycle(),
        ComponentRuntimeLifecycle::Ready
    );

    let response = block_on(rails.operations.invoke_with_context(
        rails.invocation.begin_external().expect("external root"),
        &operation_key(),
        EchoInput("ok"),
    ))
    .expect("operation invoke");
    assert_eq!(response, EchoOutput("ok"));
    assert_eq!(*events.lock().expect("events"), vec!["ok"]);
    composition.stop();
}

#[test]
fn recipe_cannot_make_staged_runtime_visible_before_activation() {
    let operations_capture: Arc<Mutex<Option<Arc<OperationRail>>>> = Arc::new(Mutex::new(None));
    let invocation_capture: Arc<Mutex<Option<Arc<InvocationRail>>>> = Arc::new(Mutex::new(None));
    let (mut composition, rails) = fixture(
        &[],
        [definition("component.a", {
            let operations_capture = Arc::clone(&operations_capture);
            let invocation_capture = Arc::clone(&invocation_capture);
            move |scope| {
                scope.operation(operation_key(), |input: EchoInput| async move {
                    Ok(EchoOutput(input.0))
                })?;
                let operations = operations_capture
                    .lock()
                    .expect("operations capture")
                    .clone()
                    .expect("operations");
                let invocation = invocation_capture
                    .lock()
                    .expect("invocation capture")
                    .clone()
                    .expect("invocation");
                assert_eq!(
                    block_on(operations.invoke_with_context(
                        invocation.begin_external().expect("gateway root"),
                        &operation_key(),
                        EchoInput("blocked"),
                    )),
                    Err(ComponentError::ComponentUnavailable(
                        ComponentId::new("component.a").expect("component id")
                    ))
                );
                Ok(Health::Healthy)
            }
        })],
    );
    *operations_capture.lock().expect("operations capture") = Some(Arc::clone(&rails.operations));
    *invocation_capture.lock().expect("invocation capture") = Some(Arc::clone(&rails.invocation));

    assert!(
        rails
            .materializer
            .materialize(&ComponentId::new("component.a").expect("component id"))
            .is_ok()
    );
    composition.stop();
}

#[test]
fn zero_contribution_and_unavailable_initial_health_materialize_cleanly() {
    let (mut composition, rails) = fixture(
        &[],
        [
            definition("component.zero", |_| Ok(Health::Healthy)),
            definition("component.unavailable", |_| Ok(Health::Unavailable)),
        ],
    );

    let zero = rails
        .materializer
        .materialize(&ComponentId::new("component.zero").expect("component id"))
        .expect("materialize zero");
    assert!(zero.is_active());
    assert_eq!(zero.health(), Health::Healthy);

    let unavailable = rails
        .materializer
        .materialize(&ComponentId::new("component.unavailable").expect("component id"))
        .expect("materialize unavailable");
    assert!(unavailable.is_active());
    assert_eq!(unavailable.health(), Health::Unavailable);
    assert_eq!(
        rails
            .registry
            .component(&ComponentId::new("component.unavailable").expect("component id")),
        Ok(unavailable)
    );
    composition.stop();
}

#[test]
fn failed_preparation_rolls_back_cleanly_and_can_retry() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let (mut composition, rails) = fixture(
        &["component.a"],
        [definition("component.a", {
            let attempts = Arc::clone(&attempts);
            move |scope| {
                attempts.fetch_add(1, Ordering::SeqCst);
                scope.operation(operation_key(), |input: EchoInput| async move {
                    Ok(EchoOutput(input.0))
                })?;
                if attempts.load(Ordering::SeqCst) == 1 {
                    return Err(ComponentError::Unavailable);
                }
                Ok(Health::Healthy)
            }
        })],
    );

    assert_eq!(
        rails
            .materializer
            .materialize(&ComponentId::new("component.a").expect("component id")),
        Err(ComponentError::ComponentRuntimeMaterializationFailed {
            component_id: ComponentId::new("component.a").expect("component id"),
            phase: ComponentRuntimeFailurePhase::Prepare,
        })
    );
    assert_eq!(
        rails
            .registry
            .component(&ComponentId::new("component.a").expect("component id")),
        Err(ComponentError::UnknownComponent(
            ComponentId::new("component.a").expect("component id")
        ))
    );
    assert_eq!(
        block_on(rails.operations.invoke_with_context(
            rails.invocation.begin_external().expect("gateway root"),
            &operation_key(),
            EchoInput("no"),
        )),
        Err(ComponentError::UnknownOperation(
            OperationId::new(OPERATION_ID).expect("operation id")
        ))
    );
    assert_eq!(
        rails.readiness.aggregate_readiness().status().health(),
        Health::Unavailable
    );

    let recovered = rails
        .materializer
        .materialize(&ComponentId::new("component.a").expect("component id"))
        .expect("retry materialize");
    assert!(recovered.is_active());
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    composition.stop();
}

#[test]
fn dematerialize_and_restart_use_fresh_participations_and_retain_scope_fencing() {
    let retained_scope: Arc<Mutex<Option<ComponentRuntimeScope>>> = Arc::new(Mutex::new(None));
    let starts = Arc::new(AtomicUsize::new(0));
    let (mut composition, rails) = fixture(
        &[],
        [definition("component.a", {
            let retained_scope = Arc::clone(&retained_scope);
            let starts = Arc::clone(&starts);
            move |scope| {
                starts.fetch_add(1, Ordering::SeqCst);
                *retained_scope.lock().expect("retained scope") = Some(scope.clone());
                scope.operation(operation_key(), |input: EchoInput| async move {
                    Ok(EchoOutput(input.0))
                })?;
                Ok(Health::Healthy)
            }
        })],
    );

    let first = rails
        .materializer
        .materialize(&ComponentId::new("component.a").expect("component id"))
        .expect("first materialize");
    let first_participation = first.participation().clone();
    let stopped = rails
        .materializer
        .dematerialize(&ComponentId::new("component.a").expect("component id"))
        .expect("dematerialize");
    assert_eq!(stopped.participation(), &first_participation);
    assert_eq!(
        rails
            .registry
            .component(&ComponentId::new("component.a").expect("component id")),
        Err(ComponentError::UnknownComponent(
            ComponentId::new("component.a").expect("component id")
        ))
    );

    let retained = retained_scope
        .lock()
        .expect("retained scope")
        .clone()
        .expect("retained scope");
    assert!(matches!(
        retained.operation(
            OperationKey::new(
                OperationId::new("fabric.component.runtime.host.extra").expect("operation id"),
                fabric_component::OperationTypeId::new("fabric.test.runtime_host.extra.input")
                    .expect("input type id"),
                fabric_component::OperationTypeId::new("fabric.test.runtime_host.extra.output")
                    .expect("output type id"),
            ),
            |input: EchoInput| async move { Ok(EchoOutput(input.0)) },
        ),
        Err(ComponentError::OperationOwnerNotParticipating { .. })
    ));

    let second = rails
        .materializer
        .materialize(&ComponentId::new("component.a").expect("component id"))
        .expect("second materialize");
    assert_ne!(first.participation(), second.participation());
    assert_eq!(starts.load(Ordering::SeqCst), 2);
    composition.stop();
}

#[test]
fn dependency_readiness_and_desired_state_remain_separate() {
    let (mut composition, rails) = fixture(
        &["component.a"],
        [
            definition("component.a", |_| Ok(Health::Healthy)),
            definition("component.b", |_| Ok(Health::Healthy)),
        ],
    );
    let a = component(rails.runtime.as_ref(), "component.a");
    let b = component(rails.runtime.as_ref(), "component.b");
    rails
        .requirements
        .register_resolved(
            a.clone(),
            requirement_contract_key().id().clone(),
            ComponentRequirementKind::Required,
            rails.resolved_requirement.clone(),
        )
        .expect("register requirement");
    assert_eq!(
        rails
            .requirements
            .requirements(a.component_id())
            .first()
            .expect("registered requirement")
            .provider(),
        &b
    );
    rails
        .control
        .set_desired(a.clone(), ComponentDesiredState::Enabled)
        .expect("set desired enabled");

    assert_eq!(
        rails.readiness.aggregate_readiness().status().health(),
        Health::Unavailable
    );
    let a_status = rails
        .materializer
        .materialize(&ComponentId::new("component.a").expect("component id"))
        .expect("materialize a");
    assert!(a_status.is_active());
    assert_eq!(
        rails.readiness.aggregate_readiness().status().health(),
        Health::Unavailable
    );
    let desired = rails
        .control
        .control(a.component_id())
        .expect("control state");
    assert_eq!(desired.desired(), ComponentDesiredState::Enabled);

    let b_status = rails
        .materializer
        .materialize(&ComponentId::new("component.b").expect("component id"))
        .expect("materialize b");
    assert!(b_status.is_active());
    assert_eq!(
        rails.readiness.aggregate_readiness().status().lifecycle(),
        ComponentRuntimeLifecycle::Ready
    );
    rails
        .materializer
        .dematerialize(&ComponentId::new("component.a").expect("component id"))
        .expect("dematerialize a");
    assert_eq!(
        rails.readiness.aggregate_readiness().status().health(),
        Health::Unavailable
    );
    assert_eq!(
        rails
            .control
            .control(&ComponentId::new("component.a").expect("component id"))
            .expect("control state")
            .desired(),
        ComponentDesiredState::Enabled
    );
    composition.stop();
}

#[test]
fn semantic_publication_truth_survives_runtime_dematerialization() {
    let (mut composition, rails) =
        fixture(&[], [definition("component.a", |_| Ok(Health::Healthy))]);
    let a = rails
        .materializer
        .materialize(&ComponentId::new("component.a").expect("component id"))
        .expect("materialize component.a")
        .component()
        .clone();
    let surface = rails
        .surfaces
        .register(a.clone(), SurfaceId::new("surface.a").expect("surface id"))
        .expect("register surface");
    let name = rails
        .namespace
        .claim(a.clone(), NamespaceName::new("name.a").expect("name"))
        .expect("claim name");
    rails
        .publications
        .publish(name.clone(), surface.clone())
        .expect("publish");

    rails
        .materializer
        .dematerialize(&ComponentId::new("component.a").expect("component id"))
        .expect("dematerialize");

    assert!(
        rails
            .gateway
            .entry(&NamespaceName::new("name.a").expect("name"))
            .is_ok()
    );
    assert_eq!(
        rails
            .requirements
            .requirements(&ComponentId::new("component.a").expect("component id"))
            .len(),
        0
    );
    composition.stop();
}
