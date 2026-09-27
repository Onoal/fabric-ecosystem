use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::{KeyValue, KeyValueError};

#[derive(Default)]
struct MemoryKeyValueState {
    entries: Mutex<BTreeMap<String, Vec<u8>>>,
}

fabric::adapter! {
    pub MemoryKeyValue for KeyValue {
        id: "onoal.package.data.key-value.memory";

        state {
            MemoryKeyValueState = MemoryKeyValueState::default();
        }

        runtime {
            fn get(&self, key: String) -> Result<Option<Vec<u8>>, KeyValueError> {
                self.state
                    .get()
                    .entries
                    .lock()
                    .map_err(|_| KeyValueError::read_failed("memory key-value state lock poisoned"))
                    .map(|entries| entries.get(&key).cloned())
            }

            fn set(&self, key: String, value: Vec<u8>) -> Result<(), KeyValueError> {
                self.state
                    .get()
                    .entries
                    .lock()
                    .map_err(|_| KeyValueError::write_failed("memory key-value state lock poisoned"))
                    .map(|mut entries| {
                        entries.insert(key, value);
                    })
            }

            fn delete(&self, key: String) -> Result<Option<Vec<u8>>, KeyValueError> {
                self.state
                    .get()
                    .entries
                    .lock()
                    .map_err(|_| KeyValueError::delete_failed("memory key-value state lock poisoned"))
                    .map(|mut entries| entries.remove(&key))
            }
        }
    }
}
