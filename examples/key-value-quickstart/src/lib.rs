//! KeyValue quickstart example for Fabric Ecosystem.

use fabric::prelude::*;
use fabric_package_key_value::{
    key_value_client, memory_key_value, KeyValueClientInstanceApi, KeyValueRoundTrip,
};

pub fn run() -> Result<KeyValueRoundTrip, Box<dyn std::error::Error>> {
    let composition = Fabric::new("fabric.ecosystem.example.key-value")?
        .with(memory_key_value("primary"))
        .with(key_value_client("primary"))
        .build()?;

    let mut instance = composition.materialize_on(
        "fabric.ecosystem.example.key-value.local",
        &HostDescriptor::native(),
    )?;
    instance.start()?;

    let client = instance.component::<fabric_package_key_value::KeyValueClient>()?;
    client.reconcile()?;
    let result = futures::executor::block_on(
        client.write_read_delete("hello".to_owned(), b"fabric".to_vec()),
    )?;

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
