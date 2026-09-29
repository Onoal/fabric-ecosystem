mod support;

use fabric::prelude::*;
use fabric_package_key_value::{KeyValue, KeyValueError};
use fabric_package_relational_database::{
    RelationalDatabase, RelationalDatabaseError, RelationalQueryResult, RelationalValue,
};
use futures::executor::block_on;
use support::fabric::{activate, started_instance};
use support::temp_database::TempDatabase;

#[derive(Clone, Debug, PartialEq, Eq)]
struct KeyValueResult {
    stored: Option<Vec<u8>>,
    after_delete: Option<Vec<u8>>,
}

fabric::component! {
    KeyValueConsumer {
        id: "onoal.test.third-party.data.key-value-consumer";

        relations {
            requires {
                store: KeyValue;
            }
        }

        api {
            fn exercise_store(&self) -> Result<KeyValueResult, KeyValueError>;
        }

        runtime {
            fn exercise_store(&self) -> Result<KeyValueResult, KeyValueError> {
                resolve_resource(self.relations().store.set("third-party".to_owned(), b"value".to_vec()))?;
                let stored = resolve_resource(self.relations().store.get("third-party".to_owned()))?;
                resolve_resource(self.relations().store.delete("third-party".to_owned()))?;
                let after_delete = resolve_resource(self.relations().store.get("third-party".to_owned()))?;
                Ok(KeyValueResult { stored, after_delete })
            }
        }
    }
}

fn key_value_consumer(store_name: &'static str) -> impl IntoFabricContribution {
    let store = KeyValue::select(store_name).expect("store selection");
    let component = KeyValueConsumer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("store").expect("role"),
            fabric::authoring::Requires::<KeyValue>::provisional(),
        ),
        &store,
    );
    FabricContribution::new().component(component)
}

fabric::component! {
    RelationalConsumer {
        id: "onoal.test.third-party.data.relational-consumer";

        relations {
            requires {
                database: RelationalDatabase;
            }
        }

        api {
            fn write_and_query(&self) -> Result<RelationalQueryResult, RelationalDatabaseError>;
        }

        runtime {
            fn write_and_query(&self) -> Result<RelationalQueryResult, RelationalDatabaseError> {
                resolve_resource(self.relations().database.execute(
                    "CREATE TABLE IF NOT EXISTS third_party_items (id INTEGER, name TEXT NOT NULL)".to_owned(),
                    vec![],
                ))?;
                resolve_resource(self.relations().database.execute(
                    "INSERT INTO third_party_items (id, name) VALUES (?1, ?2)".to_owned(),
                    vec![
                        RelationalValue::Integer(1),
                        RelationalValue::Text("public-consumer".to_owned()),
                    ],
                ))?;
                resolve_resource(self.relations().database.query(
                    "SELECT id, name FROM third_party_items ORDER BY id".to_owned(),
                    vec![],
                ))
            }
        }
    }
}

fn relational_consumer(database_name: &'static str) -> impl IntoFabricContribution {
    let database = RelationalDatabase::select(database_name).expect("database selection");
    let component = RelationalConsumer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("database").expect("role"),
            fabric::authoring::Requires::<RelationalDatabase>::provisional(),
        ),
        &database,
    );
    FabricContribution::new().component(component)
}

fn resolve_resource<T>(mut future: fabric::resource::ResourceFuture<'_, T>) -> T {
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => {
            panic!("third-party data resource operation unexpectedly yielded")
        }
    }
}

#[test]
fn external_consumer_uses_key_value_directly() {
    let composition = Fabric::new("onoal.test.third-party.data.key-value")
        .expect("fabric")
        .with(fabric_package_key_value::memory_key_value("primary"))
        .with(key_value_consumer("primary"))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.test.third-party.data.key-value.instance",
    );
    let consumer = activate::<KeyValueConsumer>(&instance);

    let result = block_on(consumer.exercise_store())
        .expect("exercise")
        .expect("key-value result");

    assert_eq!(result.stored, Some(b"value".to_vec()));
    assert_eq!(result.after_delete, None);
}

#[test]
fn external_consumer_uses_relational_database_with_sqlite_realization() {
    let database = TempDatabase::new("relational");
    let composition = Fabric::new("onoal.test.third-party.data.relational")
        .expect("fabric")
        .with(fabric_package_sqlite::sqlite_database(
            "primary-db",
            database.path_buf(),
        ))
        .with(relational_consumer("primary-db"))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.test.third-party.data.relational.instance",
    );
    let consumer = activate::<RelationalConsumer>(&instance);

    let rows = block_on(consumer.write_and_query())
        .expect("exercise")
        .expect("relational rows");

    assert_eq!(
        rows.rows()[0].values(),
        &[
            RelationalValue::Integer(1),
            RelationalValue::Text("public-consumer".to_owned()),
        ]
    );
}
