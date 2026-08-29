use std::sync::Arc;

use fabric_core::ModuleError;
use fabric_resource::ResourceError;

use crate::contract::KvService;

pub trait KvAdapter: Send + Sync {
    fn service(&self) -> Arc<dyn KvService>;

    fn initialize(&self) -> Result<(), ModuleError>;

    fn start(&self) -> Result<(), ResourceError>;

    fn stop(&self);
}
