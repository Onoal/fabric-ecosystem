#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use fabric_adapter_key_value_filesystem::{
        FileLayout, FilesystemKeyValueAdapter, FilesystemKeyValueAdapterConfig,
        encoded_path_for_test,
    };
    use fabric_adapter_key_value_memory::memory_adapter;
    use fabric_resource_key_value::{KeyValueError, KeyValueStore, KeyValueStoreConfig};
    use fabric_sdk::prelude::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ProbeInput;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ProbeOutput {
        missing_before: Result<Option<Vec<u8>>, KeyValueError>,
        put_one: Result<(), KeyValueError>,
        first_value: Result<Option<Vec<u8>>, KeyValueError>,
        put_two: Result<(), KeyValueError>,
        overwritten_value: Result<Option<Vec<u8>>, KeyValueError>,
        delete: Result<(), KeyValueError>,
        missing_after: Result<Option<Vec<u8>>, KeyValueError>,
        invalid_key: Result<Option<Vec<u8>>, KeyValueError>,
    }

    fabric_sdk::component! {
        KeyValueProbe {
            id: "fabric.package.key-value.probe";
            config {}
            requires { storage: KeyValueStore(version = "^1"); }
            operations {
                exercise {
                    id: "fabric.package.key-value.probe.exercise";
                    input: ProbeInput = "fabric.package.key-value.probe.exercise.input";
                    output: ProbeOutput = "fabric.package.key-value.probe.exercise.output";
                    handler |dependencies, input: ProbeInput| async move {
                        let _ = input;
                        let storage = dependencies.storage;
                        Ok(ProbeOutput {
                            missing_before: storage.get(b"alpha".to_vec()),
                            put_one: storage.put(b"alpha".to_vec(), b"one".to_vec()),
                            first_value: storage.get(b"alpha".to_vec()),
                            put_two: storage.put(b"alpha".to_vec(), b"two".to_vec()),
                            overwritten_value: storage.get(b"alpha".to_vec()),
                            delete: storage.delete(b"alpha".to_vec()),
                            missing_after: storage.get(b"alpha".to_vec()),
                            invalid_key: storage.get(Vec::new()),
                        })
                    };
                }
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct DualStoreInput;
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct DualStoreOutput {
        primary: Result<Option<Vec<u8>>, KeyValueError>,
        cache: Result<Option<Vec<u8>>, KeyValueError>,
    }

    fabric_sdk::component! {
        DualStoreProbe {
            id: "fabric.package.key-value.dual-store-probe";
            config {}
            requires {
                primary_store: KeyValueStore(version = "^1");
                cache_store: KeyValueStore(version = "^1");
            }
            operations {
                exercise {
                    id: "fabric.package.key-value.dual-store-probe.exercise";
                    input: DualStoreInput = "fabric.package.key-value.dual-store-probe.exercise.input";
                    output: DualStoreOutput = "fabric.package.key-value.dual-store-probe.exercise.output";
                    handler |dependencies, input: DualStoreInput| async move {
                        let _ = input;
                        let _ = dependencies.primary_store.put(b"role".to_vec(), b"primary-value".to_vec());
                        let _ = dependencies.cache_store.put(b"role".to_vec(), b"cache-value".to_vec());
                        Ok(DualStoreOutput {
                            primary: dependencies.primary_store.get(b"role".to_vec()),
                            cache: dependencies.cache_store.get(b"role".to_vec()),
                        })
                    };
                }
            }
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

    fn host() -> HostDescriptor {
        HostDescriptor::new(
            HostOperatingSystem::new("linux").expect("os"),
            HostArchitecture::new("x86_64").expect("architecture"),
        )
    }

    fn run_with<A>(adapter: A) -> (FabricManifest, ProbeOutput)
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
        let provider = KeyValueStore::select("primary", KeyValueStoreConfig {}).expect("provider");
        let built = Fabric::new("key-value.package")
            .expect("fabric")
            .resource(selection)
            .component(
                KeyValueProbe::define(KeyValueProbeConfig {}).select_named_resource_provider(
                    &key_value_probe::requirements::storage(),
                    &provider,
                ),
            )
            .build()
            .expect("build");
        let manifest = built.manifest().clone();
        assert_eq!(manifest.composition_exports().len(), 1);
        let mut instance = built
            .materialize_named_on("key-value.instance", &host())
            .expect("materialize");
        instance.start().expect("start");
        let components = instance.components().expect("component host");
        components
            .materialize::<KeyValueProbe>()
            .expect("component materialization");
        let output = futures::executor::block_on(
            components.invoke_external(&key_value_probe::operations::exercise(), ProbeInput),
        )
        .expect("typed operation invocation");
        components
            .dematerialize::<KeyValueProbe>()
            .expect("component dematerialization");
        instance.stop();
        (manifest, output)
    }

    #[test]
    fn memory_and_filesystem_adapters_are_public_realizations() {
        let root = temporary_root("filesystem");
        let (memory_manifest, memory) = run_with(memory_adapter());
        let (filesystem_manifest, filesystem) = run_with(FilesystemKeyValueAdapter::new(
            FilesystemKeyValueAdapterConfig {
                root: root.clone(),
                layout: FileLayout::Sharded { depth: 1 },
                fsync: true,
            },
        ));
        let expected = ProbeOutput {
            missing_before: Ok(None),
            put_one: Ok(()),
            first_value: Ok(Some(b"one".to_vec())),
            put_two: Ok(()),
            overwritten_value: Ok(Some(b"two".to_vec())),
            delete: Ok(()),
            missing_after: Ok(None),
            invalid_key: Err(KeyValueError::InvalidKey),
        };
        assert_eq!(memory, expected);
        assert_eq!(filesystem, expected);
        assert_eq!(
            memory_manifest.resources()[0].resource_id(),
            filesystem_manifest.resources()[0].resource_id()
        );
        assert_eq!(
            memory_manifest.resources()[0].name(),
            filesystem_manifest.resources()[0].name()
        );
        assert_eq!(
            memory_manifest.components()[0].component_id(),
            filesystem_manifest.components()[0].component_id()
        );
        assert_eq!(
            memory_manifest.components()[0].resource_requirements(),
            filesystem_manifest.components()[0].resource_requirements()
        );
        let memory_selection = &memory_manifest.component_resource_provider_selections()[0];
        let filesystem_selection = &filesystem_manifest.component_resource_provider_selections()[0];
        assert_eq!(
            memory_selection.component_id(),
            filesystem_selection.component_id()
        );
        assert_eq!(
            memory_selection.requirement_name(),
            filesystem_selection.requirement_name()
        );
        assert_eq!(
            memory_selection.resource_id(),
            filesystem_selection.resource_id()
        );
        assert_eq!(
            memory_selection.contract_id(),
            filesystem_selection.contract_id()
        );
        assert_eq!(memory_selection.provider(), filesystem_selection.provider());
        assert_eq!(
            memory_manifest.provider_selections(),
            filesystem_manifest.provider_selections(),
            "the public Manifest preserves the stable realization slot rather than exposing concrete Adapter Rust types",
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

    fn run_dual_store(swap: bool) -> DualStoreOutput {
        let primary = KeyValueStore::select("primary", KeyValueStoreConfig {}).expect("primary");
        let cache = KeyValueStore::select("cache", KeyValueStoreConfig {}).expect("cache");
        let primary_provider = primary.clone();
        let cache_provider = cache.clone();
        let (primary_provider, cache_provider) = if swap {
            (&cache_provider, &primary_provider)
        } else {
            (&primary_provider, &cache_provider)
        };
        let built = Fabric::new("key-value.occurrences")
            .expect("fabric")
            .resource(primary.using(memory_adapter()).expect("primary adapter"))
            .resource(cache.using(memory_adapter()).expect("cache adapter"))
            .component(
                DualStoreProbe::define(DualStoreProbeConfig {})
                    .select_named_resource_provider(
                        &dual_store_probe::requirements::primary_store(),
                        primary_provider,
                    )
                    .select_named_resource_provider(
                        &dual_store_probe::requirements::cache_store(),
                        cache_provider,
                    ),
            )
            .build()
            .expect("build");
        assert_eq!(built.manifest().resources().len(), 2);
        assert_eq!(
            built.manifest().components()[0]
                .resource_requirements()
                .len(),
            2
        );
        let selections = built.manifest().component_resource_provider_selections();
        let selected_primary = selections
            .iter()
            .find(|selection| selection.requirement_name().as_str() == "primary_store")
            .expect("primary role selection");
        let selected_cache = selections
            .iter()
            .find(|selection| selection.requirement_name().as_str() == "cache_store")
            .expect("cache role selection");
        assert_eq!(selected_primary.provider(), primary_provider.module_id());
        assert_eq!(selected_cache.provider(), cache_provider.module_id());
        let mut instance = built
            .materialize_named_on("key-value.occurrences.instance", &host())
            .expect("materialize");
        instance.start().expect("start");
        let components = instance.components().expect("component host");
        components
            .materialize::<DualStoreProbe>()
            .expect("component");
        let output = futures::executor::block_on(
            components.invoke_external(&dual_store_probe::operations::exercise(), DualStoreInput),
        )
        .expect("invoke");
        instance.stop();
        output
    }

    #[test]
    fn external_same_target_resource_roles_bind_independently_and_swap() {
        assert_eq!(
            run_dual_store(false),
            DualStoreOutput {
                primary: Ok(Some(b"primary-value".to_vec())),
                cache: Ok(Some(b"cache-value".to_vec())),
            }
        );
        assert_eq!(
            run_dual_store(true),
            DualStoreOutput {
                primary: Ok(Some(b"primary-value".to_vec())),
                cache: Ok(Some(b"cache-value".to_vec())),
            }
        );
    }

    #[test]
    fn resource_only_fabric_has_no_component_operator() {
        let built = Fabric::new("key-value.resource-only")
            .expect("fabric")
            .resource(
                KeyValueStore::select("primary", KeyValueStoreConfig {})
                    .expect("selection")
                    .using(memory_adapter())
                    .expect("adapter"),
            )
            .build()
            .expect("build");
        let instance = built
            .materialize_named_on("key-value.resource-only.instance", &host())
            .expect("materialize");
        assert!(instance.components().is_none());
    }
}
