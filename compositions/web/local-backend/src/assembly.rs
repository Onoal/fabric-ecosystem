use fabric::prelude::*;
use fabric_composition_http_server::http_server_stack;

use crate::LocalBackendCompositionConfig;

pub fn local_backend_stack(config: LocalBackendCompositionConfig) -> impl IntoFabricContribution {
    FabricContribution::new()
        .with(http_server_stack(config.http))
        .with(fabric_package_sqlite::sqlite_database_with(
            config.database_name,
            config.database,
        ))
        .with(fabric_package_observability_logging::console_logging_with(
            config.log_name,
            config.logging,
        ))
}
