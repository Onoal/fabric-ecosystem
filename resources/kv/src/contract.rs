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
    KvAccess, KvBinding, KvListPage, KvListQuery, KvMutation, KvRef, PreparedKv, PreparedKvState,
};

pub trait KvService: Send + Sync {
    fn prepare(&self, resource_id: &ResourceInstanceId) -> Result<PreparedKvState, ResourceError>;
    fn cleanup(&self, prepared: &PreparedKv);
    fn resolve(&self, resource_id: &ResourceInstanceId) -> Result<KvAccess, ResourceError>;
}

#[derive(Clone)]
pub struct KvContract {
    inner: Arc<dyn KvService>,
}

impl KvContract {
    pub fn new(inner: Arc<dyn KvService>) -> Self {
        Self { inner }
    }

    pub fn prepare(
        &self,
        context: ResourceContext,
        name: ResourceName,
    ) -> Result<PreparedKv, ResourceError> {
        let resource_id = ResourceInstanceId::canonical(&kv_resource_id(), &context, &name);
        let prepared = self.inner.prepare(&resource_id)?;
        Ok(PreparedKv {
            kv_ref: KvRef::from_resource_id(resource_id.clone()),
            resource_id,
            name,
            context,
            created: prepared.created,
        })
    }

    pub fn cleanup(&self, prepared: &PreparedKv) {
        self.inner.cleanup(prepared)
    }

    pub fn reference(&self, context: &ResourceContext, name: &ResourceName) -> KvRef {
        KvRef::from_resource_id(ResourceInstanceId::canonical(
            &kv_resource_id(),
            context,
            name,
        ))
    }

    pub fn resolve(
        &self,
        context: &ResourceContext,
        name: &ResourceName,
    ) -> Result<KvBinding, ResourceError> {
        let resource_id = ResourceInstanceId::canonical(&kv_resource_id(), context, name);
        let binding_name = binding_name_from_resource_name(name)?;
        self.resolve_existing(resource_id, context, binding_name)
    }

    pub fn bind(
        &self,
        context: &ResourceContext,
        name: BindingName,
        reference: &KvRef,
    ) -> Result<KvBinding, ResourceError> {
        self.resolve_existing(reference.resource_id().clone(), context, name)
    }

    pub fn get(&self, binding: &KvBinding, key: &[u8]) -> Result<Option<Vec<u8>>, ResourceError> {
        self.access(&binding.kv_ref())?.get(key)
    }

    pub fn put(&self, binding: &KvBinding, key: &[u8], value: &[u8]) -> Result<(), ResourceError> {
        self.access(&binding.kv_ref())?.put(key, value)
    }

    pub fn delete(&self, binding: &KvBinding, key: &[u8]) -> Result<(), ResourceError> {
        self.access(&binding.kv_ref())?.delete(key)
    }

    pub fn contains(&self, binding: &KvBinding, key: &[u8]) -> Result<bool, ResourceError> {
        self.access(&binding.kv_ref())?.contains(key)
    }

    pub fn list(
        &self,
        binding: &KvBinding,
        query: &KvListQuery,
    ) -> Result<KvListPage, ResourceError> {
        self.access(&binding.kv_ref())?.list(query)
    }

    pub fn write_batch(
        &self,
        binding: &KvBinding,
        mutations: &[KvMutation],
    ) -> Result<(), ResourceError> {
        self.access(&binding.kv_ref())?.write_batch(mutations)
    }

    fn resolve_existing(
        &self,
        resource_id: ResourceInstanceId,
        context: &ResourceContext,
        name: BindingName,
    ) -> Result<KvBinding, ResourceError> {
        let provenance = resource_context_binding_provenance(context);
        let consumer = resource_context_consumer(&provenance)?;
        let binding_id = BindingId::resource_instance_import(
            kv_resource_id().as_str(),
            resource_id.as_str(),
            provenance.identity_material(),
            &name,
        );
        self.inner.resolve(&resource_id)?;
        Ok(KvBinding {
            consumer,
            resource_id,
            binding_id,
            name,
        })
    }

    pub fn access(&self, reference: &KvRef) -> Result<KvAccess, ResourceError> {
        self.inner.resolve(reference.resource_id())
    }
}

pub fn kv_contract_key() -> ContractKey<KvContract> {
    ContractKey::provisional(kv_contract_id())
}

pub fn kv_contract_id() -> ContractId {
    ContractId::new("fabric.resource.kv").expect("static resource contract id")
}

pub fn kv_resource_id() -> ResourceId {
    ResourceId::new("kv").expect("static kv resource id")
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
