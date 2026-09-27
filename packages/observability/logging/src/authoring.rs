use fabric::prelude::*;

use crate::{ConsoleLogConfig, ConsoleLogSink, ConsoleLogSinkConfig, LogSink};

pub fn console_logging(name: &'static str) -> impl IntoFabricContribution {
    console_logging_with(name, ConsoleLogConfig::default())
}

pub fn console_logging_with(
    name: &'static str,
    config: ConsoleLogConfig,
) -> impl IntoFabricContribution {
    let selected = LogSink::select(name).expect("valid LogSink resource name");
    FabricContribution::new().resource(
        selected
            .using(ConsoleLogSink::new(ConsoleLogSinkConfig { config }))
            .expect("ConsoleLogSink supports LogSink"),
    )
}
