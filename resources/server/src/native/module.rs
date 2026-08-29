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

use crate::contract::{
    ServerContract, ServerHttpContract, ServerHttpService, ServerService, server_contract_key,
    server_http_contract_key,
};
use crate::native::adapter::{ServerAdapter, ServerExecution, ServerExecutionRequest};
use crate::{
    DispatchServerHttpRequest, PreparedServer, ServerError, ServerHttpResponse, ServerInstance,
    ServerInstanceId, ServerInstanceStatus, ServerSpec, StartServerRequest, StopServerRequest,
};

pub fn server_resource_id() -> ResourceId {
    ResourceId::new("server").expect("static server resource id")
}

pub struct NativeServer {
    module_id: ModuleId,
    resource_registry_requirement: ContractRequirement<ResourceRegistry>,
    resource_registry: Option<ResourceRegistry>,
    shared: Arc<SharedServerState>,
    supports_http_dispatch: bool,
}

pub(crate) struct SharedServerState {
    inner: Mutex<ServerState>,
}

struct ServerState {
    health: Health,
    started: bool,
    adapter: Box<dyn ServerAdapter>,
    live: BTreeMap<ServerInstanceId, LiveServerInstance>,
}

struct LiveServerInstance {
    instance: ServerInstance,
    execution: Box<dyn ServerExecution>,
}

impl NativeServer {
    pub fn with_adapter(adapter: Box<dyn ServerAdapter>) -> Self {
        let supports_http_dispatch = adapter.supports_http_dispatch();
        Self {
            module_id: ModuleId::new("fabric.resource.server.native")
                .expect("static server module id"),
            resource_registry_requirement: ContractRequirement::provisional(
                resource_registry_contract_id(),
            ),
            resource_registry: None,
            shared: Arc::new(SharedServerState {
                inner: Mutex::new(ServerState {
                    health: Health::Unavailable,
                    started: false,
                    adapter,
                    live: BTreeMap::new(),
                }),
            }),
            supports_http_dispatch,
        }
    }
}

impl ModuleRuntime for NativeServer {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        let mut contracts = vec![server_contract_key().declaration()];
        if self.supports_http_dispatch {
            contracts.push(server_http_contract_key().declaration());
        }
        contracts
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.resource_registry_requirement.declaration().clone()]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        let server_service: Arc<dyn ServerService> =
            Arc::clone(&self.shared) as Arc<dyn ServerService>;
        let mut contracts = vec![ModuleContract::new(
            &server_contract_key(),
            Arc::new(ServerContract::new(server_service)),
        )];
        if self.supports_http_dispatch {
            let http_service: Arc<dyn ServerHttpService> =
                Arc::clone(&self.shared) as Arc<dyn ServerHttpService>;
            contracts.push(ModuleContract::new(
                &server_http_contract_key(),
                Arc::new(ServerHttpContract::new(http_service)),
            ));
        }
        Ok(contracts)
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
        let mut state = self.shared.inner.lock().expect("server state lock");
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
                .register(self, ResourceDescriptor::new(server_resource_id()))
                .map_err(|error| ModuleError::new(error.to_string()))?;
        }
        let mut state = self.shared.inner.lock().expect("server state lock");
        state.started = true;
        state.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(registry) = &self.resource_registry {
            let _ = registry.unregister(&self.module_id);
        }
        let mut state = self.shared.inner.lock().expect("server state lock");
        for (_, mut live) in std::mem::take(&mut state.live) {
            live.execution.cleanup();
        }
        state.adapter.clear();
        state.started = false;
        state.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.shared.inner.lock().expect("server state lock").health
    }
}

impl ServerService for SharedServerState {
    fn prepare_server(&self, server: ServerSpec) -> Result<PreparedServer, ServerError> {
        self.with_state(|state| {
            state.adapter.prepare(&server)?;
            Ok(PreparedServer::new(server))
        })
    }

    fn start_server(&self, request: StartServerRequest) -> Result<ServerInstance, ServerError> {
        self.with_state(|state| {
            if request.prepared_server.server_id != request.server.server_id {
                return Err(ServerError::ProtocolViolation {
                    message: "prepared server does not match start server id".to_owned(),
                });
            }
            if !request.prepared_server.matches(&request.server) {
                return Err(ServerError::ProtocolViolation {
                    message: "prepared server does not match start server".to_owned(),
                });
            }
            let server_instance_id = ServerInstanceId::new(hex::encode(rand::random::<[u8; 16]>()))
                .map_err(|error| ServerError::StartFailed {
                    message: error.to_string(),
                })?;
            let instance = ServerInstance {
                server_instance_id: server_instance_id.clone(),
                server_id: request.server.server_id.clone(),
                status: ServerInstanceStatus::Running,
            };
            let execution = state.adapter.start(ServerExecutionRequest {
                instance_id: server_instance_id.clone(),
                start: request,
            })?;
            state.live.insert(
                server_instance_id,
                LiveServerInstance {
                    instance: instance.clone(),
                    execution,
                },
            );
            Ok(instance)
        })
    }

    fn stop_server(&self, request: StopServerRequest) -> Result<(), ServerError> {
        self.with_state(|state| {
            if let Some(mut live) = state.live.remove(&request.server_instance_id) {
                let stop_result = live.execution.stop();
                live.execution.cleanup();
                stop_result?;
            }
            Ok(())
        })
    }
}

impl ServerHttpService for SharedServerState {
    fn dispatch_http(
        &self,
        request: DispatchServerHttpRequest,
    ) -> Result<ServerHttpResponse, ServerError> {
        self.with_state(|state| {
            let live = state
                .live
                .get_mut(&request.server_instance_id)
                .ok_or_else(|| ServerError::ProtocolViolation {
                    message: format!(
                        "server instance {} is not live",
                        request.server_instance_id.as_str()
                    ),
                })?;
            let _server_id = live.instance.server_id.clone();
            live.execution.dispatch_http(request)
        })
    }
}

impl SharedServerState {
    fn with_state<T>(
        &self,
        action: impl FnOnce(&mut ServerState) -> Result<T, ServerError>,
    ) -> Result<T, ServerError> {
        let mut state = self.inner.lock().expect("server state lock");
        if !state.started {
            return Err(ServerError::Unavailable);
        }
        action(&mut state)
    }
}
