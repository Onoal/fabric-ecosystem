use fabric::prelude::*;

use crate::{CounterMetric, InMemoryCounter};

pub fn in_memory_counter(name: &'static str) -> impl IntoFabricContribution {
    let selected = CounterMetric::select(name).expect("valid CounterMetric resource name");
    FabricContribution::new().resource(
        selected
            .using(InMemoryCounter::new())
            .expect("InMemoryCounter supports CounterMetric"),
    )
}
