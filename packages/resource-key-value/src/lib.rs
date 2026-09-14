#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyValueError {
    InvalidKey,
    Unavailable,
}
mod definition {
    use super::KeyValueError;
    fabric_sdk::resource! {
        pub KeyValueStore {
            id: "fabric.resource.key-value";
            schema: "1.0.0";
            config {}
            contracts {
                primary Api { id: "fabric.resource.key-value"; version: "1.0.0";
                    fn get(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>, KeyValueError>;
                    fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), KeyValueError>;
                    fn delete(&self, key: Vec<u8>) -> Result<(), KeyValueError>;
                }
            }
            adapter Adapter { id: "fabric.resource.key-value.realization"; compatibility: "^1";
                fn get(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>, KeyValueError>;
                fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), KeyValueError>;
                fn delete(&self, key: Vec<u8>) -> Result<(), KeyValueError>;
            }
            runtime {
                fn get(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>, KeyValueError> { self.adapter.get(key) }
                fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), KeyValueError> { self.adapter.put(key, value) }
                fn delete(&self, key: Vec<u8>) -> Result<(), KeyValueError> { self.adapter.delete(key) }
            }
        }
    }
}
pub use definition::{KeyValueStore, KeyValueStoreConfig, KeyValueStoreRealization};
