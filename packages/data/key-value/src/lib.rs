//! KeyValue package for Fabric.
//!
//! The package publishes one semantic resource, a stateful in-memory adapter,
//! an augmentation marker, and reusable contribution helpers. The package name
//! never becomes built Composition truth.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use fabric::authoring::{
    ResourceAugmentation, ResourceAugmentationDefinition, ResourceAugmentationSupportDefinition,
};
use fabric::core::{
    Health, ModuleBindings, ModuleContract, ModuleDeclaration, ModuleError, ModuleId, ModuleRuntime,
};
use fabric::prelude::*;

fabric::resource! {
    pub KeyValue {
        id: "onoal.package.data.key-value";

        api {
            fn get(&self, key: String) -> Option<Vec<u8>>;
            fn set(&self, key: String, value: Vec<u8>);
            fn delete(&self, key: String) -> Option<Vec<u8>>;
        }
    }
}

#[derive(Default)]
pub struct MemoryKeyValueState {
    entries: Mutex<BTreeMap<String, Vec<u8>>>,
}

fabric::adapter! {
    pub MemoryKeyValue for KeyValue {
        id: "onoal.package.data.key-value.memory";

        state {
            MemoryKeyValueState = MemoryKeyValueState::default();
        }

        runtime {
            fn get(&self, key: String) -> Option<Vec<u8>> {
                self.state
                    .get()
                    .entries
                    .lock()
                    .expect("memory key-value state")
                    .get(&key)
                    .cloned()
            }

            fn set(&self, key: String, value: Vec<u8>) {
                self.state
                    .get()
                    .entries
                    .lock()
                    .expect("memory key-value state")
                    .insert(key, value);
            }

            fn delete(&self, key: String) -> Option<Vec<u8>> {
                self.state
                    .get()
                    .entries
                    .lock()
                    .expect("memory key-value state")
                    .remove(&key)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyValueRoundTrip {
    pub stored: Option<Vec<u8>>,
    pub deleted: Option<Vec<u8>>,
    pub after_delete: Option<Vec<u8>>,
}

fabric::component! {
    pub KeyValueClient {
        id: "onoal.package.data.key-value.client";

        relations {
            requires {
                store: KeyValue;
            }
        }

        api {
            fn write_read_delete(&self, key: String, value: Vec<u8>) -> KeyValueRoundTrip;
        }

        runtime {
            fn write_read_delete(&self, key: String, value: Vec<u8>) -> KeyValueRoundTrip {
                self.relations().store.set(key.clone(), value);
                let stored = self.relations().store.get(key.clone());
                let deleted = self.relations().store.delete(key.clone());
                let after_delete = self.relations().store.get(key);
                KeyValueRoundTrip {
                    stored,
                    deleted,
                    after_delete,
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DualKeyValueResult {
    pub primary: Option<Vec<u8>>,
    pub cache: Option<Vec<u8>>,
}

fabric::component! {
    pub DualKeyValueClient {
        id: "onoal.package.data.key-value.dual-client";

        relations {
            requires {
                primary: KeyValue;
                cache: KeyValue;
            }
        }

        api {
            fn write_both(&self) -> DualKeyValueResult;
        }

        runtime {
            fn write_both(&self) -> DualKeyValueResult {
                self.relations()
                    .primary
                    .set("shared".to_owned(), b"primary".to_vec());
                self.relations()
                    .cache
                    .set("shared".to_owned(), b"cache".to_vec());
                DualKeyValueResult {
                    primary: self.relations().primary.get("shared".to_owned()),
                    cache: self.relations().cache.get("shared".to_owned()),
                }
            }
        }
    }
}

/// Augmentation marker for stores that should be audited by package consumers.
pub struct KeyValueAudit;

#[derive(Clone)]
pub struct KeyValueAuditContract;

#[derive(Clone)]
pub struct KeyValueAuditSupport;

impl ResourceAugmentationDefinition<KeyValue> for KeyValueAudit {
    type Config = ();
    type Contract = KeyValueAuditContract;

    fn contract_key() -> fabric::core::ContractKey<Self::Contract> {
        fabric::core::ContractKey::provisional(
            fabric::core::ContractId::new("onoal.package.data.key-value.audit".to_owned())
                .expect("static contract id"),
        )
    }
}

impl ResourceAugmentationSupportDefinition<KeyValue, KeyValueAudit> for KeyValueAuditSupport {
    fn declaration(&self, provider_module_id: ModuleId) -> ModuleDeclaration {
        ModuleDeclaration::new(provider_module_id)
    }

    fn materialize(
        &self,
        _attachment: &ResourceAugmentation<KeyValue, KeyValueAudit>,
        provider_module_id: ModuleId,
    ) -> Option<Box<dyn ModuleRuntime>> {
        Some(Box::new(KeyValueAuditRuntime {
            module_id: provider_module_id,
        }))
    }
}

struct KeyValueAuditRuntime {
    module_id: ModuleId,
}

impl ModuleRuntime for KeyValueAuditRuntime {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(vec![ModuleContract::new(
            &KeyValueAudit::contract_key(),
            Arc::new(KeyValueAuditContract),
        )])
    }

    fn bind(&mut self, _bindings: &ModuleBindings) -> Result<(), ModuleError> {
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn stop(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn health(&self) -> Health {
        Health::Healthy
    }
}

/// Contribute one in-memory KeyValue occurrence.
pub fn memory_key_value(name: &'static str) -> impl IntoFabricContribution {
    let selected = KeyValue::select(name).expect("valid KeyValue resource name");
    FabricContribution::new().resource(
        selected
            .using(MemoryKeyValue::new())
            .expect("MemoryKeyValue supports KeyValue"),
    )
}

/// Contribute one in-memory KeyValue occurrence with the audit augmentation.
pub fn audited_memory_key_value(name: &'static str) -> impl IntoFabricContribution {
    let selected = KeyValue::select(name).expect("valid KeyValue resource name");
    let audit = ResourceAugmentation::<KeyValue, KeyValueAudit>::attach(&selected, ())
        .expect("valid KeyValue audit attachment")
        .using(KeyValueAuditSupport);

    FabricContribution::new()
        .resource(
            selected
                .using(MemoryKeyValue::new())
                .expect("MemoryKeyValue supports KeyValue"),
        )
        .resource_augmentation(audit)
}

/// Contribute two isolated in-memory stores.
pub fn memory_key_value_pair(
    primary: &'static str,
    secondary: &'static str,
) -> impl IntoFabricContribution {
    FabricContribution::new()
        .with(memory_key_value(primary))
        .with(memory_key_value(secondary))
}

pub fn key_value_client(store_name: &'static str) -> impl IntoFabricContribution {
    let store = KeyValue::select(store_name).expect("valid KeyValue resource name");
    let component = KeyValueClient::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("store").expect("role"),
            fabric::authoring::Requires::<KeyValue>::provisional(),
        ),
        &store,
    );

    FabricContribution::new().component(component)
}

pub fn dual_key_value_client(
    primary_name: &'static str,
    cache_name: &'static str,
) -> impl IntoFabricContribution {
    let primary = KeyValue::select(primary_name).expect("valid primary KeyValue resource name");
    let cache = KeyValue::select(cache_name).expect("valid cache KeyValue resource name");
    let component = DualKeyValueClient::define()
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("primary").expect("role"),
                fabric::authoring::Requires::<KeyValue>::provisional(),
            ),
            &primary,
        )
        .select_named_resource_provider(
            &fabric::authoring::ComponentResourceRequirement::new(
                fabric::component::ComponentRelationName::new("cache").expect("role"),
                fabric::authoring::Requires::<KeyValue>::provisional(),
            ),
            &cache,
        );

    FabricContribution::new().component(component)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn package_boundary_disappears_from_composition_truth() {
        let composition = Fabric::new("onoal.package.test.key-value.inspect")
            .expect("fabric")
            .with(audited_memory_key_value("primary"))
            .build()
            .expect("composition");

        let resources = composition.resources().collect::<Vec<_>>();
        assert_eq!(resources.len(), 1);
        assert_eq!(
            resources[0].resource_id().as_str(),
            "onoal.package.data.key-value"
        );
        assert_eq!(resources[0].name().as_str(), "primary");
        assert_eq!(
            resources[0]
                .realization()
                .adapter_definition_id()
                .expect("adapter")
                .as_str(),
            "onoal.package.data.key-value.memory"
        );
        assert_eq!(resources[0].augmentations().count(), 1);
    }

    #[test]
    fn memory_adapter_keeps_occurrence_state_isolated() {
        let composition = Fabric::new("onoal.package.test.key-value.state")
            .expect("fabric")
            .with(memory_key_value_pair("primary", "cache"))
            .with(key_value_client("primary"))
            .with(dual_key_value_client("primary", "cache"))
            .build()
            .expect("composition");
        let mut instance = composition
            .materialize_on(
                "onoal.package.test.key-value.state.instance",
                &HostDescriptor::native(),
            )
            .expect("instance");
        instance.start().expect("start");

        let client = instance.component::<KeyValueClient>().expect("client");
        client.reconcile().expect("component reconcile");

        let first = block_on(client.write_read_delete("shared".to_owned(), b"primary".to_vec()))
            .expect("first round trip");
        assert_eq!(first.stored, Some(b"primary".to_vec()));
        assert_eq!(first.deleted, Some(b"primary".to_vec()));
        assert_eq!(first.after_delete, None);

        let second =
            block_on(client.write_read_delete("shared".to_owned(), b"primary-again".to_vec()))
                .expect("second round trip");
        assert_eq!(second.stored, Some(b"primary-again".to_vec()));

        let dual = instance
            .component::<DualKeyValueClient>()
            .expect("dual client");
        dual.reconcile().expect("dual reconcile");
        let isolated = block_on(dual.write_both()).expect("dual round trip");
        assert_eq!(isolated.primary, Some(b"primary".to_vec()));
        assert_eq!(isolated.cache, Some(b"cache".to_vec()));
    }
}
