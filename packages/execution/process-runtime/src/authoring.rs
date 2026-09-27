use fabric::prelude::*;

use crate::{LocalProcessRuntime, ProcessRuntime};

pub fn local_process_runtime(name: &'static str) -> impl IntoFabricContribution {
    let runtime = ProcessRuntime::select(name).expect("valid ProcessRuntime resource name");
    FabricContribution::new().resource(
        runtime
            .using(LocalProcessRuntime::new())
            .expect("LocalProcessRuntime supports ProcessRuntime"),
    )
}
