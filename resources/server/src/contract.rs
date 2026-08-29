use std::sync::Arc;

use fabric_core::{ContractId, ContractKey};

use crate::{
    DispatchServerHttpRequest, PreparedServer, ServerError, ServerHttpResponse, ServerInstance,
    ServerSpec, StartServerRequest, StopServerRequest,
};

const SERVER_CONTRACT_ID: &str = "fabric.resource.server";
const SERVER_HTTP_CONTRACT_ID: &str = "fabric.resource.server.http";

pub fn server_contract_id() -> ContractId {
    ContractId::new(SERVER_CONTRACT_ID).expect("static server contract id")
}

pub fn server_contract_key() -> ContractKey<ServerContract> {
    ContractKey::provisional(server_contract_id())
}

pub fn server_http_contract_id() -> ContractId {
    ContractId::new(SERVER_HTTP_CONTRACT_ID).expect("static server http contract id")
}

pub fn server_http_contract_key() -> ContractKey<ServerHttpContract> {
    ContractKey::provisional(server_http_contract_id())
}

pub trait ServerService: Send + Sync {
    fn prepare_server(&self, server: ServerSpec) -> Result<PreparedServer, ServerError>;
    fn start_server(&self, request: StartServerRequest) -> Result<ServerInstance, ServerError>;
    fn stop_server(&self, request: StopServerRequest) -> Result<(), ServerError>;
}

pub trait ServerHttpService: Send + Sync {
    fn dispatch_http(
        &self,
        request: DispatchServerHttpRequest,
    ) -> Result<ServerHttpResponse, ServerError>;
}

#[derive(Clone)]
pub struct ServerContract {
    inner: Arc<dyn ServerService>,
}

impl ServerContract {
    pub fn new(inner: Arc<dyn ServerService>) -> Self {
        Self { inner }
    }

    pub fn prepare_server(&self, server: ServerSpec) -> Result<PreparedServer, ServerError> {
        self.inner.prepare_server(server)
    }

    pub fn start_server(&self, request: StartServerRequest) -> Result<ServerInstance, ServerError> {
        self.inner.start_server(request)
    }

    pub fn stop_server(&self, request: StopServerRequest) -> Result<(), ServerError> {
        self.inner.stop_server(request)
    }
}

#[derive(Clone)]
pub struct ServerHttpContract {
    inner: Arc<dyn ServerHttpService>,
}

impl ServerHttpContract {
    pub fn new(inner: Arc<dyn ServerHttpService>) -> Self {
        Self { inner }
    }

    pub fn dispatch_http(
        &self,
        request: DispatchServerHttpRequest,
    ) -> Result<ServerHttpResponse, ServerError> {
        self.inner.dispatch_http(request)
    }
}
