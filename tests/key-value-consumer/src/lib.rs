#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use fabric_adapter_key_value_filesystem::{
        FileLayout, FilesystemKeyValueAdapter, FilesystemKeyValueAdapterConfig,
        encoded_path_for_test,
    };
    use fabric_adapter_key_value_memory::memory_adapter;
    use fabric_resource_key_value::{KeyValueError, KeyValueStore, KeyValueStoreConfig};
    use fabric_sdk::prelude::*;

    type Observation = Result<Option<Vec<u8>>, KeyValueError>;

    struct KeyValueProbe;
    #[derive(Clone)]
    struct ProbeConfig {
        capture: Arc<Mutex<Vec<Observation>>>,
    }
    impl ComponentDefinition for KeyValueProbe {
        type Config = ProbeConfig;
        fn component_id() -> ComponentId {
            ComponentId::new("fabric.package.key-value.probe").expect("id")
        }
        fn declaration() -> fabric_component::ComponentDeclaration {
            fabric_component::ComponentDeclaration::new(Self::component_id(), Vec::new())
        }
        fn runtime_attachment(
            config: &Self::Config,
        ) -> Option<fabric_component::ComponentRuntimeDefinition> {
            let capture = Arc::clone(&config.capture);
            Some(fabric_component::ComponentRuntimeDefinition::new(
                Self::component_id(),
                move |scope| {
                    let store = scope.resource(&Requires::<KeyValueStore>::versioned(
                        ContractVersionRequirement::parse("^1").expect("requirement"),
                    ))?;
                    store
                        .put(b"alpha".to_vec(), b"one".to_vec())
                        .map_err(|_| fabric_component::ComponentError::Unavailable)?;
                    capture
                        .lock()
                        .expect("capture")
                        .push(store.get(b"alpha".to_vec()));
                    Ok(Health::Healthy)
                },
            ))
        }
    }

    #[derive(Clone)]
    struct MaterializerCapture {
        id: ModuleId,
        value: Arc<Mutex<Option<Arc<fabric_component::ComponentMaterializer>>>>,
    }
    impl ModuleRuntime for MaterializerCapture {
        fn id(&self) -> &ModuleId {
            &self.id
        }
        fn required_contract_declarations(&self) -> Vec<ContractRequirementDeclaration> {
            vec![
                ContractRequirement::<fabric_component::ComponentMaterializer>::provisional(
                    fabric_component::component_materializer_contract_id(),
                )
                .declaration()
                .clone(),
            ]
        }
        fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
            Ok(Vec::new())
        }
        fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
            let requirement =
                ContractRequirement::<fabric_component::ComponentMaterializer>::provisional(
                    fabric_component::component_materializer_contract_id(),
                );
            *self.value.lock().expect("materializer") = Some(
                bindings
                    .resolve(&requirement)
                    .map_err(|error| ModuleError::new(error.to_string()))?,
            );
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

    fn temporary_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "fabric-key-value-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn run_with<A>(
        adapter: A,
    ) -> (
        fabric_sdk::FabricManifest,
        Result<Option<Vec<u8>>, KeyValueError>,
    )
    where
        A: AdapterDefinition<
                Target = KeyValueStore,
                SchemaSupport = fabric_sdk::resource::AdapterResourceSchemaSupport,
            >,
    {
        let selection = KeyValueStore::select("primary", KeyValueStoreConfig {})
            .expect("selection")
            .using(adapter)
            .expect("adapter");
        let capture = Arc::new(Mutex::new(Vec::new()));
        let materializer = Arc::new(Mutex::new(None));
        let built = Fabric::new("key-value.package")
            .expect("fabric")
            .resource(selection)
            .component(
                KeyValueProbe::define(ProbeConfig {
                    capture: Arc::clone(&capture),
                })
                .requires_resource(Requires::<KeyValueStore>::versioned(
                    ContractVersionRequirement::parse("^1").expect("requirement"),
                ))
                .select_resource_provider(
                    &KeyValueStore::select("primary", KeyValueStoreConfig {}).expect("provider"),
                ),
            )
            .block("capture", |block| {
                block.module(MaterializerCapture {
                    id: ModuleId::new("fabric.package.key-value.capture").expect("module"),
                    value: Arc::clone(&materializer),
                })
            })
            .expect("capture block")
            .build()
            .expect("build");
        let host = HostDescriptor::new(
            HostOperatingSystem::new("linux").expect("os"),
            HostArchitecture::new("x86_64").expect("architecture"),
        );
        let mut instance = built
            .composition()
            .materialize_named_on("key-value.instance", &host)
            .expect("materialize");
        instance.start().expect("start");
        materializer
            .lock()
            .expect("materializer")
            .as_ref()
            .expect("bound materializer")
            .materialize(&KeyValueProbe::component_id())
            .expect("component");
        let result = capture
            .lock()
            .expect("capture")
            .pop()
            .expect("component observation");
        instance.stop();
        (built.manifest().clone(), result)
    }

    #[test]
    fn memory_and_filesystem_adapters_are_public_realizations() {
        let root = temporary_root("filesystem");
        let (_, memory) = run_with(memory_adapter());
        assert_eq!(memory.expect("memory result"), Some(b"one".to_vec()));
        let (_, filesystem) = run_with(FilesystemKeyValueAdapter::new(
            FilesystemKeyValueAdapterConfig {
                root: root.clone(),
                layout: FileLayout::Sharded { depth: 1 },
                fsync: true,
            },
        ));
        assert_eq!(
            filesystem.expect("filesystem result"),
            Some(b"one".to_vec())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn filesystem_keys_are_encoded_beneath_the_configured_root() {
        let root = temporary_root("safety");
        let path =
            encoded_path_for_test(&root, &FileLayout::Sharded { depth: 2 }, b"../../outside")
                .expect("path");
        assert!(path.starts_with(&root));
        assert_ne!(path, root.join("../../outside"));
    }

    #[test]
    fn named_key_value_occurrences_coexist() {
        let primary = KeyValueStore::select("primary", KeyValueStoreConfig {}).expect("primary");
        let cache = KeyValueStore::select("cache", KeyValueStoreConfig {}).expect("cache");
        assert_ne!(primary.module_id(), cache.module_id());
    }
}
