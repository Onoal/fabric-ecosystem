use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};

use fabric_core::{
    BlockBuilder, CompositionBuilder, CompositionError, CompositionId, ContractId, ContractKey,
    ContractRequirement, Health, InstanceGeneration, InstanceId, ModuleBindings, ModuleContract,
    ModuleError, ModuleId, ModuleRuntime,
};

use fabric_component::{
    Component, ComponentId, ComponentRegistry, ComponentRuntime, ComponentRuntimeModule,
    InvocationRail, OperationId, OperationKey, OperationRail, OperationRegistrar,
};
use fabric_component_gateway::{Gateway, GatewayModule, GatewayRequest, GatewayResponse};
use fabric_component_namespace::NamespaceModule;
use fabric_component_publication::PublicationModule;

const EXAMPLE_COMPONENT_CONTRACT_ID: &str = "fabric.component.example";
const EXAMPLE_ECHO_OPERATION_ID: &str = "example.echo";
const EXAMPLE_NESTED_INNER_OPERATION_ID: &str = "example.nested.inner";
const EXAMPLE_NESTED_OUTER_OPERATION_ID: &str = "example.nested.outer";
const EXAMPLE_UNKNOWN_OPERATION_ID: &str = "example.unknown";

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExampleComponentSnapshot {
    provider_component: Component,
}

trait ExampleComponentRuntimeService: Send + Sync {
    fn provider_component(&self) -> Component;
}

#[derive(Clone)]
struct ExampleComponentRuntime {
    inner: Arc<dyn ExampleComponentRuntimeService>,
}

impl ExampleComponentRuntime {
    fn new(inner: Arc<dyn ExampleComponentRuntimeService>) -> Self {
        Self { inner }
    }

    fn snapshot(&self) -> ExampleComponentSnapshot {
        ExampleComponentSnapshot {
            provider_component: self.inner.provider_component(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConsumerObservation {
    consumer_component: Component,
    provider_component: Component,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EchoInput {
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EchoOutput {
    value: String,
    owner_component_id: ComponentId,
}

type CapturedOperationRail = Arc<
    Mutex<
        Option<(
            InstanceId,
            InstanceGeneration,
            Arc<OperationRail>,
            Arc<InvocationRail>,
        )>,
    >,
>;
type CapturedOperationRegistrar = Arc<Mutex<Option<(InstanceId, Arc<OperationRegistrar>)>>>;
type CapturedGateway = Arc<Mutex<Option<(InstanceId, InstanceGeneration, Arc<Gateway>)>>>;
type CapturedParticipation = Arc<Mutex<Option<fabric_component::ComponentParticipation>>>;

#[derive(Clone)]
struct ComponentParticipantModule {
    module_id: ModuleId,
    component_id: ComponentId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    component: Option<Component>,
    health: Health,
}

impl ComponentParticipantModule {
    fn new(module_id: &str, component_id: &str) -> Self {
        Self {
            module_id: ModuleId::new(module_id).expect("module id"),
            component_id: ComponentId::new(component_id).expect("component id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            component: None,
            health: Health::Degraded,
        }
    }
}

impl ModuleRuntime for ComponentParticipantModule {
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
        vec![self.runtime_requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let runtime_contract = bindings
            .resolve(&self.runtime_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        self.component = Some(Component::bind(
            self.component_id.clone(),
            runtime_contract.as_ref(),
        ));
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        self.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        self.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.health
    }
}

struct ProducerSharedState {
    inner: Mutex<ProducerState>,
}

struct ProducerState {
    started: bool,
    component: Option<Component>,
}

#[derive(Clone)]
struct ExampleProducerComponent {
    module_id: ModuleId,
    component_id: ComponentId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    shared: Arc<ProducerSharedState>,
}

impl ExampleProducerComponent {
    fn new(module_id: &str, component_id: &str) -> Self {
        Self {
            module_id: ModuleId::new(module_id).expect("module id"),
            component_id: ComponentId::new(component_id).expect("component id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            shared: Arc::new(ProducerSharedState {
                inner: Mutex::new(ProducerState {
                    started: false,
                    component: None,
                }),
            }),
        }
    }
}

impl ModuleRuntime for ExampleProducerComponent {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![example_component_runtime_contract_key().id().clone()]
            .into_iter()
            .map(fabric_core::ProvidedContractDeclaration::provisional)
            .collect()
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.runtime_requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        let service: Arc<dyn ExampleComponentRuntimeService> = self.shared.clone();
        Ok(vec![ModuleContract::new(
            &example_component_runtime_contract_key(),
            Arc::new(ExampleComponentRuntime::new(service)),
        )])
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let runtime_contract = bindings
            .resolve(&self.runtime_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let component = Component::bind(self.component_id.clone(), runtime_contract.as_ref());
        self.shared
            .inner
            .lock()
            .expect("producer state lock")
            .component = Some(component);
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        let mut state = self.shared.inner.lock().expect("producer state lock");
        state.started = false;
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        self.shared
            .inner
            .lock()
            .expect("producer state lock")
            .started = true;
        Ok(())
    }

    fn stop(&mut self) {
        self.shared
            .inner
            .lock()
            .expect("producer state lock")
            .started = false;
    }

    fn health(&self) -> Health {
        if self
            .shared
            .inner
            .lock()
            .expect("producer state lock")
            .started
        {
            Health::Healthy
        } else {
            Health::Unavailable
        }
    }
}

impl ExampleComponentRuntimeService for ProducerSharedState {
    fn provider_component(&self) -> Component {
        self.inner
            .lock()
            .expect("producer state lock")
            .component
            .clone()
            .expect("component bound")
    }
}

#[derive(Clone)]
struct ExampleConsumerComponent {
    module_id: ModuleId,
    component_id: ComponentId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    example_requirement: ContractRequirement<ExampleComponentRuntime>,
    capture: Arc<Mutex<Option<ConsumerObservation>>>,
    health: Health,
}

impl ExampleConsumerComponent {
    fn new(
        module_id: &str,
        component_id: &str,
        capture: Arc<Mutex<Option<ConsumerObservation>>>,
    ) -> Self {
        Self {
            module_id: ModuleId::new(module_id).expect("module id"),
            component_id: ComponentId::new(component_id).expect("component id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            example_requirement: ContractRequirement::provisional(
                example_component_runtime_contract_id(),
            ),
            capture,
            health: Health::Degraded,
        }
    }
}

#[derive(Clone)]
struct ExampleOperationProviderComponent {
    module_id: ModuleId,
    component_id: ComponentId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registry_requirement: ContractRequirement<ComponentRegistry>,
    registrar_requirement: ContractRequirement<OperationRegistrar>,
    component: Option<Component>,
    registry: Option<Arc<ComponentRegistry>>,
    registrar: Option<Arc<OperationRegistrar>>,
    participation: Option<fabric_component::ComponentParticipation>,
    health: Health,
}

impl ExampleOperationProviderComponent {
    fn new(module_id: &str, component_id: &str) -> Self {
        Self {
            module_id: ModuleId::new(module_id).expect("module id"),
            component_id: ComponentId::new(component_id).expect("component id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry_requirement: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            registrar_requirement: ContractRequirement::provisional(
                fabric_component::operation_registrar_contract_id(),
            ),
            component: None,
            registry: None,
            registrar: None,
            participation: None,
            health: Health::Degraded,
        }
    }
}

impl ModuleRuntime for ExampleOperationProviderComponent {
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
            self.registrar_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let runtime_contract = bindings
            .resolve(&self.runtime_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let registrar = bindings
            .resolve(&self.registrar_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let registry = bindings
            .resolve(&self.registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        self.component = Some(Component::bind(
            self.component_id.clone(),
            runtime_contract.as_ref(),
        ));
        self.registry = Some(registry);
        self.registrar = Some(registrar);
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        let registry = self.registry.as_ref().expect("registry bound");
        let participation = registry
            .register(
                self.component.clone().expect("component bound"),
                Health::Degraded,
            )
            .map_err(|error| ModuleError::new(error.to_string()))?
            .participation()
            .clone();
        let component_id = self
            .component
            .as_ref()
            .expect("component bound")
            .component_id()
            .clone();
        self.registrar
            .as_ref()
            .expect("registrar bound")
            .register(
                participation.clone(),
                example_echo_operation_key(),
                move |input: EchoInput| {
                    let component_id = component_id.clone();
                    async move {
                        Ok(EchoOutput {
                            value: input.value,
                            owner_component_id: component_id,
                        })
                    }
                },
            )
            .map_err(|error| ModuleError::new(error.to_string()))?;
        registry
            .activate(&participation)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        self.participation = Some(participation);
        self.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        if let (Some(participation), Some(registry)) = (&self.participation, &self.registry) {
            let _ = registry.unregister(participation);
        }
        self.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.health
    }
}

#[derive(Clone)]
struct OperationRailCapture {
    module_id: ModuleId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    rail_requirement: ContractRequirement<OperationRail>,
    invocation_requirement: ContractRequirement<InvocationRail>,
    capture: CapturedOperationRail,
    runtime: Option<Arc<ComponentRuntime>>,
    rail: Option<Arc<OperationRail>>,
    invocation: Option<Arc<InvocationRail>>,
    health: Health,
}

impl OperationRailCapture {
    fn new(capture: CapturedOperationRail) -> Self {
        Self {
            module_id: ModuleId::new("runtime.operation.capture").expect("module id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            rail_requirement: ContractRequirement::provisional(
                fabric_component::operation_rail_contract_id(),
            ),
            invocation_requirement: ContractRequirement::provisional(
                fabric_component::invocation_contract_id(),
            ),
            capture,
            runtime: None,
            rail: None,
            invocation: None,
            health: Health::Degraded,
        }
    }
}

#[derive(Clone)]
struct ParticipationCaptureModule {
    module_id: ModuleId,
    component_id: ComponentId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registry_requirement: ContractRequirement<ComponentRegistry>,
    capture: CapturedParticipation,
    component: Option<Component>,
    registry: Option<Arc<ComponentRegistry>>,
    participation: Option<fabric_component::ComponentParticipation>,
}

impl ParticipationCaptureModule {
    fn new(module_id: &str, component_id: &str, capture: CapturedParticipation) -> Self {
        Self {
            module_id: ModuleId::new(module_id).expect("module id"),
            component_id: ComponentId::new(component_id).expect("component id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry_requirement: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            capture,
            component: None,
            registry: None,
            participation: None,
        }
    }
}

impl ModuleRuntime for ParticipationCaptureModule {
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
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let runtime_contract = bindings
            .resolve(&self.runtime_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let registry = bindings
            .resolve(&self.registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        self.component = Some(Component::bind(
            self.component_id.clone(),
            runtime_contract.as_ref(),
        ));
        self.registry = Some(registry);
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        let participation = self
            .registry
            .as_ref()
            .expect("registry bound")
            .register(
                self.component.clone().expect("component bound"),
                Health::Healthy,
            )
            .map_err(|error| ModuleError::new(error.to_string()))?
            .participation()
            .clone();
        *self.capture.lock().expect("participation capture") = Some(participation.clone());
        self.participation = Some(participation);
        Ok(())
    }

    fn stop(&mut self) {
        if let (Some(participation), Some(registry)) = (&self.participation, &self.registry) {
            let _ = registry.unregister(participation);
        }
    }

    fn health(&self) -> Health {
        Health::Healthy
    }
}

#[derive(Clone)]
struct GatewayCaptureModule {
    module_id: ModuleId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    gateway_requirement: ContractRequirement<Gateway>,
    capture: CapturedGateway,
    runtime: Option<Arc<ComponentRuntime>>,
    gateway: Option<Arc<Gateway>>,
    health: Health,
}

impl GatewayCaptureModule {
    fn new(capture: CapturedGateway) -> Self {
        Self {
            module_id: ModuleId::new("runtime.gateway.capture").expect("module id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            gateway_requirement: ContractRequirement::provisional(
                fabric_component_gateway::gateway_contract_id(),
            ),
            capture,
            runtime: None,
            gateway: None,
            health: Health::Degraded,
        }
    }
}

impl ModuleRuntime for GatewayCaptureModule {
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
            self.gateway_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        self.runtime = Some(
            bindings
                .resolve(&self.runtime_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?,
        );
        self.gateway = Some(
            bindings
                .resolve(&self.gateway_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?,
        );
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        let runtime_contract = self.runtime.as_ref().expect("runtime contract bound");
        let generation = runtime_contract
            .current_status()
            .generation()
            .expect("running runtime generation");
        *self.capture.lock().expect("runtime gateway capture lock") = Some((
            runtime_contract.instance_id(),
            generation,
            Arc::clone(self.gateway.as_ref().expect("runtime gateway bound")),
        ));
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        self.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        self.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.health
    }
}

#[derive(Clone)]
struct OperationRegistrarCapture {
    module_id: ModuleId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registrar_requirement: ContractRequirement<OperationRegistrar>,
    capture: CapturedOperationRegistrar,
    health: Health,
}

impl OperationRegistrarCapture {
    fn new(capture: CapturedOperationRegistrar) -> Self {
        Self {
            module_id: ModuleId::new("runtime.operation.registrar.capture").expect("module id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registrar_requirement: ContractRequirement::provisional(
                fabric_component::operation_registrar_contract_id(),
            ),
            capture,
            health: Health::Degraded,
        }
    }
}

impl ModuleRuntime for OperationRegistrarCapture {
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
            self.registrar_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let runtime_contract = bindings
            .resolve(&self.runtime_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let registrar = bindings
            .resolve(&self.registrar_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self
            .capture
            .lock()
            .expect("operation registrar capture lock") =
            Some((runtime_contract.instance_id(), registrar));
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        self.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        self.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.health
    }
}

#[derive(Clone)]
struct NestedOperationProviderComponent {
    module_id: ModuleId,
    component_id: ComponentId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registry_requirement: ContractRequirement<ComponentRegistry>,
    registrar_requirement: ContractRequirement<OperationRegistrar>,
    rail_requirement: ContractRequirement<OperationRail>,
    component: Option<Component>,
    registry: Option<Arc<ComponentRegistry>>,
    registrar: Option<Arc<OperationRegistrar>>,
    rail: Option<Arc<OperationRail>>,
    participation: Option<fabric_component::ComponentParticipation>,
    health: Health,
}

impl NestedOperationProviderComponent {
    fn new(module_id: &str, component_id: &str) -> Self {
        Self {
            module_id: ModuleId::new(module_id).expect("module id"),
            component_id: ComponentId::new(component_id).expect("component id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry_requirement: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            registrar_requirement: ContractRequirement::provisional(
                fabric_component::operation_registrar_contract_id(),
            ),
            rail_requirement: ContractRequirement::provisional(
                fabric_component::operation_rail_contract_id(),
            ),
            component: None,
            registry: None,
            registrar: None,
            rail: None,
            participation: None,
            health: Health::Degraded,
        }
    }
}

impl ModuleRuntime for NestedOperationProviderComponent {
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
            self.registrar_requirement.id().clone(),
            self.rail_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let runtime_contract = bindings
            .resolve(&self.runtime_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let registrar = bindings
            .resolve(&self.registrar_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let registry = bindings
            .resolve(&self.registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let rail = bindings
            .resolve(&self.rail_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        self.component = Some(Component::bind(
            self.component_id.clone(),
            runtime_contract.as_ref(),
        ));
        self.registry = Some(registry);
        self.registrar = Some(registrar);
        self.rail = Some(rail);
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        let registry = self.registry.as_ref().expect("registry bound");
        let participation = registry
            .register(
                self.component.clone().expect("component bound"),
                Health::Degraded,
            )
            .map_err(|error| ModuleError::new(error.to_string()))?
            .participation()
            .clone();
        let component_id = self
            .component
            .as_ref()
            .expect("component bound")
            .component_id()
            .clone();
        self.registrar
            .as_ref()
            .expect("registrar bound")
            .register(
                participation.clone(),
                nested_inner_operation_key(),
                move |input: EchoInput| {
                    let component_id = component_id.clone();
                    async move {
                        Ok(EchoOutput {
                            value: format!("inner:{}", input.value),
                            owner_component_id: component_id,
                        })
                    }
                },
            )
            .map_err(|error| ModuleError::new(error.to_string()))?;

        let component_id = self
            .component
            .as_ref()
            .expect("component bound")
            .component_id()
            .clone();
        let rail = Arc::clone(self.rail.as_ref().expect("operation rail bound"));
        self.registrar
            .as_ref()
            .expect("registrar bound")
            .register_with_context(
                participation.clone(),
                nested_outer_operation_key(),
                move |context, input: EchoInput| {
                    let rail = Arc::clone(&rail);
                    let component_id = component_id.clone();
                    async move {
                        let inner = rail
                            .invoke_with_context(
                                context,
                                &nested_inner_operation_key(),
                                EchoInput { value: input.value },
                            )
                            .await?;
                        Ok(EchoOutput {
                            value: format!("outer:{}:{}", component_id.as_str(), inner.value),
                            owner_component_id: component_id,
                        })
                    }
                },
            )
            .map_err(|error| ModuleError::new(error.to_string()))?;
        registry
            .activate(&participation)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        self.participation = Some(participation);
        self.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        if let (Some(participation), Some(registry)) = (&self.participation, &self.registry) {
            let _ = registry.unregister(participation);
        }
        self.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.health
    }
}

impl ModuleRuntime for OperationRailCapture {
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
            self.rail_requirement.id().clone(),
            self.invocation_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        self.runtime = Some(
            bindings
                .resolve(&self.runtime_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?,
        );
        self.rail = Some(
            bindings
                .resolve(&self.rail_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?,
        );
        self.invocation = Some(
            bindings
                .resolve(&self.invocation_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?,
        );
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        let runtime_contract = self.runtime.as_ref().expect("runtime contract bound");
        let generation = runtime_contract
            .current_status()
            .generation()
            .expect("running runtime generation");
        *self.capture.lock().expect("operation rail capture lock") = Some((
            runtime_contract.instance_id(),
            generation,
            Arc::clone(self.rail.as_ref().expect("operation rail bound")),
            Arc::clone(self.invocation.as_ref().expect("invocation rail bound")),
        ));
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        self.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        self.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.health
    }
}

impl ModuleRuntime for ExampleConsumerComponent {
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
            self.example_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let runtime_contract = bindings
            .resolve(&self.runtime_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let example_contract = bindings
            .resolve(&self.example_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let consumer_component =
            Component::bind(self.component_id.clone(), runtime_contract.as_ref());
        let provider_component = example_contract.snapshot().provider_component;
        *self.capture.lock().expect("consumer capture lock") = Some(ConsumerObservation {
            consumer_component,
            provider_component,
        });
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        self.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        self.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.health
    }
}

fn example_component_runtime_contract_id() -> ContractId {
    ContractId::new(EXAMPLE_COMPONENT_CONTRACT_ID).expect("static example component contract id")
}

fn example_component_runtime_contract_key() -> ContractKey<ExampleComponentRuntime> {
    ContractKey::provisional(example_component_runtime_contract_id())
}

fn example_echo_operation_key() -> OperationKey<EchoInput, EchoOutput> {
    OperationKey::new(
        OperationId::new(EXAMPLE_ECHO_OPERATION_ID).expect("operation id"),
        fabric_component::OperationTypeId::new("fabric.test.example.echo.input")
            .expect("input type id"),
        fabric_component::OperationTypeId::new("fabric.test.example.echo.output")
            .expect("output type id"),
    )
}

fn unknown_echo_operation_key() -> OperationKey<EchoInput, EchoOutput> {
    OperationKey::new(
        OperationId::new(EXAMPLE_UNKNOWN_OPERATION_ID).expect("operation id"),
        fabric_component::OperationTypeId::new("fabric.test.example.unknown.input")
            .expect("input type id"),
        fabric_component::OperationTypeId::new("fabric.test.example.unknown.output")
            .expect("output type id"),
    )
}

fn nested_inner_operation_key() -> OperationKey<EchoInput, EchoOutput> {
    OperationKey::new(
        OperationId::new(EXAMPLE_NESTED_INNER_OPERATION_ID).expect("operation id"),
        fabric_component::OperationTypeId::new("fabric.test.example.nested_inner.input")
            .expect("input type id"),
        fabric_component::OperationTypeId::new("fabric.test.example.nested_inner.output")
            .expect("output type id"),
    )
}

fn nested_outer_operation_key() -> OperationKey<EchoInput, EchoOutput> {
    OperationKey::new(
        OperationId::new(EXAMPLE_NESTED_OUTER_OPERATION_ID).expect("operation id"),
        fabric_component::OperationTypeId::new("fabric.test.example.nested_outer.input")
            .expect("input type id"),
        fabric_component::OperationTypeId::new("fabric.test.example.nested_outer.output")
            .expect("output type id"),
    )
}

struct Parker {
    ready: Mutex<bool>,
    cvar: Condvar,
}

impl Wake for Parker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        let mut ready = self.ready.lock().expect("parker lock");
        *ready = true;
        self.cvar.notify_one();
    }
}

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    let parker = Arc::new(Parker {
        ready: Mutex::new(false),
        cvar: Condvar::new(),
    });
    let waker = Waker::from(Arc::clone(&parker));
    let mut context = Context::from_waker(&waker);
    let mut future = Pin::from(Box::new(future));

    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => {
                let mut ready = parker.ready.lock().expect("parker lock");
                while !*ready {
                    ready = parker.cvar.wait(ready).expect("parker wait");
                }
                *ready = false;
            }
        }
    }
}

fn gateway_context(invocation: &InvocationRail) -> fabric_component::InvocationContext {
    invocation.begin_external().expect("gateway context")
}

fn materialize_instance(
    instance_id: &str,
    composition: fabric_core::Composition,
) -> fabric_core::Instance {
    composition
        .materialize(InstanceId::new(instance_id).expect("instance id"))
        .expect("materialize instance")
}

#[test]
fn component_runtime_module_composes_through_core_machinery() {
    let block = BlockBuilder::new(fabric_core::BlockId::new("runtime.block").expect("block"))
        .register_module(ComponentRuntimeModule::new())
        .build();

    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("composition"))
            .register_block(block)
            .build()
            .expect("composition");
    let mut instance = materialize_instance("runtime.alpha", composition);

    assert_eq!(
        instance.report().blocks[0].modules[0].module_id.as_str(),
        "fabric.component.runtime.module"
    );
    instance.start().expect("start");
    assert_eq!(instance.report().health, Health::Healthy);
}

#[test]
fn runtime_contract_resolves_configured_instance() {
    let block = BlockBuilder::new(fabric_core::BlockId::new("runtime.block").expect("block"))
        .register_module(ComponentRuntimeModule::new())
        .register_module(ComponentParticipantModule::new(
            "runtime.participant",
            "component.alpha",
        ))
        .build();
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("composition"))
            .register_block(block)
            .build()
            .expect("composition");
    let mut instance = materialize_instance("runtime.alpha", composition);

    instance.start().expect("start");
    assert_eq!(instance.report().health, Health::Healthy);
}

#[test]
fn stopping_runtime_module_makes_contract_unavailable() {
    let capture = Arc::new(Mutex::new(None));
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("composition"))
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.block").expect("block"))
                    .register_module(ComponentRuntimeModule::new())
                    .register_module(ComponentRuntimeCapture::new(Arc::clone(&capture)))
                    .build(),
            )
            .build()
            .expect("composition");
    let mut instance = materialize_instance("runtime.alpha", composition);
    instance.start().expect("start");
    let captured = capture
        .lock()
        .expect("runtime contract capture lock")
        .as_ref()
        .expect("captured runtime contract")
        .clone();
    instance.stop();

    assert_eq!(
        captured
            .current_instance_id()
            .expect_err("unavailable")
            .to_string(),
        "component runtime contract is unavailable"
    );
}

#[test]
fn reconstructing_with_same_instance_id_preserves_identity() {
    let first = started_instance_id("runtime.stable");
    let second = started_instance_id("runtime.stable");

    assert_eq!(first, second);
}

#[test]
fn composition_rejects_multiple_runtime_modules_without_any_consumer() {
    let first = BlockBuilder::new(fabric_core::BlockId::new("runtime.block.a").expect("block"))
        .register_module(ComponentRuntimeModule::new())
        .build();
    let second = BlockBuilder::new(fabric_core::BlockId::new("runtime.block.b").expect("block"))
        .register_module(ComponentRuntimeModule::new())
        .build();

    let error = CompositionBuilder::new(CompositionId::new("runtime.composition").expect("id"))
        .register_block(first)
        .register_block(second)
        .build()
        .expect_err("duplicate component runtime module id");

    match error {
        CompositionError::DuplicateModuleId { module_id } => {
            assert_eq!(module_id.as_str(), "fabric.component.runtime.module")
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn component_requires_component_runtime_module() {
    let error = CompositionBuilder::new(CompositionId::new("runtime.composition").expect("id"))
        .register_block(
            BlockBuilder::new(fabric_core::BlockId::new("runtime.block").expect("block"))
                .register_module(ComponentParticipantModule::new(
                    "runtime.participant",
                    "component.alpha",
                ))
                .build(),
        )
        .build()
        .expect_err("missing runtime provider");

    match error {
        CompositionError::MissingProvider {
            module_id,
            contract_id,
        } => {
            assert_eq!(module_id.as_str(), "runtime.participant");
            assert_eq!(
                contract_id,
                fabric_component::component_runtime_contract_id()
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn components_participate_in_the_same_instance() {
    let capture = Arc::new(Mutex::new(None));
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("composition"))
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.core").expect("block"))
                    .register_module(ComponentRuntimeModule::new())
                    .register_module(NamespaceModule::new())
                    .register_module(PublicationModule::new())
                    .register_module(GatewayModule::new())
                    .build(),
            )
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.producer").expect("block"))
                    .register_module(ExampleProducerComponent::new(
                        "runtime.producer.component",
                        "component.producer",
                    ))
                    .build(),
            )
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.consumer").expect("block"))
                    .register_module(ExampleConsumerComponent::new(
                        "runtime.consumer.component",
                        "component.consumer",
                        Arc::clone(&capture),
                    ))
                    .build(),
            )
            .build()
            .expect("composition");
    let mut instance = materialize_instance("runtime.alpha", composition);

    instance.start().expect("start");
    let observation = capture
        .lock()
        .expect("consumer capture lock")
        .clone()
        .expect("consumer observation");

    assert_eq!(
        observation.consumer_component.instance_id().as_str(),
        "runtime.alpha"
    );
    assert_eq!(
        observation.provider_component.instance_id().as_str(),
        "runtime.alpha"
    );
}

#[test]
fn components_resolve_contracts_without_concrete_coupling() {
    let capture = Arc::new(Mutex::new(None));
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("composition"))
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.core").expect("block"))
                    .register_module(ComponentRuntimeModule::new())
                    .register_module(NamespaceModule::new())
                    .register_module(PublicationModule::new())
                    .register_module(GatewayModule::new())
                    .build(),
            )
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.producer").expect("block"))
                    .register_module(ExampleProducerComponent::new(
                        "runtime.producer.component",
                        "component.producer",
                    ))
                    .build(),
            )
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.consumer").expect("block"))
                    .register_module(ExampleConsumerComponent::new(
                        "runtime.consumer.component",
                        "component.consumer",
                        Arc::clone(&capture),
                    ))
                    .build(),
            )
            .build()
            .expect("composition");
    let mut instance = materialize_instance("runtime.alpha", composition);

    instance.start().expect("start");
    let observation = capture
        .lock()
        .expect("consumer capture lock")
        .clone()
        .expect("consumer observation");

    assert_eq!(
        observation.consumer_component.component_id().as_str(),
        "component.consumer"
    );
    assert_eq!(
        observation.provider_component.component_id().as_str(),
        "component.producer"
    );
}

#[test]
fn missing_required_runtime_contract_fails_composition() {
    let capture = Arc::new(Mutex::new(None));
    let error = CompositionBuilder::new(CompositionId::new("runtime.composition").expect("id"))
        .register_block(
            BlockBuilder::new(fabric_core::BlockId::new("runtime.core").expect("block"))
                .register_module(ComponentRuntimeModule::new())
                .build(),
        )
        .register_block(
            BlockBuilder::new(fabric_core::BlockId::new("runtime.consumer").expect("block"))
                .register_module(ExampleConsumerComponent::new(
                    "runtime.consumer.component",
                    "component.consumer",
                    capture,
                ))
                .build(),
        )
        .build()
        .expect_err("missing example runtime provider");

    match error {
        CompositionError::MissingProvider {
            module_id,
            contract_id,
        } => {
            assert_eq!(module_id.as_str(), "runtime.consumer.component");
            assert_eq!(contract_id, example_component_runtime_contract_id());
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn operation_provider_requires_runtime_membership() {
    let error = CompositionBuilder::new(CompositionId::new("runtime.composition").expect("id"))
        .register_block(
            BlockBuilder::new(fabric_core::BlockId::new("runtime.operations").expect("block"))
                .register_module(ExampleOperationProviderComponent::new(
                    "runtime.operation.provider",
                    "component.operations",
                ))
                .build(),
        )
        .build()
        .expect_err("missing runtime membership");

    match error {
        CompositionError::MissingProvider {
            module_id,
            contract_id,
        } => {
            assert_eq!(module_id.as_str(), "runtime.operation.provider");
            assert_eq!(
                contract_id,
                fabric_component::component_runtime_contract_id()
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn operation_identity_is_stable_across_reconstruction() {
    assert_eq!(
        example_echo_operation_key().id(),
        example_echo_operation_key().id()
    );
}

#[test]
fn operation_rail_invokes_typed_operation_without_concrete_component_coupling() {
    let capture = Arc::new(Mutex::new(None));
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("composition"))
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.core").expect("block"))
                    .register_module(ComponentRuntimeModule::new())
                    .register_module(NamespaceModule::new())
                    .register_module(PublicationModule::new())
                    .register_module(GatewayModule::new())
                    .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.operations.provider").expect("block"),
                )
                .register_module(ExampleOperationProviderComponent::new(
                    "runtime.operation.provider",
                    "component.operations",
                ))
                .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.operations.capture").expect("block"),
                )
                .register_module(OperationRailCapture::new(Arc::clone(&capture)))
                .build(),
            )
            .build()
            .expect("composition");
    let mut instance = materialize_instance("runtime.alpha", composition);

    instance.start().expect("start");
    let (instance_id, _generation, rail, invocation) = capture
        .lock()
        .expect("operation rail capture lock")
        .clone()
        .expect("captured operation rail");
    let output = block_on(rail.invoke_with_context(
        gateway_context(invocation.as_ref()),
        &example_echo_operation_key(),
        EchoInput {
            value: "hello".to_owned(),
        },
    ))
    .expect("typed invoke");

    assert_eq!(instance_id.as_str(), "runtime.alpha");
    assert_eq!(output.value, "hello");
    assert_eq!(output.owner_component_id.as_str(), "component.operations");
}

#[test]
fn operation_owner_is_explicit_component() {
    let capture = Arc::new(Mutex::new(None));
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("composition"))
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.core").expect("block"))
                    .register_module(ComponentRuntimeModule::new())
                    .register_module(NamespaceModule::new())
                    .register_module(PublicationModule::new())
                    .register_module(GatewayModule::new())
                    .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.operations.provider").expect("block"),
                )
                .register_module(ExampleOperationProviderComponent::new(
                    "runtime.operation.provider",
                    "component.operations",
                ))
                .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.operations.capture").expect("block"),
                )
                .register_module(OperationRailCapture::new(Arc::clone(&capture)))
                .build(),
            )
            .build()
            .expect("composition");
    let mut instance = materialize_instance("runtime.alpha", composition);

    instance.start().expect("start");
    let (_, _, rail, _) = capture
        .lock()
        .expect("operation rail capture lock")
        .clone()
        .expect("captured operation rail");
    let owner = rail
        .owner(&example_echo_operation_key())
        .expect("operation owner");

    assert_eq!(owner.component_id().as_str(), "component.operations");
    assert_eq!(owner.instance_id().as_str(), "runtime.alpha");
}

#[test]
fn nested_operation_invocation_completes_without_reentrant_deadlock() {
    let capture = Arc::new(Mutex::new(None));
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("composition"))
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.core").expect("block"))
                    .register_module(ComponentRuntimeModule::new())
                    .register_module(NamespaceModule::new())
                    .register_module(PublicationModule::new())
                    .register_module(GatewayModule::new())
                    .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.operations.provider").expect("block"),
                )
                .register_module(NestedOperationProviderComponent::new(
                    "runtime.operation.provider.nested",
                    "component.operations.nested",
                ))
                .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.operations.capture").expect("block"),
                )
                .register_module(OperationRailCapture::new(Arc::clone(&capture)))
                .build(),
            )
            .build()
            .expect("composition");
    let mut instance = materialize_instance("runtime.alpha", composition);

    instance.start().expect("start");
    let (_instance_id, _generation, rail, invocation) = capture
        .lock()
        .expect("operation rail capture lock")
        .clone()
        .expect("captured operation rail");
    let output = block_on(rail.invoke_with_context(
        gateway_context(invocation.as_ref()),
        &nested_outer_operation_key(),
        EchoInput {
            value: "hello".to_owned(),
        },
    ))
    .expect("nested invoke");

    assert_eq!(
        output.value,
        "outer:component.operations.nested:inner:hello"
    );
    assert_eq!(
        output.owner_component_id.as_str(),
        "component.operations.nested"
    );
}

#[test]
fn duplicate_operation_ids_fail_deterministically() {
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("id"))
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.core").expect("block"))
                    .register_module(ComponentRuntimeModule::new())
                    .register_module(NamespaceModule::new())
                    .register_module(PublicationModule::new())
                    .register_module(GatewayModule::new())
                    .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.operations.a").expect("block"),
                )
                .register_module(ExampleOperationProviderComponent::new(
                    "runtime.operation.provider.a",
                    "component.operations.a",
                ))
                .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.operations.b").expect("block"),
                )
                .register_module(ExampleOperationProviderComponent::new(
                    "runtime.operation.provider.b",
                    "component.operations.b",
                ))
                .build(),
            )
            .build()
            .expect("composition");
    let mut instance = materialize_instance("runtime.alpha", composition);
    let error = instance.start().expect_err("duplicate operation id");

    match error {
        fabric_core::InstanceError::ModuleFailure {
            module_id,
            phase,
            source,
        } => {
            assert_eq!(module_id.as_str(), "runtime.operation.provider.b");
            assert_eq!(phase, "start");
            assert_eq!(
                source.to_string(),
                "OperationId `example.echo` is already registered"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn cross_instance_operation_owner_registration_is_rejected() {
    let capture = Arc::new(Mutex::new(None));
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("composition"))
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.core").expect("block"))
                    .register_module(ComponentRuntimeModule::new())
                    .register_module(NamespaceModule::new())
                    .register_module(PublicationModule::new())
                    .register_module(GatewayModule::new())
                    .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.operations.capture").expect("block"),
                )
                .register_module(OperationRegistrarCapture::new(Arc::clone(&capture)))
                .build(),
            )
            .build()
            .expect("composition");
    let mut instance = materialize_instance("runtime.alpha", composition);

    instance.start().expect("start");
    let (instance_id, registrar) = capture
        .lock()
        .expect("operation registrar capture lock")
        .clone()
        .expect("captured operation registrar");
    let foreign_capture = Arc::new(Mutex::new(None));
    let foreign_composition = CompositionBuilder::new(
        CompositionId::new("runtime.other.composition").expect("composition"),
    )
    .register_block(
        BlockBuilder::new(fabric_core::BlockId::new("runtime.other.core").expect("block"))
            .register_module(ComponentRuntimeModule::new())
            .register_module(ParticipationCaptureModule::new(
                "runtime.other.participation",
                "component.foreign",
                Arc::clone(&foreign_capture),
            ))
            .build(),
    )
    .build()
    .expect("composition");
    let mut foreign_instance = materialize_instance("runtime.other", foreign_composition);
    foreign_instance.start().expect("start foreign");
    let foreign_participation = foreign_capture
        .lock()
        .expect("foreign participation capture lock")
        .clone()
        .expect("foreign participation");
    let error = registrar
        .register(
            foreign_participation,
            example_echo_operation_key(),
            move |input: EchoInput| async move {
                Ok(EchoOutput {
                    value: input.value,
                    owner_component_id: ComponentId::new("component.foreign")
                        .expect("component id"),
                })
            },
        )
        .expect_err("cross-instance owner rejected");

    assert_eq!(instance_id.as_str(), "runtime.alpha");
    assert_eq!(
        error.to_string(),
        "OperationId `example.echo` belongs to Instance `runtime.other` but this Component runtime owns `runtime.alpha`"
    );
    foreign_instance.stop();
}

#[test]
fn unknown_operation_fails_deterministically() {
    let capture = Arc::new(Mutex::new(None));
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("composition"))
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.core").expect("block"))
                    .register_module(ComponentRuntimeModule::new())
                    .register_module(NamespaceModule::new())
                    .register_module(PublicationModule::new())
                    .register_module(GatewayModule::new())
                    .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.operations.provider").expect("block"),
                )
                .register_module(ExampleOperationProviderComponent::new(
                    "runtime.operation.provider",
                    "component.operations",
                ))
                .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.operations.capture").expect("block"),
                )
                .register_module(OperationRailCapture::new(Arc::clone(&capture)))
                .build(),
            )
            .build()
            .expect("composition");
    let mut instance = materialize_instance("runtime.alpha", composition);

    instance.start().expect("start");
    let (_instance_id, _generation, rail, invocation) = capture
        .lock()
        .expect("operation rail capture lock")
        .clone()
        .expect("captured operation rail");
    let error = block_on(rail.invoke_with_context(
        gateway_context(invocation.as_ref()),
        &unknown_echo_operation_key(),
        EchoInput {
            value: "hello".to_owned(),
        },
    ))
    .expect_err("unknown operation");

    assert_eq!(
        error.to_string(),
        "OperationId `example.unknown` is not registered"
    );
}

#[test]
fn gateway_invokes_typed_operation_without_concrete_component_coupling() {
    let capture = Arc::new(Mutex::new(None));
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("composition"))
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.core").expect("block"))
                    .register_module(ComponentRuntimeModule::new())
                    .register_module(NamespaceModule::new())
                    .register_module(PublicationModule::new())
                    .register_module(GatewayModule::new())
                    .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.operations.provider").expect("block"),
                )
                .register_module(ExampleOperationProviderComponent::new(
                    "runtime.operation.provider",
                    "component.operations",
                ))
                .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.gateway.capture").expect("block"),
                )
                .register_module(GatewayCaptureModule::new(Arc::clone(&capture)))
                .build(),
            )
            .build()
            .expect("composition");
    let mut instance = materialize_instance("runtime.alpha", composition);

    instance.start().expect("start");
    let (instance_id, _, gateway) = capture
        .lock()
        .expect("runtime gateway capture lock")
        .clone()
        .expect("captured runtime gateway");
    let response = block_on(gateway.invoke(GatewayRequest::new(
        example_echo_operation_key(),
        EchoInput {
            value: "hello".to_owned(),
        },
    )))
    .expect("gateway invoke");
    let output: GatewayResponse<EchoOutput> = response;

    assert_eq!(instance_id.as_str(), "runtime.alpha");
    assert_eq!(output.output().value, "hello");
    assert_eq!(
        output.output().owner_component_id.as_str(),
        "component.operations"
    );
}

#[test]
fn unknown_gateway_operation_fails_semantically() {
    let capture = Arc::new(Mutex::new(None));
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("composition"))
            .register_block(
                BlockBuilder::new(fabric_core::BlockId::new("runtime.core").expect("block"))
                    .register_module(ComponentRuntimeModule::new())
                    .register_module(NamespaceModule::new())
                    .register_module(PublicationModule::new())
                    .register_module(GatewayModule::new())
                    .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.operations.provider").expect("block"),
                )
                .register_module(ExampleOperationProviderComponent::new(
                    "runtime.operation.provider",
                    "component.operations",
                ))
                .build(),
            )
            .register_block(
                BlockBuilder::new(
                    fabric_core::BlockId::new("runtime.gateway.capture").expect("block"),
                )
                .register_module(GatewayCaptureModule::new(Arc::clone(&capture)))
                .build(),
            )
            .build()
            .expect("composition");
    let mut instance = materialize_instance("runtime.alpha", composition);

    instance.start().expect("start");
    let (_, _, gateway) = capture
        .lock()
        .expect("runtime gateway capture lock")
        .clone()
        .expect("captured runtime gateway");
    let error = block_on(gateway.invoke(GatewayRequest::new(
        unknown_echo_operation_key(),
        EchoInput {
            value: "hello".to_owned(),
        },
    )))
    .expect_err("unknown gateway operation");

    assert_eq!(
        error.to_string(),
        "Gateway cannot resolve OperationId `example.unknown`"
    );
}

fn started_instance_id(instance_id: &str) -> InstanceId {
    let capture = Arc::new(Mutex::new(None));
    let block = BlockBuilder::new(fabric_core::BlockId::new("runtime.block").expect("block"))
        .register_module(ComponentRuntimeModule::new())
        .register_module(ComponentRuntimeCapture::new(Arc::clone(&capture)))
        .build();
    let composition =
        CompositionBuilder::new(CompositionId::new("runtime.composition").expect("composition"))
            .register_block(block)
            .build()
            .expect("composition");
    let mut instance = materialize_instance(instance_id, composition);
    instance.start().expect("start");
    let instance_id = capture
        .lock()
        .expect("runtime contract capture lock")
        .as_ref()
        .expect("captured runtime contract")
        .as_ref()
        .current_instance_id()
        .expect("instance id");
    instance.stop();
    instance_id
}

#[derive(Clone)]
struct ComponentRuntimeCapture {
    module_id: ModuleId,
    requirement: ContractRequirement<ComponentRuntime>,
    contract: Option<Arc<ComponentRuntime>>,
    capture: Arc<Mutex<Option<Arc<ComponentRuntime>>>>,
}

impl ComponentRuntimeCapture {
    fn new(capture: Arc<Mutex<Option<Arc<ComponentRuntime>>>>) -> Self {
        Self {
            module_id: ModuleId::new("runtime.capture").expect("module id"),
            requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            contract: None,
            capture,
        }
    }
}

impl ModuleRuntime for ComponentRuntimeCapture {
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
        vec![self.requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let contract = bindings
            .resolve(&self.requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.lock().expect("runtime contract capture lock") = Some(Arc::clone(&contract));
        self.contract = Some(contract);
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
        if self.contract.is_some() {
            Health::Healthy
        } else {
            Health::Degraded
        }
    }
}
