use std::sync::Arc;

use fabric_core::ModuleError;
use fabric_resource::{ResourceError, ResourceInstanceId};
use fabric_resource_kv::{KvAccess, KvAdapter, KvContract, KvService, PreparedKv, PreparedKvState};

use crate::config::FjallKvConfig;

use super::runtime::FjallKvRuntimeCore;

pub struct FjallKvAdapter {
    core: Arc<FjallKvRuntimeCore>,
    service: Arc<FjallKvService>,
}

struct FjallKvService {
    core: Arc<FjallKvRuntimeCore>,
}

impl FjallKvAdapter {
    pub fn new(config: FjallKvConfig) -> Self {
        let core = Arc::new(FjallKvRuntimeCore::new(config));
        Self {
            service: Arc::new(FjallKvService {
                core: Arc::clone(&core),
            }),
            core,
        }
    }

    #[doc(hidden)]
    pub fn testing_contract(&self) -> KvContract {
        initialize_root(self.core.root()).expect("initialize Fjall test root");
        self.core.start().expect("start Fjall test runtime");
        KvContract::new(self.service())
    }

    #[doc(hidden)]
    pub fn testing_keyspace_count(&self) -> usize {
        self.core.testing_keyspace_count()
    }

    #[doc(hidden)]
    pub fn testing_resolve_with_pause(
        &self,
        resource_id: &ResourceInstanceId,
        entered: &std::sync::Barrier,
        release: &std::sync::Barrier,
    ) -> Result<KvAccess, ResourceError> {
        self.core
            .testing_access_handle_with_pause(resource_id, entered, release)
    }
}

impl KvAdapter for FjallKvAdapter {
    fn service(&self) -> Arc<dyn KvService> {
        Arc::clone(&self.service) as Arc<dyn KvService>
    }

    fn initialize(&self) -> Result<(), ModuleError> {
        initialize_root(self.core.root())
    }

    fn start(&self) -> Result<(), ResourceError> {
        self.core.start()
    }

    fn stop(&self) {
        self.core.stop();
    }
}

impl KvService for FjallKvService {
    fn prepare(&self, resource_id: &ResourceInstanceId) -> Result<PreparedKvState, ResourceError> {
        self.core.prepare(resource_id)
    }

    fn cleanup(&self, prepared: &PreparedKv) {
        self.core.cleanup(prepared)
    }

    fn resolve(&self, resource_id: &ResourceInstanceId) -> Result<KvAccess, ResourceError> {
        self.core.access_handle(resource_id)
    }
}

fn initialize_root(root: &std::path::Path) -> Result<(), ModuleError> {
    std::fs::create_dir_all(root).map_err(|error| ModuleError::new(error.to_string()))
}
