use crate::{ProcessError, ProcessInstanceId, ProcessSpec, StartProcessRequest};

pub struct ProcessExecutionRequest {
    pub instance_id: ProcessInstanceId,
    pub start: StartProcessRequest,
}

pub trait ProcessExecution: Send {
    fn stop(&mut self) -> Result<(), ProcessError>;

    fn cleanup(&mut self);
}

pub trait ProcessAdapter: Send {
    fn prepare(&mut self, process: &ProcessSpec) -> Result<(), ProcessError>;

    fn start(
        &mut self,
        request: ProcessExecutionRequest,
    ) -> Result<Box<dyn ProcessExecution>, ProcessError>;

    fn clear(&mut self);
}
