use std::sync::Arc;

use fabric_binding::{BindingConsumer, BindingId, BindingName};
use fabric_resource::{ResourceError, ResourceInstanceId};

use crate::kv_resource_id;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvRef {
    resource_id: ResourceInstanceId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedKv {
    pub resource_id: ResourceInstanceId,
    pub(crate) kv_ref: KvRef,
    pub name: fabric_resource::ResourceName,
    pub context: fabric_resource::ResourceContext,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedKvState {
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvBinding {
    pub(crate) consumer: BindingConsumer,
    pub(crate) resource_id: ResourceInstanceId,
    pub(crate) binding_id: BindingId,
    pub(crate) name: BindingName,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KvMutation {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvListQuery {
    prefix: Vec<u8>,
    after: Option<Vec<u8>>,
    limit: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvEntry {
    key: Vec<u8>,
    value: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvListPage {
    entries: Vec<KvEntry>,
    next_after: Option<Vec<u8>>,
}

#[doc(hidden)]
#[derive(Clone)]
pub struct KvRuntimeHandle {
    resource_id: ResourceInstanceId,
    runtime: Arc<dyn KvRuntimeApi>,
}

#[derive(Clone)]
pub enum KvAccess {
    ProviderOwned { runtime: KvRuntimeHandle },
}

impl KvRef {
    pub(crate) fn from_resource_id(resource_id: ResourceInstanceId) -> Self {
        Self { resource_id }
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, ResourceError> {
        const PREFIX: &str = "fabric-resource-kv-ref-v1:";

        let value = value.into();
        let Some(resource_id) = value.strip_prefix(PREFIX) else {
            return Err(ResourceError::InvalidInput {
                message: "kv reference must use the fabric-resource-kv-ref-v1 prefix".to_owned(),
            });
        };

        let resource_id = ResourceInstanceId::parse(resource_id.to_owned())?;
        if resource_id.resource() != kv_resource_id() {
            return Err(ResourceError::InvalidInput {
                message: "kv reference must point to a kv resource".to_owned(),
            });
        }

        Ok(Self { resource_id })
    }

    pub fn encode(&self) -> String {
        format!("fabric-resource-kv-ref-v1:{}", self.resource_id.as_str())
    }

    pub fn resource_id(&self) -> &ResourceInstanceId {
        &self.resource_id
    }
}

impl PreparedKv {
    pub fn kv_ref(&self) -> KvRef {
        self.kv_ref.clone()
    }
}

impl KvBinding {
    pub fn consumer(&self) -> &BindingConsumer {
        &self.consumer
    }

    pub fn binding_id(&self) -> &BindingId {
        &self.binding_id
    }

    pub fn name(&self) -> &BindingName {
        &self.name
    }

    pub fn resource_id(&self) -> &ResourceInstanceId {
        &self.resource_id
    }

    pub fn kv_ref(&self) -> KvRef {
        KvRef::from_resource_id(self.resource_id.clone())
    }
}

impl KvMutation {
    pub fn put(key: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) -> Self {
        Self::Put {
            key: key.into(),
            value: value.into(),
        }
    }

    pub fn delete(key: impl Into<Vec<u8>>) -> Self {
        Self::Delete { key: key.into() }
    }
}

impl KvListQuery {
    pub fn new(
        prefix: impl Into<Vec<u8>>,
        after: Option<Vec<u8>>,
        limit: usize,
    ) -> Result<Self, ResourceError> {
        if limit == 0 {
            return Err(ResourceError::InvalidInput {
                message: "kv list query limit must be greater than zero".to_owned(),
            });
        }
        Ok(Self {
            prefix: prefix.into(),
            after,
            limit,
        })
    }

    pub fn prefix(&self) -> &[u8] {
        &self.prefix
    }

    pub fn after(&self) -> Option<&[u8]> {
        self.after.as_deref()
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

impl KvEntry {
    pub fn new(key: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }

    pub fn key(&self) -> &[u8] {
        &self.key
    }

    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

impl KvListPage {
    pub fn new(entries: Vec<KvEntry>, next_after: Option<Vec<u8>>) -> Self {
        Self {
            entries,
            next_after,
        }
    }

    pub fn entries(&self) -> &[KvEntry] {
        &self.entries
    }

    pub fn next_after(&self) -> Option<&[u8]> {
        self.next_after.as_deref()
    }
}

impl KvRuntimeHandle {
    #[doc(hidden)]
    pub fn new(resource_id: ResourceInstanceId, runtime: Arc<dyn KvRuntimeApi>) -> Self {
        Self {
            resource_id,
            runtime,
        }
    }

    pub fn resource_id(&self) -> &ResourceInstanceId {
        &self.resource_id
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ResourceError> {
        self.runtime.get(key)
    }

    pub fn put(&self, key: &[u8], value: &[u8]) -> Result<(), ResourceError> {
        self.runtime.put(key, value)
    }

    pub fn delete(&self, key: &[u8]) -> Result<(), ResourceError> {
        self.runtime.delete(key)
    }

    pub fn contains(&self, key: &[u8]) -> Result<bool, ResourceError> {
        self.runtime.contains(key)
    }

    pub fn list(&self, query: &KvListQuery) -> Result<KvListPage, ResourceError> {
        self.runtime.list(query)
    }

    pub fn write_batch(&self, mutations: &[KvMutation]) -> Result<(), ResourceError> {
        self.runtime.write_batch(mutations)
    }
}

impl KvAccess {
    #[doc(hidden)]
    pub fn testing_stub(resource_id: ResourceInstanceId) -> Self {
        Self::ProviderOwned {
            runtime: KvRuntimeHandle::new(resource_id, Arc::new(UnavailableKvRuntime)),
        }
    }

    pub fn resource_id(&self) -> &ResourceInstanceId {
        match self {
            Self::ProviderOwned { runtime } => runtime.resource_id(),
        }
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ResourceError> {
        match self {
            Self::ProviderOwned { runtime } => runtime.get(key),
        }
    }

    pub fn put(&self, key: &[u8], value: &[u8]) -> Result<(), ResourceError> {
        match self {
            Self::ProviderOwned { runtime } => runtime.put(key, value),
        }
    }

    pub fn delete(&self, key: &[u8]) -> Result<(), ResourceError> {
        match self {
            Self::ProviderOwned { runtime } => runtime.delete(key),
        }
    }

    pub fn contains(&self, key: &[u8]) -> Result<bool, ResourceError> {
        match self {
            Self::ProviderOwned { runtime } => runtime.contains(key),
        }
    }

    pub fn list(&self, query: &KvListQuery) -> Result<KvListPage, ResourceError> {
        match self {
            Self::ProviderOwned { runtime } => runtime.list(query),
        }
    }

    pub fn write_batch(&self, mutations: &[KvMutation]) -> Result<(), ResourceError> {
        match self {
            Self::ProviderOwned { runtime } => runtime.write_batch(mutations),
        }
    }
}

impl std::fmt::Debug for KvRuntimeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KvRuntimeHandle")
            .field("resource_id", &self.resource_id)
            .finish_non_exhaustive()
    }
}

impl PartialEq for KvRuntimeHandle {
    fn eq(&self, other: &Self) -> bool {
        self.resource_id == other.resource_id
    }
}

impl Eq for KvRuntimeHandle {}

impl std::fmt::Debug for KvAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProviderOwned { runtime } => f
                .debug_struct("KvAccess::ProviderOwned")
                .field("resource_id", runtime.resource_id())
                .finish(),
        }
    }
}

impl PartialEq for KvAccess {
    fn eq(&self, other: &Self) -> bool {
        self.resource_id() == other.resource_id()
    }
}

impl Eq for KvAccess {}

#[doc(hidden)]
pub trait KvRuntimeApi: Send + Sync {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ResourceError>;
    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), ResourceError>;
    fn delete(&self, key: &[u8]) -> Result<(), ResourceError>;
    fn contains(&self, key: &[u8]) -> Result<bool, ResourceError>;
    fn list(&self, query: &KvListQuery) -> Result<KvListPage, ResourceError>;
    fn write_batch(&self, mutations: &[KvMutation]) -> Result<(), ResourceError>;
}

struct UnavailableKvRuntime;

impl KvRuntimeApi for UnavailableKvRuntime {
    fn get(&self, _key: &[u8]) -> Result<Option<Vec<u8>>, ResourceError> {
        Err(unavailable_kv_runtime())
    }

    fn put(&self, _key: &[u8], _value: &[u8]) -> Result<(), ResourceError> {
        Err(unavailable_kv_runtime())
    }

    fn delete(&self, _key: &[u8]) -> Result<(), ResourceError> {
        Err(unavailable_kv_runtime())
    }

    fn contains(&self, _key: &[u8]) -> Result<bool, ResourceError> {
        Err(unavailable_kv_runtime())
    }

    fn list(&self, _query: &KvListQuery) -> Result<KvListPage, ResourceError> {
        Err(unavailable_kv_runtime())
    }

    fn write_batch(&self, _mutations: &[KvMutation]) -> Result<(), ResourceError> {
        Err(unavailable_kv_runtime())
    }
}

fn unavailable_kv_runtime() -> ResourceError {
    ResourceError::Integrity {
        message: "kv access testing stub does not provide runtime operations".to_owned(),
    }
}
