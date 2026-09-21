//! The semantic KeyValue Resource.
//!
//! This crate owns keyed byte storage semantics only. Concrete storage
//! mechanisms belong to independent realization crates.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyValueError {
    InvalidKey,
    Unavailable,
}

mod definition {
    use super::KeyValueError;

    fabric::resource! {
        pub KeyValueStore {
            id: "onoal.resource.key-value";
            schema: provisional;
            config {}
            contracts {
                primary Api {
                    id: "onoal.resource.key-value.api";
                    version: provisional;

                    fn get(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>, KeyValueError>;
                    fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), KeyValueError>;
                    fn delete(&self, key: Vec<u8>) -> Result<(), KeyValueError>;
                }
            }
            adapter Adapter {
                id: "onoal.resource.key-value.realization";
                compatibility: "^1";

                fn get(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>, KeyValueError>;
                fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), KeyValueError>;
                fn delete(&self, key: Vec<u8>) -> Result<(), KeyValueError>;
            }
            runtime {
                fn get(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>, KeyValueError> {
                    self.adapter.get(key)
                }

                fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), KeyValueError> {
                    self.adapter.put(key, value)
                }

                fn delete(&self, key: Vec<u8>) -> Result<(), KeyValueError> {
                    self.adapter.delete(key)
                }
            }
        }
    }
}

pub use definition::{KeyValueStore, KeyValueStoreConfig, KeyValueStoreRealization};
