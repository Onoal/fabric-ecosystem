use std::path::Path;
use std::sync::Arc;

use fabric_adapter_authority_cedar::CedarAuthorityDecisionAdapter;
use fabric_adapter_database_sqlite::SqliteDatabaseAdapter;
use fabric_adapter_ingress_pingora::{PingoraIngressAdapter, PingoraIngressConfig};
use fabric_adapter_kv_fjall::{FjallKvAdapter, FjallKvConfig};
use fabric_adapter_worker_deno::{DenoWorkerAdapter, DenoWorkerConfig};
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, InstanceId, module_factory,
};
use fabric_resource::{ResourceBoundaryId, ResourceContext, ResourceName};
use fabric_resource_authority::{
    ActionId, ActorRef, AuthorityContract, AuthorityDecision, AuthorityRequest, AuthorityScopeId,
    NativeAuthority, NativeAuthorityConfig, RequestContext, ResourceRef,
};
use fabric_resource_connectivity::{NativeConnectivity, NativeConnectivityConfig};
use fabric_resource_database::{DatabaseConfig, NativeDatabase};
use fabric_resource_identity::{NativeIdentity, NativeIdentityConfig};
use fabric_resource_ingress::NativeIngress;
use fabric_resource_kv::NativeKv;
use fabric_resource_secrets::{
    AuthorizedSecretRef, NativeSecrets, NativeSecretsConfig, SecretMaterial, SecretRef,
};
use fabric_resource_service::{
    MarkServiceTargetReadyRequest, NativeServices, NativeServicesConfig,
    RegisterServiceTargetRequest, ServiceError, ServiceHttpHeader, ServiceHttpRequest,
    ServiceHttpResponse, ServiceHttpTargetRuntime, ServiceHttpTargetRuntimeService,
    ServiceProtocol, ServiceRequirement, ServiceScope, ServiceTarget, ServiceTargetId,
    ServiceTargetState,
};
use fabric_resource_worker::{
    BindingName, BindingTarget, DispatchHttpRequest, HttpRequest, NativeWorker,
    NativeWorkloadProjection, StartWorkerRequest, WorkerApiVersion, WorkerFeature,
    WorkerRequirement, WorkerSpec, WorkloadArtifact, WorkloadBinding, WorkloadBindingEnv,
    WorkloadBindingProjection, WorkloadEntrypoint, WorkloadId,
};
use serde_json::Value;

use crate::helpers::artifact::{ArtifactFixture, create_artifact_fixture};
use crate::helpers::capture::{CaptureModule, CapturedContracts};
use crate::helpers::layout::HarnessLayout;

const AUTHORIZED_ACTION: &str = "e9.read";
const RESOURCE_KIND: &str = "e9-resource";
const RESOURCE_ID: &str = "shared";
const SECRET_VALUE: &str = "e9-secret-material";
const SECRET_BINDING: &str = "SECRET_ENV";
const SECRET_ENV: &str = "FABRIC_TEST_SECRET";
const DB_BINDING: &str = "DB";
const DB_BASE_URL_ENV: &str = "DB_BASE_URL";
const KV_BINDING: &str = "CACHE";
const DB_RESOURCE: &str = "state";
const KV_RESOURCE: &str = "cache";
const SERVICE_SCOPE: &str = "e9.semantic.worker";
const SERVICE_NAME: &str = "web";
const SERVICE_ENV: &str = "SERVICE_BASE_URL";
const SERVICE_BINDING: &str = "API";

#[derive(Clone)]
struct FixedResponseRuntime {
    expected_method: String,
    expected_url: String,
    expected_header: (String, String),
}

impl ServiceHttpTargetRuntimeService for FixedResponseRuntime {
    fn dispatch_http(
        &self,
        _endpoint_id: &fabric_resource_service::ServiceEndpointId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, ServiceError> {
        assert_eq!(request.method, self.expected_method);
        assert_eq!(request.url, self.expected_url);
        let header = request
            .headers
            .into_iter()
            .find(|header| header.name.eq_ignore_ascii_case(&self.expected_header.0))
            .expect("service header");
        assert_eq!(header.value, self.expected_header.1);
        Ok(ServiceHttpResponse {
            status: 201,
            headers: vec![ServiceHttpHeader {
                name: "x-service-proof".to_owned(),
                value: "yes".to_owned(),
            }],
            body: b"service-deno-ok".to_vec(),
        })
    }
}

#[test]
#[ignore = "requires DENO_BIN=/private/tmp/fabric-deno-2.9.5/deno"]
fn real_deno_proves_resource_foundation_consumption_without_components() {
    let deno_bin = std::env::var("DENO_BIN").expect("DENO_BIN");
    let root = tempfile::tempdir().expect("tempdir");
    let layout = HarnessLayout::new(root.path().join("persistent"));
    layout.ensure_filesystem_layout();
    let artifact_fixture = create_artifact_fixture(&layout);
    let captured = CapturedContracts::default();
    let composition = compose(
        &layout,
        &artifact_fixture,
        Path::new(&deno_bin),
        captured.clone(),
    );
    let mut instance = composition
        .materialize(InstanceId::new("fabric.test.resource.foundation.deno").expect("instance id"))
        .expect("materialize composition");
    instance.start().expect("start composition");

    let identity = take_contract(&captured.identity);
    let authority = take_contract(&captured.authority);
    let secrets = take_contract(&captured.secrets);
    let database = take_contract(&captured.database);
    let kv = take_contract(&captured.kv);
    let worker = take_contract(&captured.worker);
    let worker_http = take_contract(&captured.worker_http);
    let service = take_contract(&captured.service);

    let principal = identity.create_principal().expect("create principal");
    let actor = actor_for_principal(&principal.id);
    let authority_scope_id = authority.create_scope().expect("create authority scope");
    let authority_resource = authority_resource(&authority_scope_id);
    let action = authorized_action();
    authority
        .grant_scope_control(&actor, &authority_scope_id)
        .expect("grant scope control");
    authority
        .grant_action(&actor, &action, &authority_resource)
        .expect("grant explicit action");
    assert_eq!(
        authorize(&authority, &actor, &action, &authority_resource),
        AuthorityDecision::Allow
    );

    let secret = secrets
        .create(
            &actor,
            &authority_scope_id,
            SecretMaterial::new(SECRET_VALUE),
        )
        .expect("create secret");
    let resource_context =
        ResourceContext::root(ResourceBoundaryId::new("e9").expect("resource boundary"));
    let prepared_database = database
        .prepare(
            resource_context.clone(),
            ResourceName::new(DB_RESOURCE).expect("database name"),
        )
        .expect("prepare database");
    let prepared_kv = kv
        .prepare(
            resource_context,
            ResourceName::new(KV_RESOURCE).expect("kv name"),
        )
        .expect("prepare kv");

    let worker_spec = workload_spec(artifact_fixture.artifact.clone());
    let prepared_worker = worker
        .prepare_worker(worker_spec.clone())
        .expect("prepare worker");
    let database_binding = WorkloadBinding::new(
        worker_spec.workload_id.clone(),
        BindingName::new(DB_BINDING).expect("db binding"),
        BindingTarget::database(prepared_database.database_ref()),
    );
    let kv_binding = WorkloadBinding::new(
        worker_spec.workload_id.clone(),
        BindingName::new(KV_BINDING).expect("kv binding"),
        BindingTarget::kv(prepared_kv.kv_ref()),
    );
    let secret_binding = WorkloadBinding::new(
        worker_spec.workload_id.clone(),
        BindingName::new(SECRET_BINDING).expect("secret binding"),
        BindingTarget::secret(AuthorizedSecretRef::new(
            secret.secret.clone(),
            actor.clone(),
        )),
    );

    let service_scope = ServiceScope::new(SERVICE_SCOPE).expect("service scope");
    let service_requirement =
        ServiceRequirement::new(ServiceProtocol::Http, SERVICE_NAME).expect("service requirement");
    let service_model = service
        .ensure_service(&service_scope, &service_requirement)
        .expect("ensure service")
        .service;
    let service_target = ServiceTarget {
        id: ServiceTargetId::new("e9_service_target").expect("target id"),
        endpoint_id: service_model.endpoint.id.clone(),
        state: ServiceTargetState::Registered,
    };
    service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service_model.id.clone(),
                target: service_target.clone(),
            },
            ServiceHttpTargetRuntime::new(Arc::new(FixedResponseRuntime {
                expected_method: "GET".to_owned(),
                expected_url: "/status?check=1".to_owned(),
                expected_header: ("x-e9-proof".to_owned(), "yes".to_owned()),
            })),
        )
        .expect("register service target");
    service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service_model.id.clone(),
            target_id: service_target.id.clone(),
        })
        .expect("mark target ready");
    let service_binding = WorkloadBinding::new(
        worker_spec.workload_id.clone(),
        BindingName::new(SERVICE_BINDING).expect("service binding"),
        BindingTarget::service(service_model.id.clone()),
    );

    let worker_instance = worker
        .start_worker(StartWorkerRequest {
            prepared_worker,
            worker: worker_spec.clone(),
            bindings: vec![
                database_binding.clone(),
                kv_binding.clone(),
                secret_binding.clone(),
                service_binding.clone(),
            ],
            binding_projections: vec![
                WorkloadBindingProjection::structured(database_binding.binding_id()),
                WorkloadBindingProjection::structured(kv_binding.binding_id()),
                WorkloadBindingProjection::environment(
                    database_binding.binding_id(),
                    WorkloadBindingEnv::new(DB_BASE_URL_ENV).expect("db env"),
                ),
                WorkloadBindingProjection::environment(
                    secret_binding.binding_id(),
                    WorkloadBindingEnv::new(SECRET_ENV).expect("secret env"),
                ),
                WorkloadBindingProjection::environment(
                    service_binding.binding_id(),
                    WorkloadBindingEnv::new(SERVICE_ENV).expect("service env"),
                ),
            ],
        })
        .expect("start worker");

    let state = request_json(
        &worker_http,
        &worker_instance.worker_instance_id,
        "GET",
        "/state",
    );
    assert_eq!(state["dbValue"], Value::Null);
    assert_eq!(state["kvValue"], Value::Null);
    assert_eq!(state["secretValue"], SECRET_VALUE);

    let write = request_json_with_body(
        &worker_http,
        &worker_instance.worker_instance_id,
        "POST",
        "/state",
        br#"{"dbValue":"db-one","kvValue":"kv-one"}"#.to_vec(),
    );
    assert_eq!(write["dbValue"], "db-one");
    assert_eq!(write["kvValue"], "kv-one");
    assert_eq!(write["secretValue"], SECRET_VALUE);

    let service_check = request_json(
        &worker_http,
        &worker_instance.worker_instance_id,
        "GET",
        "/service-check",
    );
    assert_eq!(service_check["status"], 201);
    assert_eq!(service_check["header"], "yes");
    assert_eq!(service_check["body"], "service-deno-ok");

    assert_secret_binding_does_not_bypass_authorization(
        &worker,
        artifact_fixture.artifact.clone(),
        &secret.secret,
    );

    instance.stop();
}

fn compose(
    layout: &HarnessLayout,
    artifact_fixture: &ArtifactFixture,
    deno_bin: &Path,
    captured: CapturedContracts,
) -> fabric_core::Composition {
    CompositionBuilder::new(
        CompositionId::new("fabric.test.resource.foundation.deno".to_owned())
            .expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("resource".to_owned()).expect("block id"))
            .register_module(fabric_resource_registry::ResourceRegistryModule::new())
            .register_module(NativeIdentity::new(NativeIdentityConfig {
                database_path: layout.identity_db(),
            }))
            .register_module(NativeAuthority::with_decision_adapter(
                NativeAuthorityConfig {
                    database_path: layout.authority_db(),
                },
                Arc::new(CedarAuthorityDecisionAdapter::new()),
            ))
            .register_module(NativeSecrets::new(NativeSecretsConfig {
                database_path: layout.secrets_db(),
            }))
            .register_module(module_factory({
                let database_root = layout.database_root();
                move || {
                    NativeDatabase::with_sqlite_compatibility(
                        DatabaseConfig {
                            root: database_root.clone(),
                        },
                        Arc::new(SqliteDatabaseAdapter::new()),
                    )
                }
            }))
            .register_module(module_factory({
                let kv_root = layout.kv_root();
                move || {
                    NativeKv::new(Arc::new(FjallKvAdapter::new(FjallKvConfig::new(
                        kv_root.clone(),
                    ))))
                }
            }))
            .register_module(NativeWorkloadProjection::new())
            .register_module(module_factory({
                let deno_bin = deno_bin.to_path_buf();
                let runtime_root = layout.worker_runtime_root();
                let resolver = Arc::clone(&artifact_fixture.resolver);
                move || {
                    NativeWorker::with_adapter(Box::new(
                        DenoWorkerAdapter::new(
                            DenoWorkerConfig::new(deno_bin.clone(), runtime_root.clone()),
                            Arc::clone(&resolver),
                        )
                        .expect("deno adapter"),
                    ))
                }
            }))
            .register_module(NativeServices::new(NativeServicesConfig {
                database_path: layout.service_db(),
            }))
            .register_module(module_factory(|| {
                NativeIngress::new(Arc::new(PingoraIngressAdapter::new(
                    PingoraIngressConfig::new("127.0.0.1:0".parse().expect("pingora bind address")),
                )))
            }))
            .register_module(NativeConnectivity::new(NativeConnectivityConfig {
                database_path: layout.connectivity_db(),
            }))
            .register_module(CaptureModule::new(captured))
            .build(),
    )
    .build()
    .expect("composition")
}

fn workload_spec(artifact: WorkloadArtifact) -> WorkerSpec {
    workload_spec_for("e9.consumer", artifact)
}

fn take_contract<T: Clone>(slot: &std::sync::Mutex<Option<T>>) -> T {
    slot.lock()
        .expect("capture lock")
        .clone()
        .expect("captured contract")
}

fn actor_for_principal(principal_id: &fabric_resource_identity::PrincipalId) -> ActorRef {
    ActorRef::new(format!("principal:{}", principal_id.as_str())).expect("actor ref")
}

fn authority_resource(scope_id: &AuthorityScopeId) -> ResourceRef {
    ResourceRef::new(
        scope_id.clone(),
        RESOURCE_KIND,
        format!("{RESOURCE_ID}:{}", scope_id.as_str()),
    )
    .expect("authority resource")
}

fn authorized_action() -> ActionId {
    ActionId::new(AUTHORIZED_ACTION).expect("action id")
}

fn authorize(
    authority: &AuthorityContract,
    actor: &ActorRef,
    action: &ActionId,
    resource: &ResourceRef,
) -> AuthorityDecision {
    authority
        .authorize(&AuthorityRequest {
            actor: actor.clone(),
            action: action.clone(),
            resource: resource.clone(),
            context: RequestContext::default(),
        })
        .expect("authorize")
}

fn request_json(
    worker_http: &fabric_resource_worker::WorkerHttpContract,
    worker_instance_id: &fabric_resource_worker::WorkerInstanceId,
    method: &str,
    url: &str,
) -> Value {
    request_json_with_body(worker_http, worker_instance_id, method, url, Vec::new())
}

fn request_json_with_body(
    worker_http: &fabric_resource_worker::WorkerHttpContract,
    worker_instance_id: &fabric_resource_worker::WorkerInstanceId,
    method: &str,
    url: &str,
    body: Vec<u8>,
) -> Value {
    let response = worker_http
        .dispatch_http(DispatchHttpRequest {
            worker_instance_id: worker_instance_id.clone(),
            request: HttpRequest {
                method: method.to_owned(),
                url: url.to_owned(),
                headers: Vec::new(),
                body,
            },
        })
        .expect("dispatch http");
    serde_json::from_slice(&response.body).expect("json body")
}

fn assert_secret_binding_does_not_bypass_authorization(
    worker: &fabric_resource_worker::WorkerContract,
    artifact: WorkloadArtifact,
    secret: &SecretRef,
) {
    let denied_actor = ActorRef::new("e9.denied:secret").expect("denied actor");
    let worker_spec = workload_spec_for("e9.denied", artifact);
    let prepared_worker = worker
        .prepare_worker(worker_spec.clone())
        .expect("prepare denied worker");
    let binding = WorkloadBinding::new(
        worker_spec.workload_id.clone(),
        BindingName::new(SECRET_BINDING).expect("binding name"),
        BindingTarget::secret(AuthorizedSecretRef::new(secret.clone(), denied_actor)),
    );
    let error = worker
        .start_worker(StartWorkerRequest {
            prepared_worker,
            worker: worker_spec,
            bindings: vec![binding.clone()],
            binding_projections: vec![WorkloadBindingProjection::environment(
                binding.binding_id(),
                WorkloadBindingEnv::new("DENIED_SECRET").expect("env"),
            )],
        })
        .expect_err("unauthorized secret projection should fail");
    assert!(matches!(
        error,
        fabric_resource_worker::WorkerError::StartFailed { ref message }
            if message.contains("materialize secret binding")
                && message.contains("secret access is denied")
    ));
}

fn workload_spec_for(workload_id: &str, artifact: WorkloadArtifact) -> WorkerSpec {
    WorkerSpec {
        workload_id: WorkloadId::new(workload_id).expect("workload id"),
        requirement: WorkerRequirement {
            api_version: WorkerApiVersion::parse("1.0.0").expect("api version"),
            features: vec![
                WorkerFeature::new("js.module").expect("feature"),
                WorkerFeature::new("http.fetch").expect("feature"),
            ],
        },
        artifact,
        entrypoint: WorkloadEntrypoint::new("main.ts").expect("entrypoint"),
    }
}
