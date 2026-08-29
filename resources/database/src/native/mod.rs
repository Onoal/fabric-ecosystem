mod adapter;
mod database;

pub use adapter::DatabaseAdapter;
pub use database::{DatabaseAdapterSupport, NativeDatabase};

use fabric_core::{ModuleBindings, ModuleError};

fn bind_noop(_bindings: &ModuleBindings) -> Result<(), ModuleError> {
    Ok(())
}

fn start_noop() -> Result<(), ModuleError> {
    Ok(())
}
