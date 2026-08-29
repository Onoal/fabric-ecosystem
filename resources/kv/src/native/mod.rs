mod adapter;
mod module;

pub use adapter::KvAdapter;
pub use module::NativeKv;

use fabric_core::{Health, ModuleError};

fn start_noop() -> Result<(), ModuleError> {
    Ok(())
}

fn healthy() -> Health {
    Health::Healthy
}
