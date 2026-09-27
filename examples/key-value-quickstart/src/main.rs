use fabric::prelude::*;
use fabric_package_key_value::{memory_key_value, KeyValue, KeyValueError};

#[derive(Clone, Debug, PartialEq, Eq)]
struct QuickstartResult {
    stored: Option<Vec<u8>>,
    deleted: Option<Vec<u8>>,
    after_delete: Option<Vec<u8>>,
}

fabric::component! {
    QuickstartApp {
        id: "fabric.ecosystem.example.key-value.app";

        relations {
            requires {
                store: KeyValue;
            }
        }

        api {
            fn write_read_delete(&self, key: String, value: Vec<u8>) -> Result<QuickstartResult, KeyValueError>;
        }

        runtime {
            fn write_read_delete(&self, key: String, value: Vec<u8>) -> Result<QuickstartResult, KeyValueError> {
                self.relations().store.set(key.clone(), value)?;
                let stored = self.relations().store.get(key.clone())?;
                let deleted = self.relations().store.delete(key.clone())?;
                let after_delete = self.relations().store.get(key)?;
                Ok(QuickstartResult {
                    stored,
                    deleted,
                    after_delete,
                })
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let composition = Fabric::new("fabric.ecosystem.example.key-value")?
        .with(memory_key_value("primary"))
        .with(quickstart_app("primary"))
        .build()?;

    let mut instance = composition.materialize_on(
        "fabric.ecosystem.example.key-value.local",
        &HostDescriptor::native(),
    )?;
    instance.start()?;

    let app = instance.component::<QuickstartApp>()?;
    app.reconcile()?;
    let result = futures::executor::block_on(
        app.write_read_delete("hello".to_owned(), b"fabric".to_vec()),
    )??;

    instance.stop()?;

    println!(
        "stored: {}",
        display_optional_bytes(result.stored.as_deref())
    );
    println!(
        "deleted: {}",
        display_optional_bytes(result.deleted.as_deref())
    );
    println!(
        "after delete: {}",
        display_optional_bytes(result.after_delete.as_deref())
    );

    Ok(())
}

fn quickstart_app(store_name: &'static str) -> impl IntoFabricContribution {
    let store = KeyValue::select(store_name).expect("valid KeyValue resource name");
    let component = QuickstartApp::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("store").expect("role"),
            fabric::authoring::Requires::<KeyValue>::provisional(),
        ),
        &store,
    );
    FabricContribution::new().component(component)
}

fn display_optional_bytes(value: Option<&[u8]>) -> String {
    match value {
        Some(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        None => "missing".to_owned(),
    }
}
