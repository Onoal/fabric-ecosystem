//! KeyValue quickstart example for Fabric Ecosystem.

use fabric::prelude::*;
use fabric_package_key_value::{memory_key_value, KeyValue, KeyValueError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickstartResult {
    pub stored: Option<Vec<u8>>,
    pub deleted: Option<Vec<u8>>,
    pub after_delete: Option<Vec<u8>>,
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

pub fn run() -> Result<QuickstartResult, Box<dyn std::error::Error>> {
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
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quickstart_builds_materializes_invokes_and_stops() {
        let result = run().expect("quickstart");
        assert_eq!(result.stored, Some(b"fabric".to_vec()));
        assert_eq!(result.deleted, Some(b"fabric".to_vec()));
        assert_eq!(result.after_delete, None);
    }
}
