use std::sync::Arc;

use fabric_binding::{
    BindingConsumer, BindingConsumerId, BindingConsumerKind, BindingId, BindingName,
};
use fabric_core::{ContractId, ContractKey};
use fabric_resource::ResourceId;
use fabric_resource::{
    ResourceContext, ResourceError, ResourceInstanceId, ResourceName,
    resource_context_binding_provenance,
};

use crate::{
    DatabaseBinding, DatabaseRef, PreparedDatabase, PreparedDatabaseState,
    SqliteDatabaseMaterialization,
};

pub trait DatabaseService: Send + Sync {
    fn prepare(
        &self,
        resource_id: &ResourceInstanceId,
    ) -> Result<PreparedDatabaseState, ResourceError>;
    fn cleanup(&self, prepared: &PreparedDatabase);
    fn verify(&self, resource_id: &ResourceInstanceId) -> Result<(), ResourceError>;
}

pub trait SqliteDatabaseCompatibility: Send + Sync {
    fn materialize_sqlite(
        &self,
        reference: &DatabaseRef,
    ) -> Result<SqliteDatabaseMaterialization, ResourceError>;
}

#[derive(Clone)]
pub struct DatabaseContract {
    inner: Arc<dyn DatabaseService>,
}

impl DatabaseContract {
    pub fn new(inner: Arc<dyn DatabaseService>) -> Self {
        Self { inner }
    }

    pub fn prepare(
        &self,
        context: ResourceContext,
        name: ResourceName,
    ) -> Result<PreparedDatabase, ResourceError> {
        let resource_id = ResourceInstanceId::canonical(&database_resource_id(), &context, &name);
        let prepared = self.inner.prepare(&resource_id)?;
        Ok(PreparedDatabase {
            database_ref: DatabaseRef::from_resource_id(resource_id.clone()),
            resource_id,
            name,
            context,
            created: prepared.created,
        })
    }

    pub fn cleanup(&self, prepared: &PreparedDatabase) {
        self.inner.cleanup(prepared)
    }

    pub fn reference(&self, context: &ResourceContext, name: &ResourceName) -> DatabaseRef {
        DatabaseRef::from_resource_id(ResourceInstanceId::canonical(
            &database_resource_id(),
            context,
            name,
        ))
    }

    pub fn resolve(
        &self,
        context: &ResourceContext,
        name: &ResourceName,
    ) -> Result<DatabaseBinding, ResourceError> {
        let resource_id = ResourceInstanceId::canonical(&database_resource_id(), context, name);
        let binding_name = binding_name_from_resource_name(name)?;
        self.resolve_existing(resource_id, context, binding_name)
    }

    pub fn bind(
        &self,
        context: &ResourceContext,
        name: BindingName,
        reference: &DatabaseRef,
    ) -> Result<DatabaseBinding, ResourceError> {
        self.resolve_existing(reference.resource_id().clone(), context, name)
    }

    fn resolve_existing(
        &self,
        resource_id: ResourceInstanceId,
        context: &ResourceContext,
        name: BindingName,
    ) -> Result<DatabaseBinding, ResourceError> {
        let provenance = resource_context_binding_provenance(context);
        let consumer = resource_context_consumer(&provenance)?;
        let binding_id = BindingId::resource_instance_import(
            database_resource_id().as_str(),
            resource_id.as_str(),
            provenance.identity_material(),
            &name,
        );
        self.inner.verify(&resource_id)?;
        Ok(DatabaseBinding {
            consumer,
            resource_id,
            binding_id,
            name,
        })
    }
}

pub fn database_contract_key() -> ContractKey<DatabaseContract> {
    ContractKey::provisional(database_contract_id())
}

pub fn database_contract_id() -> ContractId {
    ContractId::new("fabric.resource.database").expect("static resource contract id")
}

#[derive(Clone)]
pub struct SqliteDatabaseCompatibilityContract {
    inner: Arc<dyn SqliteDatabaseCompatibility>,
}

impl SqliteDatabaseCompatibilityContract {
    pub fn new(inner: Arc<dyn SqliteDatabaseCompatibility>) -> Self {
        Self { inner }
    }

    pub fn materialize_sqlite(
        &self,
        reference: &DatabaseRef,
    ) -> Result<SqliteDatabaseMaterialization, ResourceError> {
        self.inner.materialize_sqlite(reference)
    }
}

pub fn sqlite_database_compatibility_contract_key()
-> ContractKey<SqliteDatabaseCompatibilityContract> {
    ContractKey::provisional(sqlite_database_compatibility_contract_id())
}

pub fn sqlite_database_compatibility_contract_id() -> ContractId {
    ContractId::new("fabric.resource.database.compatibility.sqlite")
        .expect("static sqlite database compatibility contract id")
}

pub fn database_resource_id() -> ResourceId {
    ResourceId::new("database").expect("static database resource id")
}

fn binding_name_from_resource_name(name: &ResourceName) -> Result<BindingName, ResourceError> {
    BindingName::new(name.as_str()).map_err(|error| ResourceError::InvalidInput {
        message: error.to_string(),
    })
}

fn resource_context_consumer(
    provenance: &fabric_resource::ResourceContextBindingProvenance,
) -> Result<BindingConsumer, ResourceError> {
    Ok(BindingConsumer::new(
        BindingConsumerKind::new("resource-context").expect("static binding consumer kind"),
        BindingConsumerId::new(provenance.consumer_id()).map_err(|error| {
            ResourceError::InvalidInput {
                message: error.to_string(),
            }
        })?,
    ))
}
