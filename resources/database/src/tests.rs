use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use fabric_binding::BindingName;
use fabric_core::{
    BlockBuilder, CompositionBuilder, CompositionId, ContractRequirement, Health, ModuleBindings,
    ModuleContract, ModuleError, ModuleId, ModuleRuntime, module_factory,
};
use fabric_resource::{
    ResourceBoundaryId, ResourceContext, ResourceError, ResourceId, ResourceInstanceId,
    ResourceName,
};
use fabric_resource_registry::{
    ResourceConfiguration, ResourceConfigurationKind, ResourceInspection, ResourceInspectionEntry,
    ResourceRegistry, ResourceRegistryModule,
};

use crate::native::DatabaseAdapter;
use crate::{
    DatabaseConfig, DatabaseContract, DatabaseRef, DatabaseService, NativeDatabase,
    PreparedDatabase, PreparedDatabaseState, SqliteDatabaseCompatibility,
    SqliteDatabaseCompatibilityContract, SqliteDatabaseMaterialization, database_resource_id,
};

struct AssertingDatabaseService;

struct CountingDatabaseService {
    prepare_calls: AtomicUsize,
    verify_calls: AtomicUsize,
}

impl DatabaseService for AssertingDatabaseService {
    fn prepare(
        &self,
        resource_id: &ResourceInstanceId,
    ) -> Result<PreparedDatabaseState, ResourceError> {
        assert_eq!(resource_id.resource(), database_resource_id());
        Ok(PreparedDatabaseState { created: false })
    }

    fn cleanup(&self, _prepared: &PreparedDatabase) {}

    fn verify(&self, resource_id: &ResourceInstanceId) -> Result<(), ResourceError> {
        assert_eq!(resource_id.resource(), database_resource_id());
        Ok(())
    }
}

impl CountingDatabaseService {
    fn new() -> Self {
        Self {
            prepare_calls: AtomicUsize::new(0),
            verify_calls: AtomicUsize::new(0),
        }
    }
}

impl DatabaseService for CountingDatabaseService {
    fn prepare(
        &self,
        resource_id: &ResourceInstanceId,
    ) -> Result<PreparedDatabaseState, ResourceError> {
        assert_eq!(resource_id.resource(), database_resource_id());
        self.prepare_calls.fetch_add(1, Ordering::SeqCst);
        Ok(PreparedDatabaseState { created: false })
    }

    fn cleanup(&self, _prepared: &PreparedDatabase) {}

    fn verify(&self, resource_id: &ResourceInstanceId) -> Result<(), ResourceError> {
        assert_eq!(resource_id.resource(), database_resource_id());
        self.verify_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[derive(Default)]
struct SyntheticDatabaseAdapter {
    consumed: Mutex<Option<PathBuf>>,
}

impl SyntheticDatabaseAdapter {
    fn new() -> Self {
        Self::default()
    }

    fn path_for(&self, resource_id: &ResourceInstanceId) -> Result<PathBuf, ResourceError> {
        let root = self
            .consumed
            .lock()
            .expect("synthetic root")
            .clone()
            .ok_or_else(|| ResourceError::InvalidInput {
                message: "database configuration has not been consumed".to_owned(),
            })?;
        Ok(root.join(resource_id.as_str()).join("synthetic.db"))
    }
}

impl DatabaseService for SyntheticDatabaseAdapter {
    fn prepare(
        &self,
        resource_id: &ResourceInstanceId,
    ) -> Result<PreparedDatabaseState, ResourceError> {
        let path = self.path_for(resource_id)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| ResourceError::PrepareFailed {
                message: error.to_string(),
            })?;
        }
        std::fs::write(&path, b"synthetic").map_err(|error| ResourceError::PrepareFailed {
            message: error.to_string(),
        })?;
        Ok(PreparedDatabaseState { created: true })
    }

    fn cleanup(&self, prepared: &PreparedDatabase) {
        if let Ok(path) = self.path_for(&prepared.resource_id) {
            let _ = std::fs::remove_file(path);
        }
    }

    fn verify(&self, resource_id: &ResourceInstanceId) -> Result<(), ResourceError> {
        let path = self.path_for(resource_id)?;
        if path.is_file() {
            Ok(())
        } else {
            Err(ResourceError::Integrity {
                message: "synthetic database backing is missing".to_owned(),
            })
        }
    }
}

impl DatabaseAdapter for SyntheticDatabaseAdapter {
    fn initialize(&self, config: &DatabaseConfig) -> Result<(), ModuleError> {
        std::fs::create_dir_all(&config.root).map_err(|error| ModuleError::new(error.to_string()))
    }

    fn consume_configuration(
        &self,
        config: &DatabaseConfig,
    ) -> Result<(), fabric_resource_registry::ResourceRegistryError> {
        let mut consumed = self.consumed.lock().expect("synthetic root");
        if let Some(existing) = consumed.as_ref() {
            if existing != &config.root {
                return Err(
                    fabric_resource_registry::ResourceRegistryError::ConfigurationRejected {
                        message:
                            "database configuration change is not supported after initialization"
                                .to_owned(),
                    },
                );
            }
            return Ok(());
        }
        *consumed = Some(config.root.clone());
        Ok(())
    }

    fn inspect(
        &self,
    ) -> Result<
        fabric_resource_registry::ResourceInspection,
        fabric_resource_registry::ResourceRegistryError,
    > {
        ResourceInspection::new(vec![
            ResourceInspectionEntry::public(
                "configured",
                self.consumed
                    .lock()
                    .expect("synthetic root")
                    .is_some()
                    .to_string(),
            )?,
            ResourceInspectionEntry::public("adapter", "synthetic")?,
            ResourceInspectionEntry::public("resource", "database")?,
        ])
    }

    fn health(&self) -> Health {
        Health::Healthy
    }
}

#[derive(Default)]
struct SyntheticSqliteDatabaseAdapter {
    consumed: Mutex<Option<PathBuf>>,
}

impl SyntheticSqliteDatabaseAdapter {
    fn new() -> Self {
        Self::default()
    }

    fn path_for(&self, resource_id: &ResourceInstanceId) -> Result<PathBuf, ResourceError> {
        let root = self
            .consumed
            .lock()
            .expect("synthetic sqlite root")
            .clone()
            .ok_or_else(|| ResourceError::InvalidInput {
                message: "database configuration has not been consumed".to_owned(),
            })?;
        Ok(root.join(resource_id.as_str()).join("database.sqlite"))
    }
}

impl DatabaseService for SyntheticSqliteDatabaseAdapter {
    fn prepare(
        &self,
        resource_id: &ResourceInstanceId,
    ) -> Result<PreparedDatabaseState, ResourceError> {
        let path = self.path_for(resource_id)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| ResourceError::PrepareFailed {
                message: error.to_string(),
            })?;
        }
        std::fs::write(&path, b"synthetic-sqlite").map_err(|error| {
            ResourceError::PrepareFailed {
                message: error.to_string(),
            }
        })?;
        Ok(PreparedDatabaseState { created: true })
    }

    fn cleanup(&self, prepared: &PreparedDatabase) {
        if let Ok(path) = self.path_for(&prepared.resource_id) {
            let _ = std::fs::remove_file(path);
        }
    }

    fn verify(&self, resource_id: &ResourceInstanceId) -> Result<(), ResourceError> {
        let path = self.path_for(resource_id)?;
        if path.is_file() {
            Ok(())
        } else {
            Err(ResourceError::Integrity {
                message: "synthetic sqlite database backing is missing".to_owned(),
            })
        }
    }
}

impl DatabaseAdapter for SyntheticSqliteDatabaseAdapter {
    fn initialize(&self, config: &DatabaseConfig) -> Result<(), ModuleError> {
        std::fs::create_dir_all(&config.root).map_err(|error| ModuleError::new(error.to_string()))
    }

    fn consume_configuration(
        &self,
        config: &DatabaseConfig,
    ) -> Result<(), fabric_resource_registry::ResourceRegistryError> {
        let mut consumed = self.consumed.lock().expect("synthetic sqlite root");
        if let Some(existing) = consumed.as_ref() {
            if existing != &config.root {
                return Err(
                    fabric_resource_registry::ResourceRegistryError::ConfigurationRejected {
                        message:
                            "database configuration change is not supported after initialization"
                                .to_owned(),
                    },
                );
            }
            return Ok(());
        }
        *consumed = Some(config.root.clone());
        Ok(())
    }

    fn inspect(
        &self,
    ) -> Result<ResourceInspection, fabric_resource_registry::ResourceRegistryError> {
        ResourceInspection::new(vec![
            ResourceInspectionEntry::public(
                "configured",
                self.consumed
                    .lock()
                    .expect("synthetic sqlite root")
                    .is_some()
                    .to_string(),
            )?,
            ResourceInspectionEntry::public("adapter", "sqlite")?,
            ResourceInspectionEntry::public("resource", "database")?,
        ])
    }

    fn health(&self) -> Health {
        Health::Healthy
    }
}

impl SqliteDatabaseCompatibility for SyntheticSqliteDatabaseAdapter {
    fn materialize_sqlite(
        &self,
        reference: &DatabaseRef,
    ) -> Result<SqliteDatabaseMaterialization, ResourceError> {
        let path = self.path_for(reference.resource_id())?;
        self.verify(reference.resource_id())?;
        Ok(SqliteDatabaseMaterialization::new(path))
    }
}

#[derive(Clone)]
struct CaptureModule {
    module_id: ModuleId,
    shell_requirement: ContractRequirement<ResourceRegistry>,
    database_requirement: ContractRequirement<DatabaseContract>,
    sqlite_compatibility_requirement: ContractRequirement<SqliteDatabaseCompatibilityContract>,
    captured: Arc<Mutex<Option<CapturedContracts>>>,
}

#[derive(Clone)]
struct CapturedContracts {
    shell: Arc<ResourceRegistry>,
    database: Arc<DatabaseContract>,
    sqlite_compatibility: Option<SqliteDatabaseCompatibilityContract>,
}

impl CaptureModule {
    fn new(captured: Arc<Mutex<Option<CapturedContracts>>>) -> Self {
        Self {
            module_id: ModuleId::new("test.database.capture").expect("module id"),
            shell_requirement: ContractRequirement::provisional(
                fabric_resource_registry::resource_registry_contract_id(),
            ),
            database_requirement: ContractRequirement::provisional(crate::database_contract_id()),
            sqlite_compatibility_requirement: ContractRequirement::provisional(
                crate::sqlite_database_compatibility_contract_id(),
            ),
            captured,
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
        vec![
            self.shell_requirement.id().clone(),
            self.database_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.sqlite_compatibility_requirement.id().clone()]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let shell = bindings
            .resolve(&self.shell_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let database = bindings
            .resolve(&self.database_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let sqlite_compatibility = bindings
            .resolve_optional(&self.sqlite_compatibility_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.captured.lock().expect("capture lock") = Some(CapturedContracts {
            shell,
            database,
            sqlite_compatibility: sqlite_compatibility.as_deref().cloned(),
        });
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

#[test]
fn database_ref_identity_encoding_remains_stable() {
    let boundary = ResourceBoundaryId::new("apps.notes").expect("boundary");
    let context = ResourceContext::root(boundary);
    let name = ResourceName::new("primary").expect("name");

    let reference = DatabaseRef::parse(format!(
        "fabric-resource-database-ref-v1:{}",
        ResourceInstanceId::canonical(&database_resource_id(), &context, &name).as_str()
    ))
    .expect("database ref");

    assert_eq!(
        reference.encode(),
        format!(
            "fabric-resource-database-ref-v1:{}",
            reference.resource_id().as_str()
        )
    );
    assert!(
        reference
            .resource_id()
            .as_str()
            .starts_with("fabric-resource-database-")
    );
}

#[test]
fn database_ref_validation_uses_the_same_canonical_resource_identity_as_provisioning_and_shell() {
    let boundary = ResourceBoundaryId::new("apps.notes").expect("boundary");
    let context = ResourceContext::root(boundary);
    let name = ResourceName::new("primary").expect("name");
    let contract = DatabaseContract::new(Arc::new(AssertingDatabaseService));

    let prepared = contract
        .prepare(context.clone(), name.clone())
        .expect("prepare database");
    let reference = prepared.database_ref();
    let parsed = DatabaseRef::parse(reference.encode()).expect("parse database ref");
    let descriptor = fabric_resource_registry::ResourceDescriptor::new(database_resource_id());

    assert_eq!(reference.resource_id().resource(), database_resource_id());
    assert_eq!(parsed.resource_id(), reference.resource_id());
    assert_eq!(parsed.resource_id().resource(), database_resource_id());
    assert_eq!(descriptor.resource_id(), &database_resource_id());
}

#[test]
fn binding_existing_database_ref_is_explicit_and_does_not_reprepare() {
    let boundary = ResourceBoundaryId::new("apps.notes").expect("boundary");
    let owner = ResourceContext::root(boundary.clone());
    let consumer = ResourceContext::root(boundary)
        .child(fabric_resource::ResourceScope::new("shared").expect("consumer scope"));
    let name = ResourceName::new("primary").expect("name");
    let service = Arc::new(CountingDatabaseService::new());
    let contract = DatabaseContract::new(service.clone());

    let prepared = contract
        .prepare(owner.clone(), name.clone())
        .expect("prepare database");
    let reference = prepared.database_ref();
    let binding = contract
        .bind(
            &consumer,
            BindingName::new("shared-db").expect("binding name"),
            &reference,
        )
        .expect("bind existing database ref");

    assert_eq!(service.prepare_calls.load(Ordering::SeqCst), 1);
    assert_eq!(service.verify_calls.load(Ordering::SeqCst), 1);
    assert_eq!(binding.database_ref(), reference);
    assert_eq!(binding.name().as_str(), "shared-db");
    assert_eq!(binding.consumer().kind().as_str(), "resource-context");
}

#[test]
fn pre_e3_database_resource_import_binding_identity_is_preserved_exactly() {
    let context = ResourceContext::root(ResourceBoundaryId::new("apps.notes").expect("boundary"))
        .child(fabric_resource::ResourceScope::new("release").expect("scope"));
    let name = ResourceName::new("primary").expect("name");
    let contract = DatabaseContract::new(Arc::new(AssertingDatabaseService));
    let reference = contract.reference(&context, &name);
    let binding = contract
        .bind(
            &context,
            BindingName::new("shared").expect("binding name"),
            &reference,
        )
        .expect("bind database reference");

    assert_eq!(
        reference.resource_id().as_str(),
        "fabric-resource-database-1ff9996b043d0dab32cb1db44499a3fa1c80b7acfeea8c87eeacb420fb52275a"
    );
    assert_eq!(
        binding.binding_id().as_str(),
        "fabric-binding-3169f5f6eb4c282f451b4750f5e581bc412100ce76fcc2e6b17a7100fc184413"
    );
}

#[test]
fn one_database_resource_owns_multiple_stable_instances() {
    let boundary = ResourceBoundaryId::new("apps.notes").expect("boundary");
    let context = ResourceContext::root(boundary);
    let contract = DatabaseContract::new(Arc::new(AssertingDatabaseService));
    let primary = contract.reference(&context, &ResourceName::new("primary").expect("name"));
    let analytics = contract.reference(&context, &ResourceName::new("analytics").expect("name"));
    let cache = contract.reference(&context, &ResourceName::new("cache").expect("name"));

    assert_eq!(primary.resource_id().resource(), database_resource_id());
    assert_eq!(analytics.resource_id().resource(), database_resource_id());
    assert_eq!(cache.resource_id().resource(), database_resource_id());
    assert_ne!(primary.resource_id(), analytics.resource_id());
    assert_ne!(primary.resource_id(), cache.resource_id());
    assert_ne!(analytics.resource_id(), cache.resource_id());
}

#[test]
fn alternate_database_adapter_uses_the_same_resource_box_without_shell_changes() {
    let capture = Arc::new(Mutex::new(None));
    let tempdir = tempfile::tempdir().expect("tempdir");
    let root = tempdir.path().join("database");
    let config = DatabaseConfig { root: root.clone() };
    let composition = CompositionBuilder::new(
        CompositionId::new("test.database.synthetic".to_owned()).expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(fabric_core::BlockId::new("resource".to_owned()).expect("block id"))
            .register_module(ResourceRegistryModule::new())
            .register_module(module_factory({
                let config = config.clone();
                move || {
                    NativeDatabase::with_adapter(
                        config.clone(),
                        Arc::new(SyntheticDatabaseAdapter::new()),
                    )
                }
            }))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");

    let mut composition = composition
        .materialize(fabric_core::InstanceId::new("test.database.synthetic").expect("instance id"))
        .expect("materialize composition");
    composition.start().expect("start");
    let captured = capture
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured contracts");
    let resource_id = ResourceId::new("database").expect("resource id");
    let resource = captured
        .shell
        .resource(&resource_id)
        .expect("database resource");

    assert_eq!(resource.resource_id(), &database_resource_id());
    assert_eq!(
        resource.module_id().as_str(),
        "fabric.resource.database.native"
    );
    assert_eq!(resource.provided_contracts().len(), 1);
    assert_eq!(
        resource.provided_contracts()[0].as_str(),
        "fabric.resource.database"
    );
    assert_eq!(
        resource
            .configuration()
            .expect("database configuration")
            .kind()
            .as_str(),
        "database"
    );

    captured
        .shell
        .consume_configuration(
            &resource_id,
            &ResourceConfiguration::new(
                ResourceConfigurationKind::new("database").expect("configuration kind"),
                config.clone(),
            ),
        )
        .expect("consume database configuration");

    let inspection = captured.shell.inspect(&resource_id).expect("inspection");
    assert_eq!(inspection.entries()[0].key(), "adapter");
    assert_eq!(inspection.entries()[0].public_value(), Some("synthetic"));
    assert_eq!(inspection.entries()[1].key(), "configured");
    assert_eq!(inspection.entries()[1].public_value(), Some("true"));
    assert_eq!(inspection.entries()[2].key(), "resource");
    assert_eq!(inspection.entries()[2].public_value(), Some("database"));

    let context = ResourceContext::root(ResourceBoundaryId::new("apps.notes").expect("boundary"));
    let prepared = captured
        .database
        .prepare(context.clone(), ResourceName::new("primary").expect("name"))
        .expect("prepare database");
    let reference = prepared.database_ref();
    captured
        .database
        .bind(
            &context,
            BindingName::new("primary-binding").expect("binding name"),
            &reference,
        )
        .expect("bind database reference");
    assert!(captured.sqlite_compatibility.is_none());

    let reparsed = DatabaseRef::parse(reference.encode()).expect("parse database ref");
    assert_eq!(reparsed.resource_id(), reference.resource_id());
    assert_eq!(reparsed.resource_id().resource(), database_resource_id());
}

#[test]
fn sqlite_adapter_exposes_only_the_narrow_sqlite_compatibility_seam() {
    let capture = Arc::new(Mutex::new(None));
    let tempdir = tempfile::tempdir().expect("tempdir");
    let root = tempdir.path().join("database");
    let config = DatabaseConfig { root: root.clone() };
    let composition = CompositionBuilder::new(
        CompositionId::new("test.database.sqlite-compatibility".to_owned())
            .expect("composition id"),
    )
    .register_block(
        BlockBuilder::new(fabric_core::BlockId::new("resource".to_owned()).expect("block id"))
            .register_module(ResourceRegistryModule::new())
            .register_module(module_factory({
                let config = config.clone();
                move || {
                    NativeDatabase::with_sqlite_compatibility(
                        config.clone(),
                        Arc::new(SyntheticSqliteDatabaseAdapter::new()),
                    )
                }
            }))
            .register_module(CaptureModule::new(Arc::clone(&capture)))
            .build(),
    )
    .build()
    .expect("composition");

    let mut composition = composition
        .materialize(
            fabric_core::InstanceId::new("test.database.sqlite-compatibility")
                .expect("instance id"),
        )
        .expect("materialize composition");
    composition.start().expect("start");
    let captured = capture
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured contracts");
    let context = ResourceContext::root(ResourceBoundaryId::new("apps.notes").expect("boundary"));
    let prepared = captured
        .database
        .prepare(context.clone(), ResourceName::new("primary").expect("name"))
        .expect("prepare database");
    let materialized = captured
        .sqlite_compatibility
        .expect("sqlite compatibility")
        .materialize_sqlite(&prepared.database_ref())
        .expect("materialize sqlite database");

    let path = materialized.sqlite_file_path();
    assert!(path.ends_with("database.sqlite"));
    assert!(path.starts_with(&root));
}

#[test]
fn canonical_database_semantic_files_do_not_leak_sqlite_or_deno_terms() {
    for file in ["/src/config.rs", "/src/model.rs"] {
        let source = std::fs::read_to_string(format!("{}{}", env!("CARGO_MANIFEST_DIR"), file))
            .expect("source");
        for forbidden in ["rusqlite", "SqliteFile", "database.sqlite", "WAL", "Deno"] {
            assert!(
                !source.contains(forbidden),
                "{file} leaked implementation-specific material: {forbidden}"
            );
        }
    }
}

#[test]
fn shell_configuration_uses_database_owned_effective_root_without_shell_mirror() {
    let capture = Arc::new(Mutex::new(None));
    let tempdir = tempfile::tempdir().expect("tempdir");
    let root_a = tempdir.path().join("database-a");
    let root_b = tempdir.path().join("database-b");
    let config_a = DatabaseConfig {
        root: root_a.clone(),
    };
    let composition = CompositionBuilder::new(
        CompositionId::new("test.database.configuration-ownership".to_owned())
            .expect("composition id"),
    )
    .register_block(
        fabric_core::BlockBuilder::new(
            fabric_core::BlockId::new("resource".to_owned()).expect("block id"),
        )
        .register_module(ResourceRegistryModule::new())
        .register_module(module_factory({
            let config = config_a.clone();
            move || {
                NativeDatabase::with_sqlite_compatibility(
                    config.clone(),
                    Arc::new(SyntheticSqliteDatabaseAdapter::new()),
                )
            }
        }))
        .register_module(CaptureModule::new(Arc::clone(&capture)))
        .build(),
    )
    .build()
    .expect("composition");

    let mut composition = composition
        .materialize(
            fabric_core::InstanceId::new("test.database.configuration-ownership")
                .expect("instance id"),
        )
        .expect("materialize composition");
    composition.start().expect("start");
    let captured = capture
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured contracts");
    let resource_id = ResourceId::new("database").expect("resource id");
    let context = ResourceContext::root(ResourceBoundaryId::new("apps.notes").expect("boundary"));

    let prepared_primary = captured
        .database
        .prepare(context.clone(), ResourceName::new("primary").expect("name"))
        .expect("prepare primary database");
    let sqlite = captured
        .sqlite_compatibility
        .clone()
        .expect("sqlite compatibility");
    let materialized_primary = sqlite
        .materialize_sqlite(&prepared_primary.database_ref())
        .expect("materialize primary database");
    assert!(materialized_primary.sqlite_file_path().starts_with(&root_a));

    captured
        .shell
        .consume_configuration(
            &resource_id,
            &ResourceConfiguration::new(
                ResourceConfigurationKind::new("database").expect("configuration kind"),
                config_a.clone(),
            ),
        )
        .expect("replay database config");

    let rejected = captured
        .shell
        .consume_configuration(
            &resource_id,
            &ResourceConfiguration::new(
                ResourceConfigurationKind::new("database").expect("configuration kind"),
                DatabaseConfig {
                    root: root_b.clone(),
                },
            ),
        )
        .expect_err("changing database root must be rejected");
    assert!(!rejected.to_string().contains(&root_b.display().to_string()));

    let prepared_analytics = captured
        .database
        .prepare(context, ResourceName::new("analytics").expect("name"))
        .expect("prepare analytics database");
    let materialized_analytics = sqlite
        .materialize_sqlite(&prepared_analytics.database_ref())
        .expect("materialize analytics database");
    assert!(
        materialized_analytics
            .sqlite_file_path()
            .starts_with(&root_a)
    );
    assert!(
        !materialized_analytics
            .sqlite_file_path()
            .starts_with(&root_b)
    );
}
