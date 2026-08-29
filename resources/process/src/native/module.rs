use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fabric_core::{
    ContractRequirement, Health, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};
use fabric_resource::ResourceId;
use fabric_resource_registry::{
    ResourceDescriptor, ResourceRegistry, resource_registry_contract_id,
};

use crate::contract::{ProcessContract, ProcessService, process_contract_key};
use crate::native::adapter::{ProcessAdapter, ProcessExecution, ProcessExecutionRequest};
use crate::{
    PreparedProcess, ProcessError, ProcessInstance, ProcessInstanceId, ProcessInstanceStatus,
    ProcessSpec, StartProcessRequest, StopProcessRequest,
};

pub fn process_resource_id() -> ResourceId {
    ResourceId::new("process").expect("static process resource id")
}

pub struct NativeProcess {
    module_id: ModuleId,
    resource_registry_requirement: ContractRequirement<ResourceRegistry>,
    resource_registry: Option<ResourceRegistry>,
    shared: Arc<SharedProcessState>,
}

pub(crate) struct SharedProcessState {
    inner: Mutex<ProcessState>,
}

struct ProcessState {
    health: Health,
    started: bool,
    adapter: Box<dyn ProcessAdapter>,
    live: BTreeMap<ProcessInstanceId, LiveProcessInstance>,
}

struct LiveProcessInstance {
    execution: Box<dyn ProcessExecution>,
}

impl NativeProcess {
    pub fn with_adapter(adapter: Box<dyn ProcessAdapter>) -> Self {
        Self {
            module_id: ModuleId::new("fabric.resource.process.native")
                .expect("static process module id"),
            resource_registry_requirement: ContractRequirement::provisional(
                resource_registry_contract_id(),
            ),
            resource_registry: None,
            shared: Arc::new(SharedProcessState {
                inner: Mutex::new(ProcessState {
                    health: Health::Unavailable,
                    started: false,
                    adapter,
                    live: BTreeMap::new(),
                }),
            }),
        }
    }
}

impl ModuleRuntime for NativeProcess {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![process_contract_key().declaration()]
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.resource_registry_requirement.declaration().clone()]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        let process_service: Arc<dyn ProcessService> =
            Arc::clone(&self.shared) as Arc<dyn ProcessService>;
        Ok(vec![ModuleContract::new(
            &process_contract_key(),
            Arc::new(ProcessContract::new(process_service)),
        )])
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        self.resource_registry = bindings
            .resolve_optional(&self.resource_registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?
            .as_deref()
            .cloned();
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        let mut state = self.shared.inner.lock().expect("process state lock");
        for (_, mut live) in std::mem::take(&mut state.live) {
            live.execution.cleanup();
        }
        state.adapter.clear();
        state.started = false;
        state.health = Health::Unavailable;
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        if let Some(registry) = &self.resource_registry {
            registry
                .register(self, ResourceDescriptor::new(process_resource_id()))
                .map_err(|error| ModuleError::new(error.to_string()))?;
        }
        let mut state = self.shared.inner.lock().expect("process state lock");
        state.started = true;
        state.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(registry) = &self.resource_registry {
            let _ = registry.unregister(&self.module_id);
        }
        let mut state = self.shared.inner.lock().expect("process state lock");
        for (_, mut live) in std::mem::take(&mut state.live) {
            live.execution.cleanup();
        }
        state.adapter.clear();
        state.started = false;
        state.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.shared.inner.lock().expect("process state lock").health
    }
}

impl ProcessService for SharedProcessState {
    fn prepare_process(&self, process: ProcessSpec) -> Result<PreparedProcess, ProcessError> {
        self.with_state(|state| {
            state.adapter.prepare(&process)?;
            Ok(PreparedProcess::new(process))
        })
    }

    fn start_process(&self, request: StartProcessRequest) -> Result<ProcessInstance, ProcessError> {
        self.with_state(|state| {
            if request.prepared_process.process_id != request.process.process_id {
                return Err(ProcessError::ProtocolViolation {
                    message: "prepared process does not match start process id".to_owned(),
                });
            }
            if !request.prepared_process.matches(&request.process) {
                return Err(ProcessError::ProtocolViolation {
                    message: "prepared process does not match start process".to_owned(),
                });
            }
            let process_instance_id = ProcessInstanceId::new(hex::encode(
                rand::random::<[u8; 16]>(),
            ))
            .map_err(|error| ProcessError::StartFailed {
                message: error.to_string(),
            })?;
            let instance = ProcessInstance {
                process_instance_id: process_instance_id.clone(),
                process_id: request.process.process_id.clone(),
                status: ProcessInstanceStatus::Running,
            };
            let execution = state.adapter.start(ProcessExecutionRequest {
                instance_id: process_instance_id.clone(),
                start: request,
            })?;
            state
                .live
                .insert(process_instance_id, LiveProcessInstance { execution });
            Ok(instance)
        })
    }

    fn stop_process(&self, request: StopProcessRequest) -> Result<(), ProcessError> {
        self.with_state(|state| {
            if let Some(mut live) = state.live.remove(&request.process_instance_id) {
                let stop_result = live.execution.stop();
                live.execution.cleanup();
                stop_result?;
            }
            Ok(())
        })
    }
}

impl SharedProcessState {
    fn with_state<T>(
        &self,
        action: impl FnOnce(&mut ProcessState) -> Result<T, ProcessError>,
    ) -> Result<T, ProcessError> {
        let mut state = self.inner.lock().expect("process state lock");
        if !state.started {
            return Err(ProcessError::Unavailable);
        }
        action(&mut state)
    }
}
