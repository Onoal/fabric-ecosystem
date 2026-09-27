use fabric::prelude::*;

use crate::{KeyValue, MemoryKeyValue};

/// Contribute one generation-local in-memory KeyValue occurrence.
pub fn memory_key_value(name: &'static str) -> impl IntoFabricContribution {
    let selected = KeyValue::select(name).expect("valid KeyValue resource name");
    FabricContribution::new().resource(
        selected
            .using(MemoryKeyValue::new())
            .expect("MemoryKeyValue supports KeyValue"),
    )
}
