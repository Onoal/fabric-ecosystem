use crate::projection::PreparedWorkloadProjections;
use crate::{
    DispatchHttpRequest, HttpResponse, StartWorkerRequest, WorkerCapabilities, WorkerError,
    WorkerInstanceId, WorkerSpec,
};

pub struct WorkerExecutionRequest {
    pub instance_id: WorkerInstanceId,
    pub start: StartWorkerRequest,
    pub projections: PreparedWorkloadProjections,
}

pub trait WorkerExecution: Send {
    fn dispatch_http(
        &mut self,
        _request: DispatchHttpRequest,
    ) -> Result<HttpResponse, WorkerError> {
        Err(WorkerError::DispatchFailed {
            message: "worker adapter does not support http dispatch".to_owned(),
        })
    }

    fn stop(&mut self) -> Result<(), WorkerError>;

    fn cleanup(&mut self);
}

pub trait WorkerAdapter: Send {
    fn capabilities(&self) -> WorkerCapabilities;

    fn supports_http_dispatch(&self) -> bool {
        false
    }

    fn prepare(&mut self, worker: &WorkerSpec) -> Result<(), WorkerError>;

    fn start(
        &mut self,
        request: WorkerExecutionRequest,
    ) -> Result<Box<dyn WorkerExecution>, WorkerError>;

    fn clear(&mut self);
}
