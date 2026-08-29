use fabric_core::{Health, ModuleError};
use fabric_resource_registry::{ResourceInspection, ResourceRegistryError};

use crate::{DatabaseConfig, DatabaseService};

pub trait DatabaseAdapter: DatabaseService + Send + Sync {
    fn initialize(&self, config: &DatabaseConfig) -> Result<(), ModuleError>;

    fn consume_configuration(&self, config: &DatabaseConfig) -> Result<(), ResourceRegistryError>;

    fn inspect(&self) -> Result<ResourceInspection, ResourceRegistryError>;

    fn health(&self) -> Health;
}
