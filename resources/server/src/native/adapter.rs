use crate::{
    DispatchServerHttpRequest, ServerError, ServerHttpResponse, ServerInstanceId, ServerSpec,
    StartServerRequest,
};

pub struct ServerExecutionRequest {
    pub instance_id: ServerInstanceId,
    pub start: StartServerRequest,
}

pub trait ServerExecution: Send {
    fn dispatch_http(
        &mut self,
        _request: DispatchServerHttpRequest,
    ) -> Result<ServerHttpResponse, ServerError> {
        Err(ServerError::DispatchFailed {
            message: "server adapter does not support http dispatch".to_owned(),
        })
    }

    fn stop(&mut self) -> Result<(), ServerError>;

    fn cleanup(&mut self);
}

pub trait ServerAdapter: Send {
    fn supports_http_dispatch(&self) -> bool {
        false
    }

    fn prepare(&mut self, server: &ServerSpec) -> Result<(), ServerError>;

    fn start(
        &mut self,
        request: ServerExecutionRequest,
    ) -> Result<Box<dyn ServerExecution>, ServerError>;

    fn clear(&mut self);
}
