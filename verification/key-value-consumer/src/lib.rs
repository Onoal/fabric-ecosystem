//! Executable substitution and occurrence-isolation proof for KeyValue.

#[cfg(test)]
mod witness {
    use fabric::*;
    use onoal_fabric_resource_key_value::{KeyValueError, KeyValueStore, KeyValueStoreConfig};

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ProbeOutput {
        missing_before: Result<Option<Vec<u8>>, KeyValueError>,
        first_value: Result<Option<Vec<u8>>, KeyValueError>,
        overwritten_value: Result<Option<Vec<u8>>, KeyValueError>,
        missing_after: Result<Option<Vec<u8>>, KeyValueError>,
        invalid_key: Result<Option<Vec<u8>>, KeyValueError>,
    }

    fabric::component! {
        KeyValueProbe {
            id: "onoal.package.key-value.probe";
            config {}
            requires {
                storage: KeyValueStore(provisional);
            }
            operations {
                exercise {
                    id: "onoal.package.key-value.probe.exercise";
                    input: () = "onoal.package.key-value.probe.exercise.input";
                    output: ProbeOutput = "onoal.package.key-value.probe.exercise.output";
                    handler |dependencies, _input: ()| async move {
                        let storage = dependencies.storage;
                        let missing_before = storage.get(b"alpha".to_vec());
                        let _ = storage.put(b"alpha".to_vec(), b"one".to_vec());
                        let first_value = storage.get(b"alpha".to_vec());
                        let _ = storage.put(b"alpha".to_vec(), b"two".to_vec());
                        let overwritten_value = storage.get(b"alpha".to_vec());
                        let _ = storage.delete(b"alpha".to_vec());
                        let missing_after = storage.get(b"alpha".to_vec());
                        let invalid_key = storage.get(Vec::new());
                        Ok(ProbeOutput {
                            missing_before,
                            first_value,
                            overwritten_value,
                            missing_after,
                            invalid_key,
                        })
                    };
                }
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct DualStoreOutput {
        primary: Result<Option<Vec<u8>>, KeyValueError>,
        secondary: Result<Option<Vec<u8>>, KeyValueError>,
    }

    fabric::component! {
        DualStoreProbe {
            id: "onoal.package.key-value.dual-store-probe";
            config {}
            requires {
                primary: KeyValueStore(provisional);
                secondary: KeyValueStore(provisional);
            }
            operations {
                exercise {
                    id: "onoal.package.key-value.dual-store-probe.exercise";
                    input: () = "onoal.package.key-value.dual-store-probe.exercise.input";
                    output: DualStoreOutput = "onoal.package.key-value.dual-store-probe.exercise.output";
                    handler |dependencies, _input: ()| async move {
                        let _ = dependencies.primary.put(b"role".to_vec(), b"primary-value".to_vec());
                        let _ = dependencies.secondary.put(b"role".to_vec(), b"secondary-value".to_vec());
                        Ok(DualStoreOutput {
                            primary: dependencies.primary.get(b"role".to_vec()),
                            secondary: dependencies.secondary.get(b"role".to_vec()),
                        })
                    };
                }
            }
        }
    }

    fn run_with<A>(adapter: A) -> (FabricManifest, ProbeOutput)
    where
        A: AdapterDefinition<Target = KeyValueStore, Compatibility = AdapterResourceSchemaSupport>,
    {
        let selection =
            KeyValueStore::select("primary", KeyValueStoreConfig {}).expect("selection");
        let built = Fabric::new("onoal.key-value.substitution")
            .expect("composition")
            .resource(
                selection
                    .clone()
                    .using(adapter)
                    .expect("compatible adapter"),
            )
            .component(
                KeyValueProbe::define(KeyValueProbeConfig {}).select_named_resource_provider(
                    &key_value_probe::requirements::storage(),
                    &selection,
                ),
            )
            .build()
            .expect("composition");
        let manifest = built.manifest().clone();
        let mut instance = built
            .materialize_named_on(
                "onoal.key-value.substitution.instance",
                &HostDescriptor::native(),
            )
            .expect("materialize");
        instance.start().expect("start");
        let components = instance.components().expect("component host");
        components
            .materialize::<KeyValueProbe>()
            .expect("materialize component");
        let output = futures::executor::block_on(
            components.invoke_external(&key_value_probe::operations::exercise(), ()),
        )
        .expect("invoke");
        components
            .dematerialize::<KeyValueProbe>()
            .expect("dematerialize component");
        instance.stop();
        (manifest, output)
    }

    mod tests {
        use super::*;
        use onoal_fabric_adapter_key_value_filesystem::{
            FileLayout, FilesystemKeyValueAdapter, FilesystemKeyValueAdapterConfig,
        };
        use onoal_fabric_adapter_key_value_memory::memory_adapter;

        #[test]
        fn one_semantic_consumer_works_with_memory_and_filesystem_realizations() {
            let (memory_manifest, memory) = run_with(memory_adapter());
            let temporary = tempfile::tempdir().expect("temporary filesystem root");
            let (filesystem_manifest, filesystem) = run_with(FilesystemKeyValueAdapter::new(
                FilesystemKeyValueAdapterConfig {
                    root: temporary.path().to_path_buf(),
                    layout: FileLayout::Sharded { depth: 1 },
                    fsync: true,
                },
            ));
            let expected = ProbeOutput {
                missing_before: Ok(None),
                first_value: Ok(Some(b"one".to_vec())),
                overwritten_value: Ok(Some(b"two".to_vec())),
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
        }

        #[test]
        fn selected_key_value_occurrences_remain_independent() {
            let primary =
                KeyValueStore::select("primary", KeyValueStoreConfig {}).expect("primary");
            let secondary =
                KeyValueStore::select("secondary", KeyValueStoreConfig {}).expect("secondary");
            let built = Fabric::new("onoal.key-value.occurrences")
                .expect("composition")
                .resource(
                    primary
                        .clone()
                        .using(memory_adapter())
                        .expect("primary adapter"),
                )
                .resource(
                    secondary
                        .clone()
                        .using(memory_adapter())
                        .expect("secondary adapter"),
                )
                .component(
                    DualStoreProbe::define(DualStoreProbeConfig {})
                        .select_named_resource_provider(
                            &dual_store_probe::requirements::primary(),
                            &primary,
                        )
                        .select_named_resource_provider(
                            &dual_store_probe::requirements::secondary(),
                            &secondary,
                        ),
                )
                .build()
                .expect("composition");
            assert_eq!(built.manifest().resources().len(), 2);
            let mut instance = built
                .materialize_named_on(
                    "onoal.key-value.occurrences.instance",
                    &HostDescriptor::native(),
                )
                .expect("materialize");
            instance.start().expect("start");
            let components = instance.components().expect("component host");
            components
                .materialize::<DualStoreProbe>()
                .expect("materialize component");
            let output = futures::executor::block_on(
                components.invoke_external(&dual_store_probe::operations::exercise(), ()),
            )
            .expect("invoke");
            instance.stop();
            assert_eq!(
                output,
                DualStoreOutput {
                    primary: Ok(Some(b"primary-value".to_vec())),
                    secondary: Ok(Some(b"secondary-value".to_vec())),
                }
            );
        }
    }
}
