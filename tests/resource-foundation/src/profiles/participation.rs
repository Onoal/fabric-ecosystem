use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use fabric_adapter_authority_cedar::CedarAuthorityDecisionAdapter;
use fabric_adapter_database_sqlite::SqliteDatabaseAdapter;
use fabric_adapter_ingress_pingora::{PingoraIngressAdapter, PingoraIngressConfig};
use fabric_adapter_kv_fjall::{FjallKvAdapter, FjallKvConfig};
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, InstanceId, module_factory,
};
use fabric_resource::ResourceId;
use fabric_resource_authority::{NativeAuthority, NativeAuthorityConfig};
use fabric_resource_connectivity::{NativeConnectivity, NativeConnectivityConfig};
use fabric_resource_database::{DatabaseConfig, NativeDatabase};
use fabric_resource_identity::{NativeIdentity, NativeIdentityConfig};
use fabric_resource_ingress::NativeIngress;
use fabric_resource_kv::NativeKv;
use fabric_resource_registry::{
    ResourceConfiguration, ResourceConfigurationKind, ResourceRegistryModule,
};
use fabric_resource_secrets::{NativeSecrets, NativeSecretsConfig};
use fabric_resource_service::{NativeServices, NativeServicesConfig};
use fabric_resource_worker::{
    NativeWorker, WorkerAdapter, WorkerApiVersion, WorkerCapabilities, WorkerError,
    WorkerExecution, WorkerExecutionRequest, WorkerFeature, WorkerFeatureSupport, WorkerSpec,
};

use crate::helpers::layout::HarnessLayout;
use crate::helpers::shell_capture::{CapturedShell, ShellCaptureModule};

struct TestWorkerExecution;

impl WorkerExecution for TestWorkerExecution {
    fn stop(&mut self) -> Result<(), WorkerError> {
        Ok(())
    }

    fn cleanup(&mut self) {}
}

struct TestWorkerAdapter;

impl WorkerAdapter for TestWorkerAdapter {
    fn capabilities(&self) -> WorkerCapabilities {
        WorkerCapabilities {
            api_versions: vec![WorkerApiVersion::parse("1.0.0").expect("api version")],
            features: vec![
                WorkerFeatureSupport {
                    feature: WorkerFeature::new("js.module").expect("feature"),
                    supported: true,
                },
                WorkerFeatureSupport {
                    feature: WorkerFeature::new("http.fetch").expect("feature"),
                    supported: true,
                },
            ],
        }
    }

    fn supports_http_dispatch(&self) -> bool {
        true
    }

    fn prepare(&mut self, _worker: &WorkerSpec) -> Result<(), WorkerError> {
        Ok(())
    }

    fn start(
        &mut self,
        _request: WorkerExecutionRequest,
    ) -> Result<Box<dyn WorkerExecution>, WorkerError> {
        Ok(Box::new(TestWorkerExecution))
    }

    fn clear(&mut self) {}
}

#[test]
#[ignore = "requires unrestricted loopback listener for pingora ingress"]
fn full_resource_foundation_participation_lists_active_resources_only() {
    let root = tempfile::tempdir().expect("tempdir");
    let layout = HarnessLayout::new(root.path().join("foundation"));
    layout.ensure_filesystem_layout();
    let capture: CapturedShell = Arc::new(Mutex::new(None));
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.test.resource.foundation.participation".to_owned())
            .expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("resource".to_owned()).expect("block id"))
            .register_module(ResourceRegistryModule::new())
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
            .register_module(module_factory(|| {
                NativeWorker::with_adapter(Box::new(TestWorkerAdapter))
            }))
            .register_module(NativeServices::new(NativeServicesConfig {
                database_path: layout.service_db(),
            }))
            .register_module(module_factory(|| {
                NativeIngress::new(Arc::new(PingoraIngressAdapter::new(
                    PingoraIngressConfig::new(
                        "127.0.0.1:0"
                            .parse::<SocketAddr>()
                            .expect("pingora bind address"),
                    ),
                )))
            }))
            .register_module(NativeConnectivity::new(NativeConnectivityConfig {
                database_path: layout.connectivity_db(),
            }))
            .register_module(ShellCaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");

    let mut instance = composition
        .materialize(
            InstanceId::new("fabric.test.resource.foundation.participation").expect("instance id"),
        )
        .expect("materialize composition");
    instance.start().expect("start composition");
    let shell = capture
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured shell");
    let resources = shell.resources();
    let resource_ids: Vec<&str> = resources
        .iter()
        .map(|resource| resource.resource_id().as_str())
        .collect();
    let module_ids: Vec<&str> = resources
        .iter()
        .map(|resource| resource.module_id().as_str())
        .collect();
    assert_eq!(
        resource_ids,
        vec![
            "authority",
            "worker",
            "connectivity",
            "database",
            "identity",
            "ingress",
            "kv",
            "secrets",
            "service",
        ]
    );
    for forbidden in [
        "sqlite", "fjall", "deno", "process", "systemd", "pingora", "platform", "apps", "home",
    ] {
        assert!(
            !resource_ids.contains(&forbidden),
            "shell leaked implementation or upper-layer id {forbidden}"
        );
        assert!(
            module_ids
                .iter()
                .all(|module_id| !module_id.contains(forbidden)),
            "shell leaked implementation or upper-layer module id {forbidden}"
        );
    }

    let database = shell
        .resource(&ResourceId::new("database").expect("database id"))
        .expect("database resource");
    assert!(database.configuration().is_some());
    assert!(database.inspection().is_some());

    let service = shell
        .resource(&ResourceId::new("service").expect("service id"))
        .expect("service resource");
    assert!(service.configuration().is_some());
    assert!(service.inspection().is_some());

    for no_config in [
        "identity",
        "authority",
        "secrets",
        "kv",
        "worker",
        "ingress",
        "connectivity",
    ] {
        let resource = shell
            .resource(&ResourceId::new(no_config).expect("resource id"))
            .expect("resource");
        assert!(
            resource.configuration().is_none(),
            "{no_config} should not gain meaningless configuration"
        );
    }

    shell
        .consume_configuration(
            &ResourceId::new("database").expect("database id"),
            &ResourceConfiguration::new(
                ResourceConfigurationKind::new("database").expect("kind"),
                DatabaseConfig {
                    root: layout.database_root(),
                },
            ),
        )
        .expect("consume database configuration");
    shell
        .consume_configuration(
            &ResourceId::new("service").expect("service id"),
            &ResourceConfiguration::new(
                ResourceConfigurationKind::new("service.native").expect("kind"),
                NativeServicesConfig {
                    database_path: layout.service_db(),
                },
            ),
        )
        .expect("consume service configuration");

    let database_inspection = shell
        .inspect(&ResourceId::new("database").expect("database id"))
        .expect("database inspection");
    let service_inspection = shell
        .inspect(&ResourceId::new("service").expect("service id"))
        .expect("service inspection");
    assert_eq!(database_inspection.entries()[0].key(), "configured");
    assert_eq!(
        database_inspection.entries()[0].public_value(),
        Some("true")
    );
    assert_eq!(service_inspection.entries()[0].key(), "configured");
    assert_eq!(service_inspection.entries()[0].public_value(), Some("true"));
    assert!(matches!(
        shell.inspect(&ResourceId::new("secrets").expect("secrets id")),
        Err(fabric_resource_registry::ResourceRegistryError::InspectionUnsupported { .. })
    ));

    instance.stop();
}
