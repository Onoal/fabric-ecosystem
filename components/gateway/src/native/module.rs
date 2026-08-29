use std::any::TypeId;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fabric_component::{
    Component, ComponentError, ComponentId, ComponentParticipation, ComponentRegistry,
    ComponentRequirementRail, ComponentRuntime, InvocationContext, InvocationRail, OperationId,
    OperationRail, ParticipationState, component_registry_contract_id,
    component_requirement_contract_id, component_runtime_contract_id, invocation_contract_id,
    operation_rail_contract_id,
};
use fabric_component_namespace::NamespaceName;
use fabric_component_publication::{PublicationContract, publication_contract_id};
use fabric_core::{
    ContractRequirement, Health, Module, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};

use crate::{
    Gateway, GatewayEntry, GatewayError, GatewayFuture, GatewayService, GatewayStepApplicability,
    GatewayStepCall, GatewayStepExecution, GatewayStepId, GatewayStepPhase, GatewayStepRegistrar,
    GatewayStepService, gateway_contract_key, gateway_step_registrar_contract_key,
};

pub struct GatewayModule {
    module_id: ModuleId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registry_requirement: ContractRequirement<ComponentRegistry>,
    requirements_requirement: ContractRequirement<ComponentRequirementRail>,
    invocation_requirement: ContractRequirement<InvocationRail>,
    operations_requirement: ContractRequirement<OperationRail>,
    publications_requirement: ContractRequirement<PublicationContract>,
    shared: Arc<SharedGatewayState>,
}

struct SharedGatewayState {
    inner: Mutex<GatewayModuleState>,
}

struct GatewayModuleState {
    health: Health,
    runtime: Option<ComponentRuntime>,
    registry: Option<ComponentRegistry>,
    requirements: Option<ComponentRequirementRail>,
    invocation: Option<InvocationRail>,
    operations: Option<OperationRail>,
    publications: Option<PublicationContract>,
    component: Option<Component>,
    participation: Option<ComponentParticipation>,
    started: bool,
    steps: BTreeMap<GatewayStepId, RegisteredGatewayStep>,
}

struct RegisteredGatewayStep {
    owner: ComponentParticipation,
    phase: GatewayStepPhase,
    applies: GatewayStepApplicability,
    execute: GatewayStepExecution,
}

impl GatewayModule {
    pub fn new() -> Self {
        Self {
            module_id: ModuleId::new("fabric.component.gateway.module")
                .expect("static gateway module id"),
            runtime_requirement: ContractRequirement::provisional(component_runtime_contract_id()),
            registry_requirement: ContractRequirement::provisional(component_registry_contract_id()),
            requirements_requirement: ContractRequirement::provisional(
                component_requirement_contract_id(),
            ),
            invocation_requirement: ContractRequirement::provisional(invocation_contract_id()),
            operations_requirement: ContractRequirement::provisional(operation_rail_contract_id()),
            publications_requirement: ContractRequirement::provisional(publication_contract_id()),
            shared: Arc::new(SharedGatewayState {
                inner: Mutex::new(GatewayModuleState {
                    health: Health::Unavailable,
                    runtime: None,
                    registry: None,
                    requirements: None,
                    invocation: None,
                    operations: None,
                    publications: None,
                    component: None,
                    participation: None,
                    started: false,
                    steps: BTreeMap::new(),
                }),
            }),
        }
    }
}

impl Default for GatewayModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleRuntime for GatewayModule {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![
            gateway_contract_key().declaration(),
            gateway_step_registrar_contract_key().declaration(),
        ]
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![
            self.runtime_requirement.declaration().clone(),
            self.registry_requirement.declaration().clone(),
            self.requirements_requirement.declaration().clone(),
            self.invocation_requirement.declaration().clone(),
            self.operations_requirement.declaration().clone(),
            self.publications_requirement.declaration().clone(),
        ]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        let gateway: Arc<dyn GatewayService> = self.shared.clone();
        let steps: Arc<dyn GatewayStepService> = self.shared.clone();
        Ok(vec![
            ModuleContract::new(&gateway_contract_key(), Arc::new(Gateway::new(gateway))),
            ModuleContract::new(
                &gateway_step_registrar_contract_key(),
                Arc::new(GatewayStepRegistrar::new(steps)),
            ),
        ])
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let runtime = bindings
            .resolve(&self.runtime_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let registry = bindings
            .resolve(&self.registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let requirements = bindings
            .resolve(&self.requirements_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let invocation = bindings
            .resolve(&self.invocation_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let operations = bindings
            .resolve(&self.operations_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let publications = bindings
            .resolve(&self.publications_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let component_id =
            ComponentId::new("gateway").map_err(|error| ModuleError::new(error.to_string()))?;
        let component = Component::bind(component_id, runtime.as_ref());

        let mut state = self.shared.inner.lock().expect("gateway state lock");
        state.runtime = Some(runtime.as_ref().clone());
        state.registry = Some(registry.as_ref().clone());
        state.requirements = Some(requirements.as_ref().clone());
        state.invocation = Some(invocation.as_ref().clone());
        state.operations = Some(operations.as_ref().clone());
        state.publications = Some(publications.as_ref().clone());
        state.component = Some(component);
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        let mut state = self.shared.inner.lock().expect("gateway state lock");
        state.health = Health::Unavailable;
        state.started = false;
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        let (registry, component) = {
            let state = self.shared.inner.lock().expect("gateway state lock");
            let registry = state
                .registry
                .clone()
                .ok_or_else(|| ModuleError::new("gateway registry dependency is not bound"))?;
            let component = state
                .component
                .clone()
                .ok_or_else(|| ModuleError::new("gateway component is not bound"))?;
            (registry, component)
        };

        let status = registry
            .register(component, Health::Healthy)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let status = registry
            .activate(status.participation())
            .map_err(|error| ModuleError::new(error.to_string()))?;

        let mut state = self.shared.inner.lock().expect("gateway state lock");
        state.participation = Some(status.participation().clone());
        state.started = true;
        state.health = status.health();
        Ok(())
    }

    fn stop(&mut self) {
        let (registry, participation) = {
            let mut state = self.shared.inner.lock().expect("gateway state lock");
            state.started = false;
            state.health = Health::Unavailable;
            state.steps.clear();
            (state.registry.clone(), state.participation.take())
        };
        if let (Some(registry), Some(participation)) = (registry, participation) {
            let _ = registry.unregister(&participation);
        }
    }

    fn health(&self) -> Health {
        self.shared.inner.lock().expect("gateway state lock").health
    }
}

impl Module for GatewayModule {
    fn materialize(&self) -> Box<dyn ModuleRuntime> {
        Box::new(Self::new())
    }
}

impl GatewayStepService for SharedGatewayState {
    fn register(
        &self,
        owner: ComponentParticipation,
        step_id: GatewayStepId,
        phase: GatewayStepPhase,
        applies: GatewayStepApplicability,
        execute: GatewayStepExecution,
    ) -> Result<(), GatewayError> {
        let mut state = self.inner.lock().expect("gateway state lock");
        let runtime = state.runtime.as_ref().ok_or(ComponentError::Unavailable)?;
        if owner.component().instance_id() != &runtime.instance_id() {
            return Err(GatewayError::GatewayStepOwnerNotParticipating(
                step_id,
                owner.component().component_id().clone(),
            ));
        }
        let registry = state.registry.as_ref().ok_or(ComponentError::Unavailable)?;
        let status = registry
            .component(owner.component().component_id())
            .map_err(GatewayError::from)?;
        if status.participation() != &owner {
            return Err(GatewayError::GatewayStepOwnerNotParticipating(
                step_id,
                owner.component().component_id().clone(),
            ));
        }
        if status.state() != ParticipationState::Preparing {
            return Err(GatewayError::GatewayStepOwnerNotPreparing(
                step_id,
                owner.component().component_id().clone(),
            ));
        }
        if let Some(existing) = state.steps.get(&step_id) {
            let existing_is_current =
                match registry.component(existing.owner.component().component_id()) {
                    Ok(existing_status) => existing_status.participation() == &existing.owner,
                    Err(ComponentError::UnknownComponent(_)) => false,
                    Err(error) => return Err(GatewayError::from(error)),
                };
            if existing_is_current {
                return Err(GatewayError::DuplicateGatewayStepId(step_id));
            }
        }
        state.steps.insert(
            step_id,
            RegisteredGatewayStep {
                owner,
                phase,
                applies,
                execute,
            },
        );
        Ok(())
    }
}

impl GatewayService for SharedGatewayState {
    fn entry(&self, name: &NamespaceName) -> Result<GatewayEntry, GatewayError> {
        let publications = self
            .inner
            .lock()
            .expect("gateway state lock")
            .publications
            .clone()
            .ok_or(ComponentError::Unavailable)?;
        let publication = publications.publication(name).map_err(GatewayError::from)?;
        Ok(GatewayEntry::new(publication))
    }

    fn entries(&self) -> Vec<GatewayEntry> {
        self.inner
            .lock()
            .expect("gateway state lock")
            .publications
            .clone()
            .map(|publications| {
                publications
                    .publications()
                    .into_iter()
                    .map(GatewayEntry::new)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn invoke_erased(
        &self,
        context: Option<InvocationContext>,
        operation_id: &OperationId,
        input_type: TypeId,
        output_type: TypeId,
        input: Box<dyn std::any::Any + Send + Sync>,
    ) -> GatewayFuture<Box<dyn std::any::Any + Send + Sync>> {
        let (invocation, operations, requirements, steps, owner) = {
            let state = self.inner.lock().expect("gateway state lock");
            let invocation = state.invocation.clone().ok_or(ComponentError::Unavailable);
            let operations = state.operations.clone().ok_or(ComponentError::Unavailable);
            let requirements = state
                .requirements
                .clone()
                .ok_or(ComponentError::Unavailable);
            let owner = match operations
                .as_ref()
                .ok()
                .and_then(|operations| operations.operation(operation_id).ok())
            {
                Some(descriptor) => descriptor.owner().clone(),
                None => {
                    let operation_id = operation_id.clone();
                    return Box::pin(async move {
                        Err(GatewayError::UnknownGatewayOperation(operation_id))
                    });
                }
            };
            let steps = state
                .steps
                .iter()
                .map(|(step_id, step)| {
                    (
                        step_id.clone(),
                        step.owner.clone(),
                        step.phase,
                        Arc::clone(&step.applies),
                        Arc::clone(&step.execute),
                    )
                })
                .collect::<Vec<_>>();
            (invocation, operations, requirements, steps, owner)
        };

        let invocation = match context {
            Some(context) => Ok(context),
            None => invocation
                .map_err(GatewayError::from)
                .and_then(|invocation| invocation.begin_external().map_err(GatewayError::from)),
        };
        let operations = match operations {
            Ok(operations) => operations,
            Err(error) => return Box::pin(async move { Err(GatewayError::from(error)) }),
        };
        let requirements = match requirements {
            Ok(requirements) => requirements,
            Err(error) => return Box::pin(async move { Err(GatewayError::from(error)) }),
        };
        let context = match invocation {
            Ok(context) => context,
            Err(error) => return Box::pin(async move { Err(error) }),
        };

        let call = GatewayStepCall::new(context.clone(), operation_id.clone(), owner.clone());
        let mut matched_steps = Vec::new();
        for (_step_id, owner, phase, applies, execute) in steps {
            if !applies(&call) {
                continue;
            }
            let registry = self
                .inner
                .lock()
                .expect("gateway state lock")
                .registry
                .clone()
                .ok_or(ComponentError::Unavailable);
            let registry = match registry {
                Ok(registry) => registry,
                Err(error) => return Box::pin(async move { Err(GatewayError::from(error)) }),
            };
            let status = match registry.component(owner.component().component_id()) {
                Ok(status) => status,
                Err(ComponentError::UnknownComponent(_)) => continue,
                Err(error) => return Box::pin(async move { Err(GatewayError::from(error)) }),
            };
            if status.participation() != &owner {
                continue;
            }
            if status.state() != ParticipationState::Active {
                let component_id = owner.component().component_id().clone();
                return Box::pin(async move {
                    Err(GatewayError::from(ComponentError::ComponentUnavailable(
                        component_id,
                    )))
                });
            }
            match requirements.effective_availability(owner.component().component_id()) {
                Ok(availability) if availability.is_available() => {}
                Ok(_) => {
                    let component_id = owner.component().component_id().clone();
                    return Box::pin(async move {
                        Err(GatewayError::from(ComponentError::ComponentUnavailable(
                            component_id,
                        )))
                    });
                }
                Err(error) => return Box::pin(async move { Err(GatewayError::from(error)) }),
            }
            matched_steps.push((phase, execute));
        }
        matched_steps.sort_by_key(|(phase, _)| *phase);
        for (_, execute) in matched_steps {
            if let Err(error) = execute(&call) {
                return Box::pin(async move { Err(error) });
            }
        }

        let future =
            operations.invoke_erased(context, operation_id, input_type, output_type, input);
        let operation_id = operation_id.clone();
        Box::pin(async move {
            future.await.map_err(|error| match error {
                ComponentError::UnknownOperation(_) => {
                    GatewayError::UnknownGatewayOperation(operation_id)
                }
                other => GatewayError::from(other),
            })
        })
    }
}
