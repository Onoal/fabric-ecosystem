use std::process::{Command, Stdio};
use std::sync::Arc;

use fabric::core::{
    BlockBuilder, CompositionExport, ContractId, ContractKey, ContractRequirement, Health,
    HostMaterializationRequirement, Module, ModuleBindings, ModuleContract, ModuleDeclaration,
    ModuleError, ModuleId, ModuleRuntime,
};
use fabric::host::{HostArchitecture, HostOperatingSystem, HostRequirement};

use crate::definition::LocalProcessDefinition;
use crate::occurrence::{LocalProcessHandle, refresh};
use crate::{LocalProcessRuntimeError, LocalProcessStatus};

/// Fabric module declaration for one locally managed OS process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalProcessModule {
    definition: LocalProcessDefinition,
}

impl LocalProcessModule {
    pub fn new(definition: LocalProcessDefinition) -> Self {
        Self { definition }
    }

    pub fn definition(&self) -> &LocalProcessDefinition {
        &self.definition
    }

    pub fn register_in(self, block: BlockBuilder) -> BlockBuilder {
        block.register_module(self)
    }
}

pub fn local_process_contract_id(module_id: &ModuleId) -> ContractId {
    ContractId::new(format!("onoal.fabric.process.local.{}", module_id.as_str()))
        .expect("module ids are valid inside process contract ids")
}

pub fn local_process_contract_key(module_id: &ModuleId) -> ContractKey<LocalProcessHandle> {
    ContractKey::provisional(local_process_contract_id(module_id))
}

pub fn local_process_export(
    export_id: ContractId,
    module_id: &ModuleId,
) -> CompositionExport<LocalProcessHandle> {
    CompositionExport::new(
        export_id,
        ContractRequirement::provisional(local_process_contract_id(module_id)),
    )
}

impl Module for LocalProcessModule {
    fn declaration(&self) -> ModuleDeclaration {
        let module_id = self.definition.module_id().clone();
        let host_requirement = HostRequirement::new()
            .allow_operating_system(
                HostOperatingSystem::new(std::env::consts::OS).expect("native OS is valid"),
            )
            .allow_architecture(
                HostArchitecture::new(std::env::consts::ARCH).expect("native arch is valid"),
            );
        ModuleDeclaration::new(module_id.clone())
            .with_host_requirement(HostMaterializationRequirement::new(
                module_id.clone(),
                host_requirement,
            ))
            .with_provided_contracts(vec![local_process_contract_key(&module_id).declaration()])
    }

    fn materialize(&self) -> Option<Box<dyn ModuleRuntime>> {
        Some(Box::new(LocalProcessRuntime {
            definition: self.definition.clone(),
            handle: LocalProcessHandle::new(),
        }))
    }
}

struct LocalProcessRuntime {
    definition: LocalProcessDefinition,
    handle: LocalProcessHandle,
}

impl ModuleRuntime for LocalProcessRuntime {
    fn id(&self) -> &ModuleId {
        self.definition.module_id()
    }

    fn provided_contract_declarations(&self) -> Vec<fabric::core::ProvidedContractDeclaration> {
        vec![local_process_contract_key(self.id()).declaration()]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(vec![ModuleContract::new(
            &local_process_contract_key(self.id()),
            Arc::new(self.handle.clone()),
        )])
    }

    fn bind(&mut self, _bindings: &ModuleBindings) -> Result<(), ModuleError> {
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        let mut state = self
            .handle
            .state
            .lock()
            .map_err(|_| ModuleError::new("local process state lock is poisoned"))?;
        refresh(&mut state).map_err(module_error)?;
        if matches!(state.status, LocalProcessStatus::Running { .. }) || state.child.is_some() {
            return Err(module_error(LocalProcessRuntimeError::AlreadyStarted));
        }
        let mut command = Command::new(self.definition.program());
        command
            .args(self.definition.args())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match command.spawn() {
            Ok(child) => {
                let pid = child.id();
                state.child = Some(child);
                state.status = LocalProcessStatus::Running { pid };
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                state.status = LocalProcessStatus::SpawnFailed {
                    message: message.clone(),
                };
                Err(module_error(LocalProcessRuntimeError::spawn_failed(
                    message,
                )))
            }
        }
    }

    fn stop(&mut self) {
        let Ok(mut state) = self.handle.state.lock() else {
            return;
        };
        if refresh(&mut state).is_err() {
            return;
        }
        let Some(mut child) = state.child.take() else {
            return;
        };
        let pid = child.id();
        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
        }
        let _ = child.wait();
        state.status = LocalProcessStatus::Stopped { pid: Some(pid) };
    }

    fn health(&self) -> Health {
        let Ok(mut state) = self.handle.state.lock() else {
            return Health::Unavailable;
        };
        if refresh(&mut state).is_err() {
            return Health::Unavailable;
        }
        match &state.status {
            LocalProcessStatus::Running { .. } => Health::Healthy,
            LocalProcessStatus::Exited { exit, .. } if exit.successful() => Health::Degraded,
            LocalProcessStatus::NotStarted | LocalProcessStatus::Stopped { .. } => Health::Degraded,
            LocalProcessStatus::Exited { .. } | LocalProcessStatus::SpawnFailed { .. } => {
                Health::Unavailable
            }
        }
    }
}

fn module_error(error: LocalProcessRuntimeError) -> ModuleError {
    ModuleError::new(error.to_string())
}
