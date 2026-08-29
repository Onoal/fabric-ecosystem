use std::sync::{Arc, Mutex};

use fabric_adapter_authority_cedar::CedarAuthorityDecisionAdapter;
use fabric_adapter_database_sqlite::SqliteDatabaseAdapter;
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, InstanceId,
    ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime, module_factory,
};
use fabric_resource_authority::{
    ActorRef, AuthorityContract, NativeAuthority, NativeAuthorityConfig,
};
use fabric_resource_database::{DatabaseConfig, DatabaseContract, NativeDatabase};
use fabric_resource_secrets::{
    AuthorizedSecretRef, NativeSecrets, NativeSecretsConfig, SecretMaterial, SecretRef,
    SecretsContract,
};
use fabric_resource_service::{
    MarkServiceTargetReadyRequest, NativeServices, NativeServicesConfig,
    RegisterServiceTargetRequest, ServiceContract, ServiceError, ServiceHttpHeader,
    ServiceHttpRequest, ServiceHttpResponse, ServiceHttpTargetRuntime,
    ServiceHttpTargetRuntimeService, ServiceProtocol, ServiceRequirement, ServiceScope,
    ServiceTarget, ServiceTargetId, ServiceTargetState, WithdrawServiceTargetRequest,
};
use tempfile::tempdir;

use crate::{
    BindingName, BindingTarget, WorkloadBinding, WorkloadBindingEnv, WorkloadBindingProjection,
    WorkloadId,
};

use super::{
    NativeWorkloadProjection, PreparedWorkloadEnvironmentValue, WorkloadProjectionContract,
};

#[derive(Default)]
struct Capture {
    projection: Mutex<Option<WorkloadProjectionContract>>,
    authority: Mutex<Option<AuthorityContract>>,
    database: Mutex<Option<DatabaseContract>>,
    secrets: Mutex<Option<SecretsContract>>,
    service: Mutex<Option<ServiceContract>>,
}

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    projection_requirement: ContractRequirement<WorkloadProjectionContract>,
    authority_requirement: ContractRequirement<AuthorityContract>,
    database_requirement: ContractRequirement<DatabaseContract>,
    secrets_requirement: ContractRequirement<SecretsContract>,
    service_requirement: ContractRequirement<ServiceContract>,
    capture: Arc<Capture>,
}

impl CaptureModule {
    fn new(capture: Arc<Capture>) -> Self {
        Self {
            module_id: ModuleId::new("fabric.resource.worker.projection.capture")
                .expect("module id"),
            projection_requirement: ContractRequirement::provisional(
                crate::workload_projection_contract_id(),
            ),
            authority_requirement: ContractRequirement::provisional(
                fabric_resource_authority::authority_contract_id(),
            ),
            database_requirement: ContractRequirement::provisional(
                fabric_resource_database::database_contract_id(),
            ),
            secrets_requirement: ContractRequirement::provisional(
                fabric_resource_secrets::secrets_contract_id(),
            ),
            service_requirement: ContractRequirement::provisional(
                fabric_resource_service::service_contract_id(),
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
        vec![self.projection_requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![
            self.authority_requirement.id().clone(),
            self.database_requirement.id().clone(),
            self.secrets_requirement.id().clone(),
            self.service_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let projection = bindings
            .resolve(&self.projection_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let authority = bindings
            .resolve_optional(&self.authority_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let database = bindings
            .resolve_optional(&self.database_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let secrets = bindings
            .resolve_optional(&self.secrets_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let service = bindings
            .resolve_optional(&self.service_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.projection.lock().expect("projection lock") = Some((*projection).clone());
        *self.capture.authority.lock().expect("authority lock") = authority.as_deref().cloned();
        *self.capture.database.lock().expect("database lock") = database.as_deref().cloned();
        *self.capture.secrets.lock().expect("secrets lock") = secrets.as_deref().cloned();
        *self.capture.service.lock().expect("service lock") = service.as_deref().cloned();
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

fn start_test_instance(
    composition_id: &str,
    composition: fabric_core::Composition,
) -> fabric_core::Instance {
    let mut instance = composition
        .materialize(InstanceId::new(composition_id).expect("instance id"))
        .expect("materialize composition");
    instance.start().expect("start");
    instance
}

#[test]
fn binding_without_projection_is_valid_and_materializes_nothing() {
    let capture = Arc::new(Capture::default());
    let tempdir = tempdir().expect("tempdir");
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.projection.zero".to_owned())
            .expect("composition"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("projection".to_owned()).expect("block"))
            .register_module(NativeWorkloadProjection::new())
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("database".to_owned()).expect("block"))
            .register_module(module_factory({
                let database_root = tempdir.path().join("db");
                move || {
                    NativeDatabase::with_sqlite_compatibility(
                        DatabaseConfig {
                            root: database_root.clone(),
                        },
                        Arc::new(SqliteDatabaseAdapter::new()),
                    )
                }
            }))
            .build(),
    )
    .build()
    .expect("composition");
    let _instance = start_test_instance("fabric.resource.worker.projection.zero", composition);

    let projection = take_projection(&capture);
    let prepared_database = take_database(&capture)
        .prepare(
            fabric_resource::ResourceContext::root(
                fabric_resource::ResourceBoundaryId::new("notes").expect("boundary"),
            ),
            fabric_resource::ResourceName::new("primary").expect("name"),
        )
        .expect("prepare database");
    let binding = WorkloadBinding::new(
        WorkloadId::new("apps.notes").expect("workload"),
        BindingName::new("DB").expect("binding name"),
        BindingTarget::database(prepared_database.database_ref()),
    );

    let prepared = projection
        .prepare(&[binding], &[])
        .expect("prepare projections");
    assert!(prepared.is_empty());
}

#[test]
fn projected_database_binding_produces_structured_and_public_environment_outputs() {
    let capture = Arc::new(Capture::default());
    let tempdir = tempdir().expect("tempdir");
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.projection.database".to_owned())
            .expect("composition"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("projection".to_owned()).expect("block"))
            .register_module(NativeWorkloadProjection::new())
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("database".to_owned()).expect("block"))
            .register_module(module_factory({
                let database_root = tempdir.path().join("db");
                move || {
                    NativeDatabase::with_sqlite_compatibility(
                        DatabaseConfig {
                            root: database_root.clone(),
                        },
                        Arc::new(SqliteDatabaseAdapter::new()),
                    )
                }
            }))
            .build(),
    )
    .build()
    .expect("composition");
    let _instance = start_test_instance("fabric.resource.worker.projection.database", composition);

    let projection = take_projection(&capture);
    let reference = take_database(&capture)
        .prepare(
            fabric_resource::ResourceContext::root(
                fabric_resource::ResourceBoundaryId::new("notes").expect("boundary"),
            ),
            fabric_resource::ResourceName::new("primary").expect("name"),
        )
        .expect("prepare database")
        .database_ref();
    let binding = WorkloadBinding::new(
        WorkloadId::new("apps.notes").expect("workload"),
        BindingName::new("db").expect("binding name"),
        BindingTarget::database(reference),
    );

    let prepared = projection
        .prepare(
            std::slice::from_ref(&binding),
            &[
                WorkloadBindingProjection::structured(binding.binding_id()),
                WorkloadBindingProjection::environment(
                    binding.binding_id(),
                    WorkloadBindingEnv::new("DB_BASE_URL").expect("env"),
                ),
            ],
        )
        .expect("prepare projections");

    assert_eq!(prepared.structured()["db"]["kind"], "database");
    assert!(prepared.structured()["db"]["baseUrl"].is_string());
    assert!(matches!(
        prepared.environment()["DB_BASE_URL"],
        PreparedWorkloadEnvironmentValue::Public(_)
    ));
    assert!(
        prepared.environment()["DB_BASE_URL"]
            .expose()
            .starts_with("http://127.0.0.1:")
    );
    assert_eq!(prepared.network_authorities().len(), 1);
}

#[test]
fn no_secret_projection_means_no_secret_materialization() {
    let capture = Arc::new(Capture::default());
    let tempdir = tempdir().expect("tempdir");
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.projection.secret.none".to_owned())
            .expect("composition"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("projection".to_owned()).expect("block"))
            .register_module(NativeWorkloadProjection::new())
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("authority".to_owned()).expect("block"))
            .register_module(NativeAuthority::with_decision_adapter(
                NativeAuthorityConfig {
                    database_path: tempdir.path().join("authority.sqlite"),
                },
                Arc::new(CedarAuthorityDecisionAdapter::new()),
            ))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("secrets".to_owned()).expect("block"))
            .register_module(NativeSecrets::new(NativeSecretsConfig {
                database_path: tempdir.path().join("secrets.sqlite"),
            }))
            .build(),
    )
    .build()
    .expect("composition");
    let _instance =
        start_test_instance("fabric.resource.worker.projection.secret.none", composition);

    let actor = ActorRef::new("apps.notes:runtime").expect("actor");
    let secret = create_secret(&capture, &actor);
    let binding = WorkloadBinding::new(
        WorkloadId::new("apps.notes").expect("workload"),
        BindingName::new("SECRET").expect("binding name"),
        BindingTarget::secret(AuthorizedSecretRef::new(secret, actor)),
    );

    let prepared = take_projection(&capture)
        .prepare(std::slice::from_ref(&binding), &[])
        .expect("prepare projections");
    assert!(prepared.is_empty());
}

#[test]
fn secret_environment_projection_is_sensitive_and_unauthorized_projection_fails() {
    let capture = Arc::new(Capture::default());
    let tempdir = tempdir().expect("tempdir");
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.projection.secret.env".to_owned())
            .expect("composition"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("projection".to_owned()).expect("block"))
            .register_module(NativeWorkloadProjection::new())
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("authority".to_owned()).expect("block"))
            .register_module(NativeAuthority::with_decision_adapter(
                NativeAuthorityConfig {
                    database_path: tempdir.path().join("authority.sqlite"),
                },
                Arc::new(CedarAuthorityDecisionAdapter::new()),
            ))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("secrets".to_owned()).expect("block"))
            .register_module(NativeSecrets::new(NativeSecretsConfig {
                database_path: tempdir.path().join("secrets.sqlite"),
            }))
            .build(),
    )
    .build()
    .expect("composition");
    let _instance =
        start_test_instance("fabric.resource.worker.projection.secret.env", composition);

    let actor = ActorRef::new("apps.notes:runtime").expect("actor");
    let secret = create_secret(&capture, &actor);
    let binding = WorkloadBinding::new(
        WorkloadId::new("apps.notes").expect("workload"),
        BindingName::new("SECRET").expect("binding name"),
        BindingTarget::secret(AuthorizedSecretRef::new(secret.clone(), actor.clone())),
    );
    let prepared = take_projection(&capture)
        .prepare(
            std::slice::from_ref(&binding),
            &[WorkloadBindingProjection::environment(
                binding.binding_id(),
                WorkloadBindingEnv::new("FABRIC_TEST_SECRET").expect("env"),
            )],
        )
        .expect("prepare secret env projection");
    assert!(prepared.environment()["FABRIC_TEST_SECRET"].is_sensitive());
    let debug = format!("{:?}", prepared);
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("fv0-secret-material"));

    let denied_binding = WorkloadBinding::new(
        WorkloadId::new("apps.notes").expect("workload"),
        BindingName::new("SECRET_DENIED").expect("binding name"),
        BindingTarget::secret(AuthorizedSecretRef::new(
            secret,
            ActorRef::new("apps.notes:denied").expect("denied actor"),
        )),
    );
    let error = take_projection(&capture)
        .prepare(
            std::slice::from_ref(&denied_binding),
            &[WorkloadBindingProjection::environment(
                denied_binding.binding_id(),
                WorkloadBindingEnv::new("FABRIC_TEST_SECRET").expect("env"),
            )],
        )
        .expect_err("unauthorized secret projection must fail");
    assert!(error.to_string().contains("secret access is denied"));
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordedServiceDispatch {
    endpoint_id: String,
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Default)]
struct ServiceProbe {
    dispatches: Mutex<Vec<RecordedServiceDispatch>>,
}

impl ServiceProbe {
    fn dispatches(&self) -> std::sync::MutexGuard<'_, Vec<RecordedServiceDispatch>> {
        self.dispatches.lock().expect("service dispatches")
    }
}

struct FixedResponseRuntime {
    probe: Arc<ServiceProbe>,
    status: u16,
    headers: Vec<(String, String)>,
    response_body: String,
}

impl ServiceHttpTargetRuntimeService for FixedResponseRuntime {
    fn dispatch_http(
        &self,
        endpoint_id: &fabric_resource_service::ServiceEndpointId,
        request: ServiceHttpRequest,
    ) -> Result<ServiceHttpResponse, ServiceError> {
        self.probe.dispatches().push(RecordedServiceDispatch {
            endpoint_id: endpoint_id.as_str().to_owned(),
            method: request.method,
            url: request.url,
            headers: request
                .headers
                .iter()
                .map(|header| (header.name.clone(), header.value.clone()))
                .collect(),
            body: request.body,
        });
        Ok(ServiceHttpResponse {
            status: self.status,
            headers: self
                .headers
                .iter()
                .map(|(name, value)| ServiceHttpHeader {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect(),
            body: self.response_body.clone().into_bytes(),
        })
    }
}

#[test]
fn service_binding_projection_dispatches_to_current_ready_target_without_reprojection() {
    let capture = Arc::new(Capture::default());
    let tempdir = tempdir().expect("tempdir");
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.projection.service".to_owned())
            .expect("composition"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("projection".to_owned()).expect("block"))
            .register_module(NativeWorkloadProjection::new())
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("service".to_owned()).expect("block"))
            .register_module(NativeServices::new(NativeServicesConfig {
                database_path: tempdir.path().join("services.sqlite"),
            }))
            .build(),
    )
    .build()
    .expect("composition");
    let _instance = start_test_instance("fabric.resource.worker.projection.service", composition);

    let service = take_service(&capture);
    let prepared_service = service
        .ensure_service(
            &ServiceScope::new("apps.notes.api").expect("scope"),
            &ServiceRequirement::new(ServiceProtocol::Http, "api").expect("requirement"),
        )
        .expect("ensure service")
        .service;
    let binding = WorkloadBinding::new(
        WorkloadId::new("apps.notes").expect("workload"),
        BindingName::new("api").expect("binding name"),
        BindingTarget::service(prepared_service.id.clone()),
    );
    let prepared = take_projection(&capture)
        .prepare(
            std::slice::from_ref(&binding),
            &[
                WorkloadBindingProjection::structured(binding.binding_id()),
                WorkloadBindingProjection::environment(
                    binding.binding_id(),
                    WorkloadBindingEnv::new("API_BASE_URL").expect("env"),
                ),
            ],
        )
        .expect("prepare service projection");
    let base_url = prepared.environment()["API_BASE_URL"].expose().to_owned();

    assert_eq!(prepared.structured()["api"]["kind"], "service");
    assert_eq!(
        prepared.structured()["api"]["serviceId"],
        prepared_service.id.as_str()
    );
    assert_eq!(prepared.network_authorities().len(), 1);

    match ureq::get(&format!("{base_url}/status")).call() {
        Err(ureq::Error::StatusCode(503)) => {}
        other => panic!("expected 503 without ready target, got {other:?}"),
    }

    let first_probe = Arc::new(ServiceProbe::default());
    let first_target = ServiceTarget {
        id: ServiceTargetId::new("notes_api_a").expect("target id"),
        endpoint_id: prepared_service.endpoint.id.clone(),
        state: ServiceTargetState::Registered,
    };
    service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: prepared_service.id.clone(),
                target: first_target.clone(),
            },
            ServiceHttpTargetRuntime::new(Arc::new(FixedResponseRuntime {
                probe: Arc::clone(&first_probe),
                status: 200,
                headers: Vec::new(),
                response_body: "service-a".to_owned(),
            })),
        )
        .expect("register first target");
    service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: prepared_service.id.clone(),
            target_id: first_target.id.clone(),
        })
        .expect("mark first ready");

    let first_response = ureq::get(&format!("{base_url}/status?check=1"))
        .call()
        .expect("first request");
    assert_eq!(
        first_response.into_body().read_to_string().expect("body"),
        "service-a"
    );
    let first_dispatch = &first_probe.dispatches()[0];
    assert_eq!(
        first_dispatch.endpoint_id,
        prepared_service.endpoint.id.as_str()
    );
    assert_eq!(first_dispatch.method, "GET");
    assert_eq!(first_dispatch.url, "/status?check=1");
    assert!(
        first_dispatch
            .headers
            .contains(&("accept".to_owned(), "*/*".to_owned()))
    );
    assert_eq!(first_dispatch.body, Vec::<u8>::new());

    service
        .withdraw_target(WithdrawServiceTargetRequest {
            service_id: prepared_service.id.clone(),
            target_id: first_target.id,
        })
        .expect("withdraw first target");

    let second_probe = Arc::new(ServiceProbe::default());
    let second_target = ServiceTarget {
        id: ServiceTargetId::new("notes_api_b").expect("target id"),
        endpoint_id: prepared_service.endpoint.id.clone(),
        state: ServiceTargetState::Registered,
    };
    service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: prepared_service.id.clone(),
                target: second_target.clone(),
            },
            ServiceHttpTargetRuntime::new(Arc::new(FixedResponseRuntime {
                probe: Arc::clone(&second_probe),
                status: 200,
                headers: Vec::new(),
                response_body: "service-b".to_owned(),
            })),
        )
        .expect("register second target");
    service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: prepared_service.id.clone(),
            target_id: second_target.id.clone(),
        })
        .expect("mark second ready");

    let second_response = ureq::get(&format!("{base_url}/status?check=2"))
        .call()
        .expect("second request");
    assert_eq!(
        second_response.into_body().read_to_string().expect("body"),
        "service-b"
    );
    let second_dispatch = &second_probe.dispatches()[0];
    assert_eq!(
        second_dispatch.endpoint_id,
        prepared_service.endpoint.id.as_str()
    );
    assert_eq!(second_dispatch.method, "GET");
    assert_eq!(second_dispatch.url, "/status?check=2");
    assert!(
        second_dispatch
            .headers
            .contains(&("accept".to_owned(), "*/*".to_owned()))
    );
    assert_eq!(second_dispatch.body, Vec::<u8>::new());

    service
        .withdraw_target(WithdrawServiceTargetRequest {
            service_id: prepared_service.id.clone(),
            target_id: second_target.id,
        })
        .expect("withdraw second target");

    match ureq::get(&format!("{base_url}/status?check=3")).call() {
        Err(ureq::Error::StatusCode(503)) => {}
        other => panic!("expected 503 after withdrawing last target, got {other:?}"),
    }
}

#[test]
fn service_binding_projection_preserves_post_request_and_response_semantics() {
    let capture = Arc::new(Capture::default());
    let tempdir = tempdir().expect("tempdir");
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.resource.worker.projection.service.post".to_owned())
            .expect("composition"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("projection".to_owned()).expect("block"))
            .register_module(NativeWorkloadProjection::new())
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("service".to_owned()).expect("block"))
            .register_module(NativeServices::new(NativeServicesConfig {
                database_path: tempdir.path().join("services.sqlite"),
            }))
            .build(),
    )
    .build()
    .expect("composition");
    let _instance = start_test_instance(
        "fabric.resource.worker.projection.service.post",
        composition,
    );

    let service = take_service(&capture);
    let prepared_service = service
        .ensure_service(
            &ServiceScope::new("apps.notes.api").expect("scope"),
            &ServiceRequirement::new(ServiceProtocol::Http, "api").expect("requirement"),
        )
        .expect("ensure service")
        .service;
    let binding = WorkloadBinding::new(
        WorkloadId::new("apps.notes").expect("workload"),
        BindingName::new("api").expect("binding name"),
        BindingTarget::service(prepared_service.id.clone()),
    );
    let prepared = take_projection(&capture)
        .prepare(
            std::slice::from_ref(&binding),
            &[WorkloadBindingProjection::environment(
                binding.binding_id(),
                WorkloadBindingEnv::new("API_BASE_URL").expect("env"),
            )],
        )
        .expect("prepare service projection");
    let base_url = prepared.environment()["API_BASE_URL"].expose().to_owned();

    let probe = Arc::new(ServiceProbe::default());
    let target = ServiceTarget {
        id: ServiceTargetId::new("notes_api_post").expect("target id"),
        endpoint_id: prepared_service.endpoint.id.clone(),
        state: ServiceTargetState::Registered,
    };
    service
        .register_target(
            RegisterServiceTargetRequest {
                service_id: prepared_service.id.clone(),
                target: target.clone(),
            },
            ServiceHttpTargetRuntime::new(Arc::new(FixedResponseRuntime {
                probe: Arc::clone(&probe),
                status: 201,
                headers: vec![("X-Service-Proof".to_owned(), "yes".to_owned())],
                response_body: "created".to_owned(),
            })),
        )
        .expect("register target");
    service
        .mark_target_ready(MarkServiceTargetReadyRequest {
            service_id: prepared_service.id.clone(),
            target_id: target.id.clone(),
        })
        .expect("mark target ready");

    let response = ureq::post(&format!("{base_url}/items?draft=true"))
        .header("X-Test", "hello")
        .send("payload")
        .expect("post request");
    assert_eq!(response.status(), 201);
    assert_eq!(
        response
            .headers()
            .get("x-service-proof")
            .expect("service proof header")
            .to_str()
            .expect("header value"),
        "yes"
    );
    assert_eq!(
        response.into_body().read_to_string().expect("body"),
        "created"
    );
    let dispatch = &probe.dispatches()[0];
    assert_eq!(dispatch.endpoint_id, prepared_service.endpoint.id.as_str());
    assert_eq!(dispatch.method, "POST");
    assert_eq!(dispatch.url, "/items?draft=true");
    assert_eq!(dispatch.body, b"payload".to_vec());
    assert!(
        dispatch
            .headers
            .contains(&("x-test".to_owned(), "hello".to_owned()))
    );
}

fn take_projection(capture: &Capture) -> WorkloadProjectionContract {
    capture
        .projection
        .lock()
        .expect("projection lock")
        .clone()
        .expect("projection contract")
}

fn take_authority(capture: &Capture) -> AuthorityContract {
    capture
        .authority
        .lock()
        .expect("authority lock")
        .clone()
        .expect("authority contract")
}

fn take_database(capture: &Capture) -> DatabaseContract {
    capture
        .database
        .lock()
        .expect("database lock")
        .clone()
        .expect("database contract")
}

fn take_secrets(capture: &Capture) -> SecretsContract {
    capture
        .secrets
        .lock()
        .expect("secrets lock")
        .clone()
        .expect("secrets contract")
}

fn take_service(capture: &Capture) -> ServiceContract {
    capture
        .service
        .lock()
        .expect("service lock")
        .clone()
        .expect("service contract")
}

fn create_secret(capture: &Capture, actor: &ActorRef) -> SecretRef {
    let authority = take_authority(capture);
    let scope_id = authority.create_scope().expect("create scope");
    authority
        .grant_scope_control(actor, &scope_id)
        .expect("grant scope control");
    take_secrets(capture)
        .create(actor, &scope_id, SecretMaterial::new("fv0-secret-material"))
        .expect("create secret")
        .secret
}
