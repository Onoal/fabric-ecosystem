use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use fabric_resource_key_value::{KeyValueError, KeyValueStore, KeyValueStoreRealization};

#[derive(Debug)]
pub struct MemoryState(Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>);

impl MemoryState {
    pub fn fresh() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }
}

impl Clone for MemoryState {
    fn clone(&self) -> Self {
        Self::fresh()
    }
}

pub fn memory_adapter() -> MemoryKeyValueAdapter {
    MemoryKeyValueAdapter::new(MemoryKeyValueAdapterConfig {
        state: MemoryState::fresh(),
    })
}

fabric_sdk::adapter! {
    pub MemoryKeyValueAdapter
        for resource KeyValueStore
        implements KeyValueStoreRealization
    {
        schema: "^1";
        realization: "1.0.0";

        config { state: MemoryState; }

        runtime {
            fn get(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>, KeyValueError> {
                if key.is_empty() { return Err(KeyValueError::InvalidKey); }
                Ok(self.config.state.0.read().map_err(|_| KeyValueError::Unavailable)?.get(&key).cloned())
            }

            fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), KeyValueError> {
                if key.is_empty() { return Err(KeyValueError::InvalidKey); }
                self.config.state.0.write().map_err(|_| KeyValueError::Unavailable)?.insert(key, value);
                Ok(())
            }

            fn delete(&self, key: Vec<u8>) -> Result<(), KeyValueError> {
                if key.is_empty() { return Err(KeyValueError::InvalidKey); }
                self.config.state.0.write().map_err(|_| KeyValueError::Unavailable)?.remove(&key);
                Ok(())
            }
        }
    }
}
