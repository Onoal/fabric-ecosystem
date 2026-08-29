use std::sync::Arc;

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, InstanceId,
    ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime, module_factory,
};
use fabric_resource_server::{
    DispatchServerHttpRequest, NativeServer, ServerContract, ServerError, ServerHttpContract,
    ServerHttpRequest, ServerId, ServerSpec, StartServerRequest, StopServerRequest,
};
use sha2::{Digest, Sha256};

use crate::adapter::DenoServerAdapter;
use crate::artifact::{DenoServerArtifactResolver, ResolvedDenoServerArtifact};
use crate::config::DenoServerConfig;

struct MissingResolver;

impl DenoServerArtifactResolver for MissingResolver {
    fn resolve(
        &self,
        _artifact: &fabric_resource_server::ServerArtifact,
    ) -> Result<ResolvedDenoServerArtifact, ServerError> {
        Err(ServerError::PrepareFailed {
            message: "resolver not configured".to_owned(),
        })
    }
}

#[derive(Default)]
struct Capture {
    server: std::sync::Mutex<Option<ServerContract>>,
    server_http: std::sync::Mutex<Option<ServerHttpContract>>,
}

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    requirement: ContractRequirement<ServerContract>,
    http_requirement: ContractRequirement<ServerHttpContract>,
    capture: Arc<Capture>,
}

impl CaptureModule {
    fn new(capture: Arc<Capture>) -> Self {
        Self {
            module_id: ModuleId::new("fabric.resource.server.deno.capture").expect("module id"),
            requirement: ContractRequirement::provisional(
                fabric_resource_server::server_contract_id(),
            ),
            http_requirement: ContractRequirement::provisional(
                fabric_resource_server::server_http_contract_id(),
            ),
            capture,
        }
    }
}

impl ModuleRuntime for CaptureModule {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        Vec::new()
            .into_iter()
            .map(fabric_core::ProvidedContractDeclaration::provisional)
            .collect()
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.http_requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let server = bindings
            .resolve(&self.requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let server_http = bindings
            .resolve_optional(&self.http_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.server.lock().expect("capture lock") = Some((*server).clone());
        *self.capture.server_http.lock().expect("http capture lock") =
            server_http.as_deref().cloned();
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn stop(&mut self) {}

    fn health(&self) -> fabric_core::Health {
        fabric_core::Health::Healthy
    }
}

fn deno_server_module(
    config: DenoServerConfig,
    resolver: Arc<dyn DenoServerArtifactResolver>,
) -> impl fabric_core::Module {
    module_factory(move || {
        NativeServer::with_adapter(Box::new(
            DenoServerAdapter::new(config.clone(), Arc::clone(&resolver)).expect("adapter"),
        ))
    })
}

#[test]
fn normal_suite_does_not_require_deno_bin() {
    let config = DenoServerConfig::new(
        std::path::PathBuf::from("/nonexistent/deno"),
        tempfile::tempdir().expect("tempdir").path().join("runtime"),
    );
    let _adapter = DenoServerAdapter::new(config, Arc::new(MissingResolver)).expect("adapter");
}

#[test]
fn deno_server_adapter_can_be_composed_without_worker() {
    let config = DenoServerConfig::new(
        std::path::PathBuf::from("/nonexistent/deno"),
        tempfile::tempdir().expect("tempdir").path().join("runtime"),
    );
    let resolver: Arc<dyn DenoServerArtifactResolver> = Arc::new(MissingResolver);
    let capture = Arc::new(Capture::default());
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.server.deno.donor".to_owned()).expect("composition"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("server".to_owned()).expect("block id"))
            .register_module(deno_server_module(config, resolver))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("capture".to_owned()).expect("capture block id"))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let mut instance = composition
        .materialize(
            InstanceId::new("fabric.resource.server.deno.donor".to_owned()).expect("instance id"),
        )
        .expect("materialize");
    instance.start().expect("start");
    assert!(
        capture
            .server
            .lock()
            .expect("server capture")
            .clone()
            .is_some()
    );
    assert!(
        capture
            .server_http
            .lock()
            .expect("server http capture")
            .clone()
            .is_some()
    );
}

#[test]
#[ignore = "requires DENO_BIN=/path/to/deno 2.9.5 and unrestricted loopback listeners"]
fn real_deno_server_serves_http_until_stop() {
    let deno_bin = std::env::var("DENO_BIN").expect("DENO_BIN");
    let root = tempfile::tempdir().expect("tempdir");
    let module_root = root.path().join("module");
    std::fs::create_dir_all(&module_root).expect("module root");
    let entry = module_root.join("server.ts");
    std::fs::write(
        &entry,
        r#"
const host = Deno.env.get("FABRIC_DENO_SERVER_HOST");
const port = Number(Deno.env.get("FABRIC_DENO_SERVER_PORT"));

if (!host || !Number.isFinite(port)) {
  throw new Error("missing server host/port configuration");
}

Deno.serve({ hostname: host, port }, (request) => {
  const url = new URL(request.url);
  return new Response(`server:${url.pathname}`, { status: 200 });
});
"#,
    )
    .expect("write entry");
    let artifact_file = root.path().join("bundle.tar");
    std::fs::write(&artifact_file, b"fabric-server-test").expect("artifact");
    let digest = Sha256::digest(std::fs::read(&artifact_file).expect("read artifact"));

    struct Resolver {
        artifact_file: std::path::PathBuf,
        module_root: std::path::PathBuf,
    }
    impl DenoServerArtifactResolver for Resolver {
        fn resolve(
            &self,
            _artifact: &fabric_resource_server::ServerArtifact,
        ) -> Result<ResolvedDenoServerArtifact, ServerError> {
            Ok(ResolvedDenoServerArtifact {
                artifact_file: self.artifact_file.clone(),
                module_root: self.module_root.clone(),
            })
        }
    }

    let capture = Arc::new(Capture::default());
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.server.deno.real".to_owned()).expect("composition"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("server".to_owned()).expect("block id"))
            .register_module(deno_server_module(
                DenoServerConfig::new(deno_bin.into(), root.path().join("runtime")),
                Arc::new(Resolver {
                    artifact_file: artifact_file.clone(),
                    module_root: module_root.clone(),
                }),
            ))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("capture".to_owned()).expect("capture block id"))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");
    let mut instance = composition
        .materialize(
            InstanceId::new("fabric.resource.server.deno.real".to_owned()).expect("instance id"),
        )
        .expect("materialize");
    instance.start().expect("start");

    let server = capture
        .server
        .lock()
        .expect("server capture")
        .clone()
        .expect("server");
    let server_http = capture
        .server_http
        .lock()
        .expect("server http capture")
        .clone()
        .expect("server http");
    let spec = ServerSpec {
        server_id: ServerId::new("deno-server").expect("server id"),
        artifact: fabric_resource_server::ServerArtifact {
            reference: "fabric://artifact/deno-server".to_owned(),
            sha256: digest.into(),
        },
        entrypoint: fabric_resource_server::ServerEntrypoint::new("server.ts").expect("entrypoint"),
    };
    let prepared = server.prepare_server(spec.clone()).expect("prepare");
    let live = server
        .start_server(StartServerRequest {
            prepared_server: prepared,
            server: spec,
        })
        .expect("start");
    let response = server_http
        .dispatch_http(DispatchServerHttpRequest {
            server_instance_id: live.server_instance_id.clone(),
            request: ServerHttpRequest {
                method: "GET".to_owned(),
                url: "http://server.local/ping".to_owned(),
                headers: Vec::new(),
                body: Vec::new(),
            },
        })
        .expect("dispatch");
    assert_eq!(response.status, 200);
    assert_eq!(
        String::from_utf8(response.body).expect("body"),
        "server:/ping"
    );
    server
        .stop_server(StopServerRequest {
            server_instance_id: live.server_instance_id,
        })
        .expect("stop");
}
