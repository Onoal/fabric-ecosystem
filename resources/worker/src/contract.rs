use std::sync::Arc;

use fabric_core::{ContractId, ContractKey};

use crate::{
    DispatchHttpRequest, HttpResponse, PreparedWorker, StartWorkerRequest, StopWorkerRequest,
    WorkerCapabilities, WorkerError, WorkerInstance, WorkerSpec,
};

const WORKER_CONTRACT_ID: &str = "fabric.resource.worker";
const WORKER_HTTP_CONTRACT_ID: &str = "fabric.resource.worker.http";

pub fn worker_contract_id() -> ContractId {
    ContractId::new(WORKER_CONTRACT_ID).expect("static worker contract id")
}

pub fn worker_contract_key() -> ContractKey<WorkerContract> {
    ContractKey::provisional(worker_contract_id())
}

pub fn worker_http_contract_id() -> ContractId {
    ContractId::new(WORKER_HTTP_CONTRACT_ID).expect("static worker http contract id")
}

pub fn worker_http_contract_key() -> ContractKey<WorkerHttpContract> {
    ContractKey::provisional(worker_http_contract_id())
}

pub trait WorkerService: Send + Sync {
    fn inspect_capabilities(&self) -> Result<WorkerCapabilities, WorkerError>;
    fn prepare_worker(&self, worker: WorkerSpec) -> Result<PreparedWorker, WorkerError>;
    fn start_worker(&self, request: StartWorkerRequest) -> Result<WorkerInstance, WorkerError>;
    fn stop_worker(&self, request: StopWorkerRequest) -> Result<(), WorkerError>;
}

pub trait WorkerHttpService: Send + Sync {
    fn dispatch_http(&self, request: DispatchHttpRequest) -> Result<HttpResponse, WorkerError>;
}

#[derive(Clone)]
pub struct WorkerContract {
    inner: Arc<dyn WorkerService>,
}

impl WorkerContract {
    pub fn new(inner: Arc<dyn WorkerService>) -> Self {
        Self { inner }
    }

    pub fn inspect_capabilities(&self) -> Result<WorkerCapabilities, WorkerError> {
        self.inner.inspect_capabilities()
    }

    pub fn prepare_worker(&self, worker: WorkerSpec) -> Result<PreparedWorker, WorkerError> {
        self.inner.prepare_worker(worker)
    }

    pub fn start_worker(&self, request: StartWorkerRequest) -> Result<WorkerInstance, WorkerError> {
        self.inner.start_worker(request)
    }

    pub fn stop_worker(&self, request: StopWorkerRequest) -> Result<(), WorkerError> {
        self.inner.stop_worker(request)
    }
}

#[derive(Clone)]
pub struct WorkerHttpContract {
    inner: Arc<dyn WorkerHttpService>,
}

impl WorkerHttpContract {
    pub fn new(inner: Arc<dyn WorkerHttpService>) -> Self {
        Self { inner }
    }

    pub fn dispatch_http(&self, request: DispatchHttpRequest) -> Result<HttpResponse, WorkerError> {
        self.inner.dispatch_http(request)
    }
}
