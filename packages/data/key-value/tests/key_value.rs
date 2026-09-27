use fabric::prelude::*;
use fabric_package_key_value::{memory_key_value, KeyValue, KeyValueError};
use futures::executor::block_on;

#[derive(Clone, Debug, PartialEq, Eq)]
struct StoreExercise {
    first_read: Option<Vec<u8>>,
    replaced: Option<Vec<u8>>,
    deleted: Option<Vec<u8>>,
    after_delete: Option<Vec<u8>>,
    missing_delete: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DualStoreExercise {
    primary: Option<Vec<u8>>,
    cache: Option<Vec<u8>>,
}

fabric::component! {
    TestStoreUser {
        id: "onoal.package.data.key-value.test-store-user";

        relations {
            requires {
                store: KeyValue;
            }
        }

        api {
            fn exercise(&self, key: String, first: Vec<u8>, second: Vec<u8>) -> Result<StoreExercise, KeyValueError>;
            fn read(&self, key: String) -> Result<Option<Vec<u8>>, KeyValueError>;
        }

        runtime {
            fn exercise(&self, key: String, first: Vec<u8>, second: Vec<u8>) -> Result<StoreExercise, KeyValueError> {
                self.relations().store.set(key.clone(), first)?;
                let first_read = self.relations().store.get(key.clone())?;
                self.relations().store.set(key.clone(), second)?;
                let replaced = self.relations().store.get(key.clone())?;
                let deleted = self.relations().store.delete(key.clone())?;
                let after_delete = self.relations().store.get(key.clone())?;
                let missing_delete = self.relations().store.delete(key)?;
                Ok(StoreExercise {
                    first_read,
                    replaced,
                    deleted,
                    after_delete,
                    missing_delete,
                })
            }

            fn read(&self, key: String) -> Result<Option<Vec<u8>>, KeyValueError> {
                self.relations().store.get(key)
            }
        }
    }
}

fabric::component! {
    TestDualStoreUser {
        id: "onoal.package.data.key-value.test-dual-store-user";

        relations {
            requires {
                primary: KeyValue;
                cache: KeyValue;
            }
        }

        api {
            fn write_both(&self) -> Result<DualStoreExercise, KeyValueError>;
        }

        runtime {
            fn write_both(&self) -> Result<DualStoreExercise, KeyValueError> {
                self.relations()
                    .primary
                    .set("shared".to_owned(), b"primary".to_vec())?;
                self.relations()
                    .cache
                    .set("shared".to_owned(), b"cache".to_vec())?;
                Ok(DualStoreExercise {
                    primary: self.relations().primary.get("shared".to_owned())?,
                    cache: self.relations().cache.get("shared".to_owned())?,
                })
            }
        }
    }
}

fn bind_store_user(store_name: &'static str) -> impl IntoFabricContribution {
    let store = KeyValue::select(store_name).expect("store selection");
    let component = TestStoreUser::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("store").expect("role"),
            fabric::authoring::Requires::<KeyValue>::provisional(),
        ),
        &store,
    );
    FabricContribution::new().component(component)
}

fn bind_dual_store_user(
    primary_name: &'static str,
    cache_name: &'static str,
) -> impl IntoFabricContribution {
    let primary = KeyValue::select(primary_name).expect("primary selection");
    let cache = KeyValue::select(cache_name).expect("cache selection");
    let component = TestDualStoreUser::define()
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

#[test]
fn package_boundary_disappears_from_composition_truth() {
    let composition = Fabric::new("onoal.package.test.key-value.inspect")
        .expect("fabric")
        .with(memory_key_value("primary"))
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
    assert_eq!(resources[0].augmentations().count(), 0);
}

#[test]
fn memory_store_supports_set_get_replace_delete_and_missing_keys() {
    let composition = Fabric::new("onoal.package.test.key-value.operations")
        .expect("fabric")
        .with(memory_key_value("primary"))
        .with(bind_store_user("primary"))
        .build()
        .expect("composition");
    let mut instance = composition
        .materialize_on(
            "onoal.package.test.key-value.operations.instance",
            &HostDescriptor::native(),
        )
        .expect("instance");
    instance.start().expect("start");

    let user = instance.component::<TestStoreUser>().expect("store user");
    user.reconcile().expect("component reconcile");
    let result = block_on(user.exercise(
        "hello".to_owned(),
        b"fabric".to_vec(),
        b"ecosystem".to_vec(),
    ))
    .expect("component call")
    .expect("key-value operation");

    assert_eq!(result.first_read, Some(b"fabric".to_vec()));
    assert_eq!(result.replaced, Some(b"ecosystem".to_vec()));
    assert_eq!(result.deleted, Some(b"ecosystem".to_vec()));
    assert_eq!(result.after_delete, None);
    assert_eq!(result.missing_delete, None);

    instance.stop().expect("stop");
}

#[test]
fn multiple_named_stores_remain_isolated() {
    let composition = Fabric::new("onoal.package.test.key-value.isolation")
        .expect("fabric")
        .with(memory_key_value("primary"))
        .with(memory_key_value("cache"))
        .with(bind_dual_store_user("primary", "cache"))
        .build()
        .expect("composition");
    let mut instance = composition
        .materialize_on(
            "onoal.package.test.key-value.isolation.instance",
            &HostDescriptor::native(),
        )
        .expect("instance");
    instance.start().expect("start");

    let user = instance
        .component::<TestDualStoreUser>()
        .expect("dual store user");
    user.reconcile().expect("component reconcile");
    let result = block_on(user.write_both())
        .expect("component call")
        .expect("key-value operation");

    assert_eq!(result.primary, Some(b"primary".to_vec()));
    assert_eq!(result.cache, Some(b"cache".to_vec()));

    instance.stop().expect("stop");
}

#[test]
fn fresh_generation_starts_with_empty_memory_state() {
    let composition = Fabric::new("onoal.package.test.key-value.generation")
        .expect("fabric")
        .with(memory_key_value("primary"))
        .with(bind_store_user("primary"))
        .build()
        .expect("composition");

    let mut first = composition
        .materialize_on(
            "onoal.package.test.key-value.generation.first",
            &HostDescriptor::native(),
        )
        .expect("first instance");
    first.start().expect("start first");
    let first_user = first.component::<TestStoreUser>().expect("first user");
    first_user.reconcile().expect("first reconcile");
    let _ =
        block_on(first_user.exercise("shared".to_owned(), b"first".to_vec(), b"stored".to_vec()))
            .expect("first component call")
            .expect("first operations");
    first.stop().expect("stop first");

    let mut second = composition
        .materialize_on(
            "onoal.package.test.key-value.generation.second",
            &HostDescriptor::native(),
        )
        .expect("second instance");
    second.start().expect("start second");
    let second_user = second.component::<TestStoreUser>().expect("second user");
    second_user.reconcile().expect("second reconcile");
    let value = block_on(second_user.read("shared".to_owned()))
        .expect("second component call")
        .expect("second read");
    assert_eq!(value, None);
    second.stop().expect("stop second");
}
