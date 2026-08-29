use std::sync::Arc;

use fabric_resource_worker::{
    WorkerAdapter, WorkerCapabilities, WorkerError, WorkerExecution, WorkerExecutionRequest,
    WorkerSpec,
};

use crate::adapter::runtime::DenoWorkerRuntime;
use crate::artifact::DenoArtifactResolver;
use crate::config::DenoWorkerConfig;

pub struct DenoWorkerAdapter {
    runtime: DenoWorkerRuntime,
}

impl DenoWorkerAdapter {
    pub fn new(
        config: DenoWorkerConfig,
        resolver: Arc<dyn DenoArtifactResolver>,
    ) -> Result<Self, WorkerError> {
        Ok(Self {
            runtime: DenoWorkerRuntime::new(config, resolver)?,
        })
    }
}

impl WorkerAdapter for DenoWorkerAdapter {
    fn capabilities(&self) -> WorkerCapabilities {
        self.runtime.capabilities()
    }

    fn supports_http_dispatch(&self) -> bool {
        true
    }

    fn prepare(&mut self, worker: &WorkerSpec) -> Result<(), WorkerError> {
        self.runtime.prepare_worker(worker)
    }

    fn start(
        &mut self,
        request: WorkerExecutionRequest,
    ) -> Result<Box<dyn WorkerExecution>, WorkerError> {
        self.runtime.start_execution(request)
    }

    fn clear(&mut self) {
        self.runtime.clear();
    }
}
