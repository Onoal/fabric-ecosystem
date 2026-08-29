use std::sync::{Arc, Mutex};

use fabric_adapter_database_sqlite::SqliteDatabaseAdapter;
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
    module_factory,
};
use fabric_resource::ResourceId;
use fabric_resource_database::{DatabaseConfig, NativeDatabase};
use fabric_resource_registry::{
    ResourceConfiguration, ResourceConfigurationKind, ResourceRegistry, ResourceRegistryModule,
    resource_configuration_facet_id, resource_inspection_facet_id,
};
use fabric_resource_service::{NativeServices, NativeServicesConfig};
use fabric_resource_worker::{
    NativeWorker, WorkerAdapter, WorkerApiVersion, WorkerCapabilities, WorkerError,
    WorkerExecution, WorkerExecutionRequest, WorkerFeature, WorkerFeatureSupport,
};

type CapturedResourceRegistry = Arc<Mutex<Option<Arc<ResourceRegistry>>>>;

#[derive(Clone)]
struct ResourceRegistryCaptureModule {
    module_id: ModuleId,
    requirement: ContractRequirement<ResourceRegistry>,
    captured: CapturedResourceRegistry,
}

impl ResourceRegistryCaptureModule {
    fn new(captured: CapturedResourceRegistry) -> Self {
        Self {
            module_id: ModuleId::new("fabric.fabric.vertical.resource-registry.capture")
                .expect("capture module id"),
            requirement: ContractRequirement::provisional(
                fabric_resource_registry::resource_registry_contract_id(),
            ),
            captured,
        }
    }
}

impl ModuleRuntime for ResourceRegistryCaptureModule {
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

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let resource_registry = bindings
            .resolve(&self.requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.captured.lock().expect("resource registry capture") = Some(resource_registry.clone());
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn stop(&mut self) {}

    fn health(&self) -> Health {
        Health::Healthy
    }
}

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

    fn prepare(&mut self, _worker: &fabric_resource_worker::WorkerSpec) -> Result<(), WorkerError> {
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
fn existing_resources_participate_through_the_resource_registry_box() {
    let capture = Arc::new(Mutex::new(None));
    let tempdir = tempfile::tempdir().expect("tempdir");
    let composition = CompositionBuilder::new(
        CompositionId::new("fabric.fabric.vertical.resource-registry".to_owned())
            .expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(BlockId::new("resource-registry".to_owned()).expect("block id"))
            .register_module(ResourceRegistryModule::new())
            .register_module(module_factory({
                let database_root = tempdir.path().join("database");
                move || {
                    NativeDatabase::with_sqlite_compatibility(
                        DatabaseConfig {
                            root: database_root.clone(),
                        },
                        Arc::new(SqliteDatabaseAdapter::new()),
                    )
                }
            }))
            .register_module(module_factory(|| {
                NativeWorker::with_adapter(Box::new(TestWorkerAdapter))
            }))
            .register_module(NativeServices::new(NativeServicesConfig {
                database_path: tempdir.path().join("service").join("services.sqlite"),
            }))
            .register_module(ResourceRegistryCaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");

    let mut instance = composition
        .materialize(
            InstanceId::new("fabric.fabric.vertical.resource-registry").expect("instance id"),
        )
        .expect("materialize composition");
    instance.start().expect("start");
    let resource_registry = capture
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured resource registry");
    let resources = resource_registry.resources();
    assert_eq!(resources.len(), 3);
    assert_eq!(resources[0].resource_id().as_str(), "database");
    assert_eq!(
        resources[0].module_id().as_str(),
        "fabric.resource.database.native"
    );
    assert_eq!(resources[0].provided_contracts().len(), 2);
    assert!(
        resources[0]
            .provided_contracts()
            .iter()
            .any(|contract_id| contract_id.as_str() == "fabric.resource.database")
    );
    assert!(
        resources[0]
            .provided_contracts()
            .iter()
            .any(|contract_id| contract_id.as_str()
                == "fabric.resource.database.compatibility.sqlite")
    );
    assert!(resources[0].supports_facet(&resource_configuration_facet_id()));
    assert_eq!(
        resources[0]
            .configuration()
            .expect("database config")
            .kind()
            .as_str(),
        "database"
    );
    assert!(resources[0].supports_facet(&resource_inspection_facet_id()));
    assert_eq!(resources[1].resource_id().as_str(), "service");
    assert_eq!(
        resources[1].module_id().as_str(),
        "fabric.resource.service.native"
    );
    assert_eq!(
        resources[1].provided_contracts()[0].as_str(),
        "fabric.resource.service"
    );
    assert!(resources[1].supports_facet(&resource_configuration_facet_id()));
    assert_eq!(
        resources[1]
            .configuration()
            .expect("service config")
            .kind()
            .as_str(),
        "service.native"
    );
    assert!(resources[1].supports_facet(&resource_inspection_facet_id()));
    assert_eq!(resources[2].resource_id().as_str(), "worker");
    assert_eq!(
        resources[2].module_id().as_str(),
        "fabric.resource.worker.native"
    );
    assert_eq!(
        resources[2].provided_contracts()[0].as_str(),
        "fabric.resource.worker"
    );
    assert!(resources[2].configuration().is_none());
    assert!(resources[2].inspection().is_none());

    let database_id = ResourceId::new("database").expect("resource id");
    let service_id = ResourceId::new("service").expect("resource id");
    resource_registry
        .consume_configuration(
            &database_id,
            &ResourceConfiguration::new(
                ResourceConfigurationKind::new("database").expect("configuration kind"),
                DatabaseConfig {
                    root: tempdir.path().join("database"),
                },
            ),
        )
        .expect("database configuration consumption");
    resource_registry
        .consume_configuration(
            &service_id,
            &ResourceConfiguration::new(
                ResourceConfigurationKind::new("service.native").expect("configuration kind"),
                NativeServicesConfig {
                    database_path: tempdir.path().join("service").join("services.sqlite"),
                },
            ),
        )
        .expect("service configuration consumption");

    let database_inspection = resource_registry
        .inspect(&database_id)
        .expect("database inspection");
    let service_inspection = resource_registry
        .inspect(&service_id)
        .expect("service inspection");
    assert_eq!(database_inspection.entries()[0].key(), "adapter");
    assert_eq!(
        database_inspection.entries()[0].public_value(),
        Some("sqlite")
    );
    assert_eq!(database_inspection.entries()[1].key(), "configured");
    assert_eq!(
        database_inspection.entries()[1].public_value(),
        Some("true")
    );
    assert_eq!(database_inspection.entries()[2].key(), "resource");
    assert_eq!(
        database_inspection.entries()[2].public_value(),
        Some("database")
    );
    assert_eq!(service_inspection.entries()[0].key(), "configured");
    assert_eq!(service_inspection.entries()[0].public_value(), Some("true"));
    assert_eq!(
        service_inspection.entries()[1].public_value(),
        Some("service")
    );
    assert_eq!(service_inspection.entries()[2].public_value(), Some("true"));

    let worker = resource_registry
        .resource(&ResourceId::new("worker").expect("resource id"))
        .expect("worker resource");
    assert!(worker.configuration().is_none());

    instance.stop();
}
