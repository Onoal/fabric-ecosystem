use std::sync::Arc;

use fabric_resource_server::{
    ServerAdapter, ServerError, ServerExecution, ServerExecutionRequest, ServerSpec,
};

use crate::adapter::runtime::DenoServerRuntime;
use crate::artifact::DenoServerArtifactResolver;
use crate::config::DenoServerConfig;

pub struct DenoServerAdapter {
    runtime: DenoServerRuntime,
}

impl DenoServerAdapter {
    pub fn new(
        config: DenoServerConfig,
        resolver: Arc<dyn DenoServerArtifactResolver>,
    ) -> Result<Self, ServerError> {
        Ok(Self {
            runtime: DenoServerRuntime::new(config, resolver)?,
        })
    }
}

impl ServerAdapter for DenoServerAdapter {
    fn supports_http_dispatch(&self) -> bool {
        true
    }

    fn prepare(&mut self, server: &ServerSpec) -> Result<(), ServerError> {
        self.runtime.prepare_server(server)
    }

    fn start(
        &mut self,
        request: ServerExecutionRequest,
    ) -> Result<Box<dyn ServerExecution>, ServerError> {
        self.runtime.start_execution(request)
    }

    fn clear(&mut self) {
        self.runtime.clear();
    }
}
