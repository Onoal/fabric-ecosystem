use std::sync::Arc;

use fabric_core::ContractRequirement;
use fabric_core::{ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime};
use fabric_resource_registry::{
    ResourceConfigurationFacet, ResourceConfigurationKind, ResourceDescriptor,
    ResourceInspectionFacet, ResourceRegistry, resource_registry_contract_id,
};

use crate::DatabaseConfig;
use crate::contract::{
    DatabaseContract, DatabaseService, SqliteDatabaseCompatibility,
    SqliteDatabaseCompatibilityContract, database_contract_key, database_resource_id,
    sqlite_database_compatibility_contract_key,
};

use super::adapter::DatabaseAdapter;
use super::{bind_noop, start_noop};

#[derive(Clone, Default)]
pub struct DatabaseAdapterSupport {
    sqlite_compatibility: Option<Arc<dyn SqliteDatabaseCompatibility>>,
}

impl DatabaseAdapterSupport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sqlite_compatibility(
        mut self,
        sqlite_compatibility: Arc<dyn SqliteDatabaseCompatibility>,
    ) -> Self {
        self.sqlite_compatibility = Some(sqlite_compatibility);
        self
    }
}

pub struct NativeDatabase {
    module_id: ModuleId,
    resource_registry_requirement: ContractRequirement<ResourceRegistry>,
    resource_registry: Option<ResourceRegistry>,
    config: DatabaseConfig,
    adapter: Arc<dyn DatabaseAdapter>,
    sqlite_compatibility: Option<Arc<dyn SqliteDatabaseCompatibility>>,
}

impl NativeDatabase {
    pub fn new(
        config: DatabaseConfig,
        adapter: Arc<dyn DatabaseAdapter>,
        support: DatabaseAdapterSupport,
    ) -> Self {
        Self {
            module_id: ModuleId::new("fabric.resource.database.native").expect("static module id"),
            resource_registry_requirement: ContractRequirement::provisional(
                resource_registry_contract_id(),
            ),
            resource_registry: None,
            config,
            adapter,
            sqlite_compatibility: support.sqlite_compatibility,
        }
    }

    pub fn with_adapter(config: DatabaseConfig, adapter: Arc<dyn DatabaseAdapter>) -> Self {
        Self::new(config, adapter, DatabaseAdapterSupport::new())
    }

    pub fn with_sqlite_compatibility<A>(config: DatabaseConfig, adapter: Arc<A>) -> Self
    where
        A: DatabaseAdapter + SqliteDatabaseCompatibility + 'static,
    {
        let database_adapter: Arc<dyn DatabaseAdapter> = adapter.clone();
        let sqlite_compatibility: Arc<dyn SqliteDatabaseCompatibility> = adapter;
        Self::new(
            config,
            database_adapter,
            DatabaseAdapterSupport::new().with_sqlite_compatibility(sqlite_compatibility),
        )
    }
}

impl ModuleRuntime for NativeDatabase {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        let mut contracts = vec![database_contract_key().declaration()];
        if self.sqlite_compatibility.is_some() {
            contracts.push(sqlite_database_compatibility_contract_key().declaration());
        }
        contracts
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![self.resource_registry_requirement.declaration().clone()]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        let mut contracts = vec![ModuleContract::new(
            &database_contract_key(),
            Arc::new(DatabaseContract::new(
                Arc::clone(&self.adapter) as Arc<dyn DatabaseService>
            )),
        )];
        if let Some(sqlite_compatibility) = &self.sqlite_compatibility {
            contracts.push(ModuleContract::new(
                &sqlite_database_compatibility_contract_key(),
                Arc::new(SqliteDatabaseCompatibilityContract::new(Arc::clone(
                    sqlite_compatibility,
                ))),
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
        bind_noop(bindings)
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        self.adapter.initialize(&self.config)
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        self.adapter
            .consume_configuration(&self.config)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        if let Some(shell) = &self.resource_registry {
            let configuration_adapter = Arc::clone(&self.adapter);
            let inspection_adapter = Arc::clone(&self.adapter);
            shell
                .register(
                    self,
                    ResourceDescriptor::new(database_resource_id())
                        .with_configuration(
                            ResourceConfigurationFacet::typed_with::<DatabaseConfig>(
                                ResourceConfigurationKind::new("database")
                                    .expect("static resource configuration kind"),
                                move |config| configuration_adapter.consume_configuration(config),
                            ),
                        )
                        .with_inspection(ResourceInspectionFacet::new(move || {
                            inspection_adapter.inspect()
                        })),
                )
                .map_err(|error| ModuleError::new(error.to_string()))?;
        }
        start_noop()
    }

    fn stop(&mut self) {
        if let Some(shell) = &self.resource_registry {
            let _ = shell.unregister(&self.module_id);
        }
    }

    fn health(&self) -> fabric_core::Health {
        self.adapter.health()
    }
}
