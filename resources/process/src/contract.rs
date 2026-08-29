use std::sync::Arc;

use fabric_core::{ContractId, ContractKey};

use crate::{
    PreparedProcess, ProcessError, ProcessInstance, ProcessSpec, StartProcessRequest,
    StopProcessRequest,
};

const PROCESS_CONTRACT_ID: &str = "fabric.resource.process";

pub fn process_contract_id() -> ContractId {
    ContractId::new(PROCESS_CONTRACT_ID).expect("static process contract id")
}

pub fn process_contract_key() -> ContractKey<ProcessContract> {
    ContractKey::provisional(process_contract_id())
}

pub trait ProcessService: Send + Sync {
    fn prepare_process(&self, process: ProcessSpec) -> Result<PreparedProcess, ProcessError>;
    fn start_process(&self, request: StartProcessRequest) -> Result<ProcessInstance, ProcessError>;
    fn stop_process(&self, request: StopProcessRequest) -> Result<(), ProcessError>;
}

#[derive(Clone)]
pub struct ProcessContract {
    inner: Arc<dyn ProcessService>,
}

impl ProcessContract {
    pub fn new(inner: Arc<dyn ProcessService>) -> Self {
        Self { inner }
    }

    pub fn prepare_process(&self, process: ProcessSpec) -> Result<PreparedProcess, ProcessError> {
        self.inner.prepare_process(process)
    }

    pub fn start_process(
        &self,
        request: StartProcessRequest,
    ) -> Result<ProcessInstance, ProcessError> {
        self.inner.start_process(request)
    }

    pub fn stop_process(&self, request: StopProcessRequest) -> Result<(), ProcessError> {
        self.inner.stop_process(request)
    }
}
