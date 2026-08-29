use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use fabric_adapter_kv_fjall::{FjallKvAdapter, FjallKvConfig};
use fabric_binding::BindingName;
use fabric_component::{
    Component, ComponentId, ComponentParticipation, ComponentRegistry, ComponentRuntime,
    ComponentRuntimeModule,
};
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    Instance, InstanceId, Module, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime, module_factory,
};
use fabric_resource::{
    ResourceBoundaryId, ResourceContext, ResourceError, ResourceInstanceId, ResourceName,
    ResourceScope,
};
use fabric_resource_kv::{
    KvAccess, KvAdapter, KvContract, KvEntry, KvListPage, KvListQuery, KvMutation, KvRuntimeApi,
    KvRuntimeHandle, KvService, NativeKv, PreparedKv, PreparedKvState,
};

const COMPONENT_ID: &str = "component.kv.consumer";
const INSTANCE_ID: &str = "fabric.fabric.vertical.cross-api";
const COMPOSITION_ID: &str = "fabric.fabric.vertical.cross-api";
const KV_RESOURCE_NAME: &str = "shared-cache";
const BINDING_NAME: &str = "cache";
const KEY: &[u8] = b"message";
const VALUE: &[u8] = b"hello-from-component";

type MemoryKvEntries = BTreeMap<Vec<u8>, Vec<u8>>;
type MemoryKvResourceStore = BTreeMap<ResourceInstanceId, MemoryKvEntries>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct KvConsumerObservation {
    component_id: ComponentId,
    binding_id: String,
    resource_id: String,
    value: Vec<u8>,
}

#[derive(Clone)]
struct KvConsumerComponentModule {
    module_id: ModuleId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registry_requirement: ContractRequirement<ComponentRegistry>,
    kv_requirement: ContractRequirement<KvContract>,
    runtime: Option<ComponentRuntime>,
    registry: Option<ComponentRegistry>,
    kv: Option<KvContract>,
    participation: Option<ComponentParticipation>,
    observed: Arc<Mutex<Option<KvConsumerObservation>>>,
}

impl KvConsumerComponentModule {
    fn new(observed: Arc<Mutex<Option<KvConsumerObservation>>>) -> Self {
        Self {
            module_id: ModuleId::new("fabric.test.component.kv-consumer").expect("module id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry_requirement: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            kv_requirement: ContractRequirement::provisional(fabric_resource_kv::kv_contract_id()),
            runtime: None,
            registry: None,
            kv: None,
            participation: None,
            observed,
        }
    }

    fn component(runtime: &ComponentRuntime) -> Component {
        Component::bind(
            ComponentId::new(COMPONENT_ID).expect("component id"),
            runtime,
        )
    }
}

impl ModuleRuntime for KvConsumerComponentModule {
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
            self.runtime_requirement.id().clone(),
            self.registry_requirement.id().clone(),
            self.kv_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        self.runtime = Some(
            bindings
                .resolve(&self.runtime_requirement)
                .map_err(module_error)?
                .as_ref()
                .clone(),
        );
        self.registry = Some(
            bindings
                .resolve(&self.registry_requirement)
                .map_err(module_error)?
                .as_ref()
                .clone(),
        );
        self.kv = Some(
            bindings
                .resolve(&self.kv_requirement)
                .map_err(module_error)?
                .as_ref()
                .clone(),
        );
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| ModuleError::new("component runtime not bound"))?;
        let registry = self
            .registry
            .as_ref()
            .ok_or_else(|| ModuleError::new("component registry not bound"))?;
        let kv = self
            .kv
            .as_ref()
            .ok_or_else(|| ModuleError::new("kv contract not bound"))?;

        let component = Self::component(runtime);
        let status = registry
            .register(component.clone(), Health::Healthy)
            .map_err(module_error)?;
        let status = registry
            .activate(status.participation())
            .map_err(module_error)?;

        let owner_context =
            ResourceContext::root(ResourceBoundaryId::new("fc5.cross-api").expect("boundary"));
        let consumer_context = owner_context
            .clone()
            .child(ResourceScope::new("component-consumer").expect("scope"));
        let name = ResourceName::new(KV_RESOURCE_NAME).expect("resource name");
        let prepared = kv
            .prepare(owner_context, name)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let binding = kv
            .bind(
                &consumer_context,
                BindingName::new(BINDING_NAME).expect("binding name"),
                &prepared.kv_ref(),
            )
            .map_err(|error| ModuleError::new(error.to_string()))?;
        kv.put(&binding, KEY, VALUE)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let value = kv
            .get(&binding, KEY)
            .map_err(|error| ModuleError::new(error.to_string()))?
            .expect("stored kv value");

        *self.observed.lock().expect("observation lock") = Some(KvConsumerObservation {
            component_id: component.component_id().clone(),
            binding_id: binding.binding_id().as_str().to_owned(),
            resource_id: binding.resource_id().as_str().to_owned(),
            value,
        });
        self.participation = Some(status.participation().clone());
        Ok(())
    }

    fn stop(&mut self) {
        if let (Some(registry), Some(participation)) = (&self.registry, self.participation.take()) {
            let _ = registry.unregister(&participation);
        }
    }

    fn health(&self) -> Health {
        Health::Healthy
    }
}

#[derive(Default)]
struct MemoryKvStore {
    entries: Mutex<MemoryKvResourceStore>,
}

struct MemoryKvRuntime {
    resource_id: ResourceInstanceId,
    store: Arc<MemoryKvStore>,
}

#[derive(Clone)]
struct MemoryKvService {
    store: Arc<MemoryKvStore>,
}

struct MemoryKvAdapter {
    service: Arc<MemoryKvService>,
}

impl MemoryKvAdapter {
    fn new() -> Self {
        let store = Arc::new(MemoryKvStore::default());
        Self {
            service: Arc::new(MemoryKvService { store }),
        }
    }
}

impl KvRuntimeApi for MemoryKvRuntime {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ResourceError> {
        Ok(self
            .store
            .entries
            .lock()
            .expect("kv store lock")
            .get(&self.resource_id)
            .and_then(|entry| entry.get(key).cloned()))
    }

    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), ResourceError> {
        self.store
            .entries
            .lock()
            .expect("kv store lock")
            .entry(self.resource_id.clone())
            .or_default()
            .insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn delete(&self, key: &[u8]) -> Result<(), ResourceError> {
        if let Some(entries) = self
            .store
            .entries
            .lock()
            .expect("kv store lock")
            .get_mut(&self.resource_id)
        {
            entries.remove(key);
        }
        Ok(())
    }

    fn contains(&self, key: &[u8]) -> Result<bool, ResourceError> {
        Ok(self
            .store
            .entries
            .lock()
            .expect("kv store lock")
            .get(&self.resource_id)
            .is_some_and(|entries| entries.contains_key(key)))
    }

    fn list(&self, query: &KvListQuery) -> Result<KvListPage, ResourceError> {
        let state = self.store.entries.lock().expect("kv store lock");
        let mut entries = state
            .get(&self.resource_id)
            .into_iter()
            .flat_map(|entries| entries.iter())
            .filter(|(key, _)| key.starts_with(query.prefix()))
            .map(|(key, value)| KvEntry::new(key.clone(), value.clone()))
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.key().cmp(right.key()));
        if let Some(after) = query.after() {
            entries.retain(|entry| entry.key() > after);
        }
        let limit = query.limit();
        let next_after = if entries.len() > limit {
            Some(entries[limit - 1].key().to_vec())
        } else {
            None
        };
        entries.truncate(limit);
        Ok(KvListPage::new(entries, next_after))
    }

    fn write_batch(&self, mutations: &[KvMutation]) -> Result<(), ResourceError> {
        for mutation in mutations {
            match mutation {
                KvMutation::Put { key, value } => self.put(key, value)?,
                KvMutation::Delete { key } => self.delete(key)?,
            }
        }
        Ok(())
    }
}

impl KvService for MemoryKvService {
    fn prepare(&self, resource_id: &ResourceInstanceId) -> Result<PreparedKvState, ResourceError> {
        let mut state = self.store.entries.lock().expect("kv store lock");
        let created = state.insert(resource_id.clone(), BTreeMap::new()).is_none();
        if !created {
            let _ = state.get(resource_id).expect("existing kv state");
        }
        Ok(PreparedKvState { created })
    }

    fn cleanup(&self, _prepared: &PreparedKv) {}

    fn resolve(&self, resource_id: &ResourceInstanceId) -> Result<KvAccess, ResourceError> {
        self.store
            .entries
            .lock()
            .expect("kv store lock")
            .entry(resource_id.clone())
            .or_default();
        Ok(KvAccess::ProviderOwned {
            runtime: KvRuntimeHandle::new(
                resource_id.clone(),
                Arc::new(MemoryKvRuntime {
                    resource_id: resource_id.clone(),
                    store: Arc::clone(&self.store),
                }),
            ),
        })
    }
}

impl KvAdapter for MemoryKvAdapter {
    fn service(&self) -> Arc<dyn KvService> {
        Arc::clone(&self.service) as Arc<dyn KvService>
    }

    fn initialize(&self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&self) -> Result<(), ResourceError> {
        Ok(())
    }

    fn stop(&self) {}
}

fn module_error(error: impl std::fmt::Display) -> ModuleError {
    ModuleError::new(error.to_string())
}

struct CompositionFixture {
    composition: fabric_core::Composition,
    observed: Arc<Mutex<Option<KvConsumerObservation>>>,
}

impl CompositionFixture {
    fn start(&self) -> (Instance, KvConsumerObservation) {
        let mut instance = self
            .composition
            .materialize(InstanceId::new(INSTANCE_ID).expect("instance id"))
            .expect("materialize composition");
        instance.start().expect("start composition");
        let observed = self
            .observed
            .lock()
            .expect("observation lock")
            .clone()
            .expect("observation");
        (instance, observed)
    }
}

fn build_composition_fixture(
    composition_id: &str,
    kv_module: impl Module + 'static,
) -> CompositionFixture {
    let observed = Arc::new(Mutex::new(None));
    let composition =
        CompositionBuilder::new(CompositionId::new(composition_id).expect("composition id"))
            .register_block(
                BlockBuilder::new(BlockId::new("resources").expect("block id"))
                    .register_module(kv_module)
                    .build(),
            )
            .register_block(
                BlockBuilder::new(BlockId::new("components").expect("block id"))
                    .register_module(ComponentRuntimeModule::new())
                    .register_module(KvConsumerComponentModule::new(Arc::clone(&observed)))
                    .build(),
            )
            .build()
            .expect("composition");
    CompositionFixture {
        composition,
        observed,
    }
}

fn fjall_composition(root: &Path) -> CompositionFixture {
    build_composition_fixture(
        COMPOSITION_ID,
        module_factory({
            let root = root.to_path_buf();
            move || {
                NativeKv::new(Arc::new(FjallKvAdapter::new(FjallKvConfig::new(
                    root.clone(),
                ))))
            }
        }),
    )
}

fn memory_composition() -> CompositionFixture {
    build_composition_fixture(
        "fabric.fabric.vertical.cross-api.memory",
        module_factory(|| NativeKv::new(Arc::new(MemoryKvAdapter::new()))),
    )
}

#[test]
fn component_consumes_kv_through_explicit_contract_binding_without_resource_registry_lookup() {
    let root = tempfile::tempdir().expect("tempdir");
    let composition = fjall_composition(root.path());
    let (mut instance, observed) = composition.start();

    assert_eq!(
        observed.component_id,
        ComponentId::new(COMPONENT_ID).expect("component id")
    );
    assert!(observed.binding_id.starts_with("fabric-binding-"));
    assert!(observed.resource_id.starts_with("fabric-resource-kv-"));
    assert_eq!(observed.value, VALUE);

    instance.stop();
}

#[test]
fn same_component_code_consumes_the_same_kv_resource_api_with_a_different_adapter() {
    let root = tempfile::tempdir().expect("tempdir");
    let fjall = fjall_composition(root.path().join("fjall").as_path());
    let memory = memory_composition();

    let (mut fjall_instance, fjall_observed) = fjall.start();
    let (mut memory_instance, memory_observed) = memory.start();

    assert_eq!(fjall_observed.component_id, memory_observed.component_id);
    assert_eq!(fjall_observed.binding_id, memory_observed.binding_id);
    assert_eq!(fjall_observed.resource_id, memory_observed.resource_id);
    assert_eq!(fjall_observed.value, memory_observed.value);

    fjall_instance.stop();
    memory_instance.stop();
}

#[test]
fn component_resource_witness_source_stays_adapter_and_registry_free() {
    let source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cross_api/component_resource.rs"),
    )
    .expect("read component resource witness");

    let witness_start = source
        .find("struct KvConsumerComponentModule")
        .expect("component witness definition");
    let witness_end = source
        .find("struct MemoryKvStore")
        .expect("component witness end");
    let witness_source = &source[witness_start..witness_end];

    assert!(!witness_source.contains("ResourceRegistry"));
    assert!(!witness_source.contains("resource_registry_contract_key"));
    assert!(!witness_source.contains("fabric_adapter_"));
}
