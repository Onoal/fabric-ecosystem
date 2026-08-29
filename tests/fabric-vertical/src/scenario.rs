use std::path::Path;
use std::sync::Arc;

use fabric_adapter_authority_cedar::CedarAuthorityDecisionAdapter;
use fabric_adapter_database_sqlite::SqliteDatabaseAdapter;
use fabric_adapter_ingress_pingora::{PingoraIngressAdapter, PingoraIngressConfig};
use fabric_adapter_kv_fjall::{FjallKvAdapter, FjallKvConfig};
use fabric_adapter_worker_deno::{DenoWorkerAdapter, DenoWorkerConfig};
use fabric_core::{
    BlockBuilder, BlockId, Composition, CompositionBuilder, CompositionId, Instance, InstanceId,
    module_factory,
};
use fabric_resource::{ResourceBoundaryId, ResourceContext, ResourceName};
use fabric_resource_authority::{
    ActionId, ActorRef, AuthorityContract, AuthorityDecision, AuthorityRequest, AuthorityScopeId,
    NativeAuthority, NativeAuthorityConfig, RequestContext, ResourceRef,
};
use fabric_resource_connectivity::{
    ConnectivityError, ConnectivityScope, LocalConnectivityAccess, NativeConnectivity,
    NativeConnectivityConfig, Reachability,
};
use fabric_resource_database::{DatabaseConfig, DatabaseRef, NativeDatabase};
use fabric_resource_identity::{NativeIdentity, NativeIdentityConfig, Principal, PrincipalId};
use fabric_resource_ingress::NativeIngress;
use fabric_resource_kv::{KvRef, NativeKv};
use fabric_resource_secrets::{
    AuthorizedSecretRef, NativeSecrets, NativeSecretsConfig, SecretMaterial, SecretRef,
    StoredSecret,
};
use fabric_resource_service::{
    MarkServiceTargetReadyRequest, NativeServices, NativeServicesConfig,
    RegisterServiceTargetRequest, Service, ServiceHttpTargetRuntime, ServiceId, ServiceRequirement,
    ServiceScope, ServiceTarget, ServiceTargetId, ServiceTargetState,
};
use fabric_resource_worker::{
    BindingName, BindingTarget, DispatchHttpRequest, HttpRequest, HttpResponse, NativeWorker,
    NativeWorkloadProjection, StartWorkerRequest, StopWorkerRequest, WorkerApiVersion,
    WorkerContract, WorkerError, WorkerFeature, WorkerHttpContract, WorkerInstance,
    WorkerRequirement, WorkerSpec, WorkloadArtifact, WorkloadBinding, WorkloadBindingEnv,
    WorkloadBindingProjection, WorkloadEntrypoint, WorkloadId,
};
use sha2::Digest;

use crate::artifact::{ArtifactFixture, create_artifact_fixture};
use crate::capture::{CaptureModule, CapturedContracts};
use crate::layout::HarnessLayout;
use crate::runtime_adapter::WorkerServiceTargetRuntime;

const AUTHORIZED_ACTION: &str = "fv0.read";
const RESOURCE_KIND: &str = "fv0-resource";
const RESOURCE_ID: &str = "shared";
const SECRET_VALUE: &str = "fv0-secret-material";
const SECRET_BINDING: &str = "SECRET_ENV";
const SECRET_ENV: &str = "FABRIC_TEST_SECRET";
const DB_BINDING: &str = "DB";
const DB_BASE_URL_ENV: &str = "DB_BASE_URL";
const KV_BINDING: &str = "CACHE";
const DB_RESOURCE: &str = "state";
const KV_RESOURCE: &str = "cache";
const SERVICE_SCOPE: &str = "fv0.semantic.worker";
const SERVICE_NAME: &str = "web";

#[derive(Clone)]
struct StableBootState {
    principal_id: PrincipalId,
    actor: ActorRef,
    authority_scope_id: AuthorityScopeId,
    secret_ref: SecretRef,
    database_ref: DatabaseRef,
    kv_ref: KvRef,
    service_id: ServiceId,
    service_endpoint_id: fabric_resource_service::ServiceEndpointId,
    ingress_route_id: fabric_resource_ingress::IngressRouteId,
    reachability_id: fabric_resource_connectivity::ReachabilityId,
}

struct BootState {
    instance: Instance,
    principal: Principal,
    actor: ActorRef,
    authority_scope_id: AuthorityScopeId,
    secret: StoredSecret,
    database_ref: DatabaseRef,
    kv_ref: KvRef,
    worker_instance: WorkerInstance,
    service: Service,
    service_target: ServiceTarget,
    reachability: Reachability,
    local_access: LocalConnectivityAccess,
}

#[test]
#[ignore = "requires DENO_BIN=/private/tmp/fabric-deno-2.9.5/deno"]
fn real_fv0_proves_pure_semantic_fabric_vertical() {
    let deno_bin = std::env::var("DENO_BIN").expect("DENO_BIN");
    let root = tempfile::tempdir().expect("tempdir");
    let layout = HarnessLayout::new(root.path().join("persistent"));
    layout.ensure_filesystem_layout();
    let artifact_fixture = create_artifact_fixture(&layout);

    let mut first_boot = boot(&layout, &artifact_fixture, Path::new(&deno_bin), 1, None);
    let first_url = first_boot.local_access.url.clone();
    let first_worker_instance_id = first_boot.worker_instance.worker_instance_id.clone();
    let first_target_id = first_boot.service_target.id.clone();
    let first_stderr_log = workload_stderr_log(&layout, &first_worker_instance_id);

    let first_write = post_state(
        &first_boot.local_access,
        &first_stderr_log,
        "db-first",
        "kv-first",
    );
    assert_eq!(first_write["dbValue"], "db-first");
    assert_eq!(first_write["kvValue"], "kv-first");
    assert_eq!(first_write["secretValue"], SECRET_VALUE);

    let stable = StableBootState {
        principal_id: first_boot.principal.id.clone(),
        actor: first_boot.actor.clone(),
        authority_scope_id: first_boot.authority_scope_id.clone(),
        secret_ref: first_boot.secret.secret.clone(),
        database_ref: first_boot.database_ref.clone(),
        kv_ref: first_boot.kv_ref.clone(),
        service_id: first_boot.service.id.clone(),
        service_endpoint_id: first_boot.service.endpoint.id.clone(),
        ingress_route_id: first_boot.reachability.ingress_route_id.clone(),
        reachability_id: first_boot.reachability.id.clone(),
    };

    first_boot.instance.stop();
    assert_old_access_is_stale(&first_url);
    assert_worker_runtime_root_is_cleared(&layout);

    let second_boot = boot(
        &layout,
        &artifact_fixture,
        Path::new(&deno_bin),
        2,
        Some(&stable),
    );

    assert_eq!(second_boot.principal.id, stable.principal_id);
    assert_eq!(second_boot.actor, stable.actor);
    assert_eq!(second_boot.authority_scope_id, stable.authority_scope_id);
    assert_eq!(second_boot.secret.secret, stable.secret_ref);
    assert_eq!(second_boot.database_ref, stable.database_ref);
    assert_eq!(second_boot.kv_ref, stable.kv_ref);
    assert_eq!(second_boot.service.id, stable.service_id);
    assert_eq!(second_boot.service.endpoint.id, stable.service_endpoint_id);
    assert_eq!(
        second_boot.reachability.ingress_route_id,
        stable.ingress_route_id
    );
    assert_eq!(second_boot.reachability.id, stable.reachability_id);

    assert_ne!(
        second_boot.worker_instance.worker_instance_id,
        first_worker_instance_id
    );
    assert_ne!(second_boot.service_target.id, first_target_id);
    let second_stderr_log =
        workload_stderr_log(&layout, &second_boot.worker_instance.worker_instance_id);

    let second_read = get_state(&second_boot.local_access, &second_stderr_log);
    assert_eq!(second_read["dbValue"], "db-first");
    assert_eq!(second_read["kvValue"], "kv-first");
    assert_eq!(second_read["secretValue"], SECRET_VALUE);
}

fn boot(
    layout: &HarnessLayout,
    artifact_fixture: &ArtifactFixture,
    deno_bin: &Path,
    boot_index: usize,
    previous: Option<&StableBootState>,
) -> BootState {
    let captured = CapturedContracts::default();
    let composition = compose(layout, artifact_fixture, deno_bin, captured.clone());
    let mut instance = composition
        .materialize(InstanceId::new("fabric.test.vertical.fv0").expect("instance id"))
        .expect("materialize composition");
    instance.start().expect("start fv0 composition");

    let identity = take_contract(&captured.identity);
    let authority = take_contract(&captured.authority);
    let secrets = take_contract(&captured.secrets);
    let database = take_contract(&captured.database);
    let kv = take_contract(&captured.kv);
    let worker = take_contract(&captured.worker);
    let worker_http = take_contract(&captured.worker_http);
    let service = take_contract(&captured.service);
    let ingress = take_contract(&captured.ingress);
    let connectivity = take_contract(&captured.connectivity);
    let local_connectivity_access = take_contract(&captured.local_connectivity_access);

    let principal = match previous {
        Some(previous) => identity
            .get_principal(&previous.principal_id)
            .expect("get persistent principal")
            .expect("principal exists"),
        None => identity.create_principal().expect("create principal"),
    };

    let actor = previous
        .map(|previous| previous.actor.clone())
        .unwrap_or_else(|| actor_for_principal(&principal.id));
    let authority_scope_id = previous
        .map(|previous| previous.authority_scope_id.clone())
        .unwrap_or_else(|| authority.create_scope().expect("create authority scope"));

    let authority_resource = authority_resource(&authority_scope_id);
    let action = authorized_action();
    if previous.is_none() {
        authority
            .grant_scope_control(&actor, &authority_scope_id)
            .expect("grant scope control");
        authority
            .grant_action(&actor, &action, &authority_resource)
            .expect("grant explicit action");
    }
    assert_eq!(
        authorize(&authority, &actor, &action, &authority_resource),
        AuthorityDecision::Allow
    );
    assert_eq!(
        authorize(
            &authority,
            &ActorRef::new("fv0.other:denied").expect("other actor"),
            &action,
            &authority_resource
        ),
        AuthorityDecision::Deny
    );

    let secret = previous
        .map(|previous| StoredSecret {
            secret: previous.secret_ref.clone(),
            version: secrets
                .materialize(&actor, &previous.secret_ref)
                .expect("materialize persisted secret")
                .version,
        })
        .unwrap_or_else(|| {
            secrets
                .create(
                    &actor,
                    &authority_scope_id,
                    SecretMaterial::new(SECRET_VALUE),
                )
                .expect("create secret")
        });
    let materialized_secret = secrets
        .materialize(&actor, &secret.secret)
        .expect("materialize secret");
    assert_eq!(materialized_secret.value.expose(), SECRET_VALUE);

    let resource_context =
        ResourceContext::root(ResourceBoundaryId::new("fv0").expect("resource boundary"));
    let database_name = ResourceName::new(DB_RESOURCE).expect("database name");
    let kv_name = ResourceName::new(KV_RESOURCE).expect("kv name");
    let prepared_database = database
        .prepare(resource_context.clone(), database_name.clone())
        .expect("prepare database");
    let prepared_kv = kv
        .prepare(resource_context.clone(), kv_name.clone())
        .expect("prepare kv");
    let database_ref = prepared_database.database_ref();
    let kv_ref = prepared_kv.kv_ref();

    let worker_spec = workload_spec(artifact_fixture.artifact.clone());
    let prepared_worker = worker
        .prepare_worker(worker_spec.clone())
        .expect("prepare worker");
    let database_binding = WorkloadBinding::new(
        worker_spec.workload_id.clone(),
        BindingName::new(DB_BINDING).expect("db binding name"),
        BindingTarget::database(database_ref.clone()),
    );
    let kv_binding = WorkloadBinding::new(
        worker_spec.workload_id.clone(),
        BindingName::new(KV_BINDING).expect("kv binding name"),
        BindingTarget::kv(kv_ref.clone()),
    );
    let secret_binding = WorkloadBinding::new(
        worker_spec.workload_id.clone(),
        BindingName::new(SECRET_BINDING).expect("secret binding name"),
        BindingTarget::secret(AuthorizedSecretRef::new(
            secret.secret.clone(),
            actor.clone(),
        )),
    );
    let worker_instance = worker
        .start_worker(StartWorkerRequest {
            prepared_worker,
            worker: worker_spec.clone(),
            bindings: vec![
                database_binding.clone(),
                kv_binding.clone(),
                secret_binding.clone(),
            ],
            binding_projections: vec![
                WorkloadBindingProjection::structured(database_binding.binding_id()),
                WorkloadBindingProjection::structured(kv_binding.binding_id()),
                WorkloadBindingProjection::environment(
                    database_binding.binding_id(),
                    WorkloadBindingEnv::new(DB_BASE_URL_ENV).expect("db env variable"),
                ),
                WorkloadBindingProjection::environment(
                    secret_binding.binding_id(),
                    WorkloadBindingEnv::new(SECRET_ENV).expect("secret env variable"),
                ),
            ],
        })
        .expect("start worker");
    assert_eq!(
        worker_instance.status,
        fabric_resource_worker::WorkerInstanceStatus::Running
    );
    assert_authorized_workload_reads_secret(&worker_http, &worker_instance);
    assert_unbound_workload_cannot_consume_secret(
        &worker,
        &worker_http,
        artifact_fixture.artifact.clone(),
    );
    assert_secret_binding_does_not_bypass_authorization(
        &worker,
        artifact_fixture.artifact.clone(),
        &secret.secret,
    );

    let service_scope = ServiceScope::new(SERVICE_SCOPE).expect("service scope");
    let service_requirement =
        ServiceRequirement::new(fabric_resource_service::ServiceProtocol::Http, SERVICE_NAME)
            .expect("service requirement");
    let prepared_service = service
        .ensure_service(&service_scope, &service_requirement)
        .expect("ensure service");
    let service_record = prepared_service.service.clone();
    if previous.is_some() {
        assert!(
            service
                .list_targets(&service_record.id)
                .expect("list targets")
                .is_empty()
        );
    }

    let service_target = ServiceTarget {
        id: ServiceTargetId::new(format!("fv0-target-{boot_index}")).expect("service target id"),
        endpoint_id: service_record.endpoint.id.clone(),
        state: ServiceTargetState::Registered,
    };
    service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: service_record.id.clone(),
                target: service_target.clone(),
            },
            ServiceHttpTargetRuntime::new(WorkerServiceTargetRuntime::shared(
                worker_http.clone(),
                worker_instance.worker_instance_id.clone(),
                service_record.endpoint.id.clone(),
            )),
        )
        .expect("register service target");
    service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: service_record.id.clone(),
            target_id: service_target.id.clone(),
        })
        .expect("mark service target ready");

    let route = ingress
        .ensure_route(&fabric_resource_ingress::IngressRouteTarget::for_service(
            &service_record,
        ))
        .expect("ensure ingress route");
    let prepared_reachability = connectivity
        .ensure_reachability(&route.id, ConnectivityScope::Local)
        .expect("ensure reachability");
    let resolved_reachability = connectivity
        .resolve_reachability(&route.id, ConnectivityScope::Local)
        .expect("resolve reachability");
    assert_eq!(
        resolved_reachability.id,
        prepared_reachability.reachability.id
    );

    if previous.is_some() {
        let stale_before_activate = local_connectivity_access
            .resolve_local_access(&resolved_reachability.id)
            .expect_err("stale local access must not survive restart");
        assert!(matches!(stale_before_activate, ConnectivityError::Inactive));
    }

    let reachability = connectivity
        .activate_reachability(&resolved_reachability.id)
        .expect("activate reachability");
    let local_access = local_connectivity_access
        .resolve_local_access(&reachability.id)
        .expect("resolve local access");

    BootState {
        instance,
        principal,
        actor,
        authority_scope_id,
        secret,
        database_ref,
        kv_ref,
        worker_instance,
        service: service_record,
        service_target,
        reachability,
        local_access,
    }
}

fn compose(
    layout: &HarnessLayout,
    artifact_fixture: &ArtifactFixture,
    deno_bin: &Path,
    captured: CapturedContracts,
) -> Composition {
    let identity_block = BlockBuilder::new(
        BlockId::new("fabric.resource.identity".to_owned()).expect("identity block"),
    )
    .register_module(NativeIdentity::new(NativeIdentityConfig {
        database_path: layout.identity_db(),
    }))
    .build();
    let authority_block = BlockBuilder::new(
        BlockId::new("fabric.resource.authority".to_owned()).expect("authority block"),
    )
    .register_module(NativeAuthority::with_decision_adapter(
        NativeAuthorityConfig {
            database_path: layout.authority_db(),
        },
        Arc::new(CedarAuthorityDecisionAdapter::new()),
    ))
    .build();
    let secrets_block = BlockBuilder::new(
        BlockId::new("fabric.resource.secrets".to_owned()).expect("secrets block"),
    )
    .register_module(NativeSecrets::new(NativeSecretsConfig {
        database_path: layout.secrets_db(),
    }))
    .build();
    let database_block =
        BlockBuilder::new(BlockId::new("fabric.resources.database".to_owned()).expect("db block"))
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
            .build();
    let kv_block =
        BlockBuilder::new(BlockId::new("fabric.resources.kv".to_owned()).expect("kv block"))
            .register_module(module_factory({
                let kv_root = layout.kv_root();
                move || {
                    NativeKv::new(Arc::new(FjallKvAdapter::new(FjallKvConfig::new(
                        kv_root.clone(),
                    ))))
                }
            }))
            .build();
    let worker_block =
        BlockBuilder::new(BlockId::new("fabric.resource.worker".to_owned()).expect("worker block"))
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
            .build();
    let projection_block =
        BlockBuilder::new(BlockId::new("fabric.projection".to_owned()).expect("projection block"))
            .register_module(NativeWorkloadProjection::new())
            .build();
    let service_block = BlockBuilder::new(
        BlockId::new("fabric.resource.service".to_owned()).expect("service block"),
    )
    .register_module(NativeServices::new(NativeServicesConfig {
        database_path: layout.service_db(),
    }))
    .build();
    let ingress_block = BlockBuilder::new(
        BlockId::new("fabric.resource.ingress".to_owned()).expect("ingress block"),
    )
    .register_module(module_factory(|| {
        NativeIngress::new(Arc::new(PingoraIngressAdapter::new(
            PingoraIngressConfig::new("127.0.0.1:0".parse().expect("loopback bind")),
        )))
    }))
    .build();
    let connectivity_block = BlockBuilder::new(
        BlockId::new("fabric.resource.connectivity".to_owned()).expect("connectivity block"),
    )
    .register_module(NativeConnectivity::new(NativeConnectivityConfig {
        database_path: layout.connectivity_db(),
    }))
    .build();
    let capture_block =
        BlockBuilder::new(BlockId::new("fabric.test.vertical".to_owned()).expect("capture block"))
            .register_module(CaptureModule::new(captured))
            .build();

    CompositionBuilder::new(
        CompositionId::new("fabric.test.vertical".to_owned()).expect("composition id"),
    )
    .register_block(identity_block)
    .register_block(authority_block)
    .register_block(secrets_block)
    .register_block(database_block)
    .register_block(kv_block)
    .register_block(worker_block)
    .register_block(projection_block)
    .register_block(service_block)
    .register_block(ingress_block)
    .register_block(connectivity_block)
    .register_block(capture_block)
    .build()
    .expect("build composition")
}

fn workload_spec(artifact: WorkloadArtifact) -> WorkerSpec {
    workload_spec_for("fv0-semantic-vertical", artifact)
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

fn authorized_action() -> ActionId {
    ActionId::new(AUTHORIZED_ACTION).expect("authorized action")
}

fn authority_resource(scope_id: &AuthorityScopeId) -> ResourceRef {
    ResourceRef::new(scope_id.clone(), RESOURCE_KIND, RESOURCE_ID).expect("authority resource")
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

fn actor_for_principal(principal_id: &PrincipalId) -> ActorRef {
    let digest = hex::encode(sha2::Sha256::digest(principal_id.as_str().as_bytes()));
    ActorRef::new(format!("fv0.principal:{digest}")).expect("actor ref")
}

fn workload_stderr_log(
    layout: &HarnessLayout,
    worker_instance_id: &fabric_resource_worker::WorkerInstanceId,
) -> std::path::PathBuf {
    layout
        .worker_runtime_root()
        .join(worker_instance_id.as_str())
        .join("stderr.log")
}

fn assert_authorized_workload_reads_secret(
    worker_http: &WorkerHttpContract,
    worker_instance: &WorkerInstance,
) {
    let response = dispatch_secret_request(worker_http, worker_instance);
    assert_eq!(response.status, 200, "authorized workload must read secret");
    let json = response_json(response);
    assert_eq!(json["secretValue"], SECRET_VALUE);
}

fn assert_unbound_workload_cannot_consume_secret(
    worker: &WorkerContract,
    worker_http: &WorkerHttpContract,
    artifact: WorkloadArtifact,
) {
    let worker_spec = workload_spec_for("fv0-secret-unbound", artifact);
    let prepared_worker = worker
        .prepare_worker(worker_spec.clone())
        .expect("prepare unbound worker");
    let worker_instance = worker
        .start_worker(StartWorkerRequest {
            prepared_worker,
            worker: worker_spec,
            bindings: Vec::new(),
            binding_projections: Vec::new(),
        })
        .expect("start unbound worker");
    let response = dispatch_secret_request(worker_http, &worker_instance);
    assert_eq!(
        response.status, 503,
        "unbound workload must not receive another workload's secret"
    );
    let json = response_json(response);
    assert_eq!(json["secretValue"], serde_json::Value::Null);
    assert_eq!(json["error"], "missing FABRIC_TEST_SECRET projection",);
    worker
        .stop_worker(StopWorkerRequest {
            worker_instance_id: worker_instance.worker_instance_id,
        })
        .expect("stop unbound worker");
}

fn assert_secret_binding_does_not_bypass_authorization(
    worker: &WorkerContract,
    artifact: WorkloadArtifact,
    secret: &SecretRef,
) {
    let denied_actor = ActorRef::new("fv0.denied:secret").expect("denied actor");
    let worker_spec = workload_spec_for("fv0-secret-denied", artifact);
    let prepared_worker = worker
        .prepare_worker(worker_spec.clone())
        .expect("prepare denied worker");
    let binding = WorkloadBinding::new(
        worker_spec.workload_id.clone(),
        BindingName::new(SECRET_BINDING).expect("secret binding name"),
        BindingTarget::secret(AuthorizedSecretRef::new(secret.clone(), denied_actor)),
    );
    let error = worker
        .start_worker(StartWorkerRequest {
            prepared_worker,
            worker: worker_spec.clone(),
            bindings: vec![binding.clone()],
            binding_projections: vec![WorkloadBindingProjection::environment(
                binding.binding_id(),
                WorkloadBindingEnv::new(SECRET_ENV).expect("secret env variable"),
            )],
        })
        .expect_err("unauthorized secret binding must fail");
    assert!(matches!(
        error,
        WorkerError::StartFailed { ref message }
            if message.contains("materialize secret binding")
                && message.contains("secret access is denied")
    ));
}

fn dispatch_secret_request(
    worker_http: &WorkerHttpContract,
    worker_instance: &WorkerInstance,
) -> HttpResponse {
    worker_http
        .dispatch_http(DispatchHttpRequest {
            worker_instance_id: worker_instance.worker_instance_id.clone(),
            request: HttpRequest {
                method: "GET".to_owned(),
                url: "/secret".to_owned(),
                headers: Vec::new(),
                body: Vec::new(),
            },
        })
        .expect("dispatch secret request")
}

fn response_json(response: HttpResponse) -> serde_json::Value {
    serde_json::from_slice(&response.body).expect("parse response body")
}

fn post_state(
    access: &LocalConnectivityAccess,
    stderr_log: &Path,
    db_value: &str,
    kv_value: &str,
) -> serde_json::Value {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();
    let response = agent
        .post(&format!("{}/state", access.url))
        .content_type("application/json")
        .send(
            serde_json::json!({
                "dbValue": db_value,
                "kvValue": kv_value,
            })
            .to_string(),
        )
        .expect("post state");
    let status = response.status();
    let body = response
        .into_body()
        .read_to_string()
        .expect("read post body");
    let stderr = std::fs::read_to_string(stderr_log).unwrap_or_default();
    assert_eq!(
        status, 200,
        "post state body: {body}\nworkload stderr:\n{stderr}"
    );
    serde_json::from_str(&body).expect("parse post body")
}

fn get_state(access: &LocalConnectivityAccess, stderr_log: &Path) -> serde_json::Value {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();
    let response = agent
        .get(&format!("{}/state", access.url))
        .call()
        .expect("get state");
    let status = response.status();
    let body = response
        .into_body()
        .read_to_string()
        .expect("read get body");
    let stderr = std::fs::read_to_string(stderr_log).unwrap_or_default();
    assert_eq!(
        status, 200,
        "get state body: {body}\nworkload stderr:\n{stderr}"
    );
    serde_json::from_str(&body).expect("parse get body")
}

fn assert_old_access_is_stale(old_url: &str) {
    let result = ureq::get(&format!("{old_url}/state")).call();
    assert!(
        result.is_err(),
        "old local connectivity access must be stale"
    );
}

fn assert_worker_runtime_root_is_cleared(layout: &HarnessLayout) {
    let runtime_root = layout.worker_runtime_root();
    if !runtime_root.exists() {
        return;
    }
    let entries = std::fs::read_dir(runtime_root)
        .expect("read worker runtime root")
        .count();
    assert_eq!(
        entries, 0,
        "stopped composition must not retain live deno runtime dirs"
    );
}

fn take_contract<T: Clone>(slot: &Arc<std::sync::Mutex<Option<T>>>) -> T {
    slot.lock()
        .expect("contract slot")
        .clone()
        .expect("captured contract")
}
