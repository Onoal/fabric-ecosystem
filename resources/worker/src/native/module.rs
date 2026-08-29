use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fabric_core::{
    ContractRequirement, Health, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};
use fabric_projection::ProjectionError;
use fabric_resource::ResourceId;
use fabric_resource_registry::{
    ResourceDescriptor, ResourceRegistry, resource_registry_contract_id,
};

use crate::compatibility::evaluate_compatibility;
use crate::contract::{
    WorkerContract, WorkerHttpContract, WorkerHttpService, WorkerService, worker_contract_key,
    worker_http_contract_key,
};
use crate::native::adapter::{WorkerAdapter, WorkerExecution, WorkerExecutionRequest};
use crate::projection::{
    PreparedWorkloadProjections, WorkloadProjectionContract, workload_projection_contract_id,
};
use crate::{
    DispatchHttpRequest, HttpResponse, PreparedWorker, StartWorkerRequest, StopWorkerRequest,
    WorkerCapabilities, WorkerError, WorkerInstance, WorkerInstanceId, WorkerInstanceStatus,
    WorkerSpec, WorkloadBinding, WorkloadBindingProjection, validate_workload_bindings,
};

pub fn worker_resource_id() -> ResourceId {
    ResourceId::new("worker").expect("static worker resource id")
}

pub struct NativeWorker {
    module_id: ModuleId,
    resource_registry_requirement: ContractRequirement<ResourceRegistry>,
    projection_requirement: ContractRequirement<WorkloadProjectionContract>,
    resource_registry: Option<ResourceRegistry>,
    shared: Arc<SharedWorkerState>,
    supports_http_dispatch: bool,
}

pub(crate) struct SharedWorkerState {
    inner: Mutex<WorkerState>,
}

struct WorkerState {
    health: Health,
    started: bool,
    projection: Option<WorkloadProjectionContract>,
    adapter: Box<dyn WorkerAdapter>,
    live: BTreeMap<WorkerInstanceId, LiveWorkerInstance>,
}

struct LiveWorkerInstance {
    instance: WorkerInstance,
    execution: Box<dyn WorkerExecution>,
}

impl NativeWorker {
    pub fn with_adapter(adapter: Box<dyn WorkerAdapter>) -> Self {
        let supports_http_dispatch = adapter.supports_http_dispatch();
        Self {
            module_id: ModuleId::new("fabric.resource.worker.native")
                .expect("static worker module id"),
            resource_registry_requirement: ContractRequirement::provisional(
                resource_registry_contract_id(),
            ),
            projection_requirement: ContractRequirement::provisional(
                workload_projection_contract_id(),
            ),
            resource_registry: None,
            shared: Arc::new(SharedWorkerState {
                inner: Mutex::new(WorkerState {
                    health: Health::Unavailable,
                    started: false,
                    projection: None,
                    adapter,
                    live: BTreeMap::new(),
                }),
            }),
            supports_http_dispatch,
        }
    }
}

impl ModuleRuntime for NativeWorker {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        let mut contracts = vec![worker_contract_key().declaration()];
        if self.supports_http_dispatch {
            contracts.push(worker_http_contract_key().declaration());
        }
        contracts
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![
            self.resource_registry_requirement.declaration().clone(),
            self.projection_requirement.declaration().clone(),
        ]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        let worker_service: Arc<dyn WorkerService> =
            Arc::clone(&self.shared) as Arc<dyn WorkerService>;
        let mut contracts = vec![ModuleContract::new(
            &worker_contract_key(),
            Arc::new(WorkerContract::new(worker_service)),
        )];
        if self.supports_http_dispatch {
            let http_service: Arc<dyn WorkerHttpService> =
                Arc::clone(&self.shared) as Arc<dyn WorkerHttpService>;
            contracts.push(ModuleContract::new(
                &worker_http_contract_key(),
                Arc::new(WorkerHttpContract::new(http_service)),
            ));
        }
        Ok(contracts)
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        self.resource_registry = bindings
            .resolve_optional(&self.resource_registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?
            .as_deref()
            .cloned();
        let projection = bindings
            .resolve_optional(&self.projection_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        self.shared
            .inner
            .lock()
            .expect("worker state lock")
            .projection = projection.as_deref().cloned();
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        let mut state = self.shared.inner.lock().expect("worker state lock");
        for (_, mut live) in std::mem::take(&mut state.live) {
            live.execution.cleanup();
        }
        state.adapter.clear();
        state.started = false;
        state.health = Health::Unavailable;
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        if let Some(shell) = &self.resource_registry {
            shell
                .register(self, ResourceDescriptor::new(worker_resource_id()))
                .map_err(|error| ModuleError::new(error.to_string()))?;
        }
        let mut state = self.shared.inner.lock().expect("worker state lock");
        state.started = true;
        state.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(shell) = &self.resource_registry {
            let _ = shell.unregister(&self.module_id);
        }
        let mut state = self.shared.inner.lock().expect("worker state lock");
        for (_, mut live) in std::mem::take(&mut state.live) {
            live.execution.cleanup();
        }
        state.adapter.clear();
        state.started = false;
        state.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.shared.inner.lock().expect("worker state lock").health
    }
}

impl WorkerService for SharedWorkerState {
    fn inspect_capabilities(&self) -> Result<WorkerCapabilities, WorkerError> {
        self.with_state(|state| Ok(state.adapter.capabilities()))
    }

    fn prepare_worker(&self, worker: WorkerSpec) -> Result<PreparedWorker, WorkerError> {
        self.with_state(|state| {
            let compatibility =
                evaluate_compatibility(&worker.requirement, &state.adapter.capabilities())?;
            if !compatibility.is_compatible() {
                return Err(WorkerError::Incompatible {
                    issues: compatibility.issues().to_vec(),
                });
            }
            state.adapter.prepare(&worker)?;
            Ok(PreparedWorker::new(worker))
        })
    }

    fn start_worker(&self, request: StartWorkerRequest) -> Result<WorkerInstance, WorkerError> {
        self.with_state(|state| {
            if request.prepared_worker.workload_id != request.worker.workload_id {
                return Err(WorkerError::ProtocolViolation {
                    message: "prepared worker does not match start worker workload".to_owned(),
                });
            }
            if !request.prepared_worker.matches(&request.worker) {
                return Err(WorkerError::ProtocolViolation {
                    message: "prepared worker does not match start worker".to_owned(),
                });
            }
            validate_workload_bindings(
                &request.worker.workload_id,
                &request.bindings,
                &request.binding_projections,
            )?;
            let projections = prepare_projections(
                state.projection.as_ref(),
                &request.bindings,
                &request.binding_projections,
            )?;
            let worker_instance_id = WorkerInstanceId::new(hex::encode(rand::random::<[u8; 16]>()))
                .map_err(|error| WorkerError::StartFailed {
                    message: error.to_string(),
                })?;
            let instance = WorkerInstance {
                worker_instance_id: worker_instance_id.clone(),
                workload_id: request.worker.workload_id.clone(),
                status: WorkerInstanceStatus::Running,
            };
            let execution = state.adapter.start(WorkerExecutionRequest {
                instance_id: worker_instance_id.clone(),
                start: request,
                projections,
            })?;
            state.live.insert(
                worker_instance_id,
                LiveWorkerInstance {
                    instance: instance.clone(),
                    execution,
                },
            );
            Ok(instance)
        })
    }

    fn stop_worker(&self, request: StopWorkerRequest) -> Result<(), WorkerError> {
        self.with_state(|state| {
            if let Some(mut live) = state.live.remove(&request.worker_instance_id) {
                let stop_result = live.execution.stop();
                live.execution.cleanup();
                stop_result?;
            }
            Ok(())
        })
    }
}

impl WorkerHttpService for SharedWorkerState {
    fn dispatch_http(&self, request: DispatchHttpRequest) -> Result<HttpResponse, WorkerError> {
        self.with_state(|state| {
            let live = state
                .live
                .get_mut(&request.worker_instance_id)
                .ok_or_else(|| WorkerError::ProtocolViolation {
                    message: format!(
                        "worker instance {} is not live",
                        request.worker_instance_id.as_str()
                    ),
                })?;
            let _workload_id = live.instance.workload_id.clone();
            live.execution.dispatch_http(request)
        })
    }
}

impl SharedWorkerState {
    fn with_state<T>(
        &self,
        action: impl FnOnce(&mut WorkerState) -> Result<T, WorkerError>,
    ) -> Result<T, WorkerError> {
        let mut state = self.inner.lock().expect("worker state lock");
        if !state.started {
            return Err(WorkerError::Unavailable);
        }
        action(&mut state)
    }
}

fn prepare_projections(
    projection: Option<&WorkloadProjectionContract>,
    bindings: &[WorkloadBinding],
    binding_projections: &[WorkloadBindingProjection],
) -> Result<PreparedWorkloadProjections, WorkerError> {
    if binding_projections.is_empty() {
        return Ok(PreparedWorkloadProjections::default());
    }
    let projection = projection.ok_or_else(|| WorkerError::StartFailed {
        message: "workload projection contract is unavailable".to_owned(),
    })?;
    projection
        .prepare(bindings, binding_projections)
        .map_err(map_projection_error)
}

fn map_projection_error(error: ProjectionError) -> WorkerError {
    match error {
        ProjectionError::Unavailable => WorkerError::Unavailable,
        ProjectionError::InvalidInput { message } => WorkerError::InvalidInput { message },
        ProjectionError::MaterializationFailed { message } => WorkerError::StartFailed { message },
    }
}
