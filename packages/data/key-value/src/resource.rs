use crate::KeyValueError;

fabric::resource! {
    pub KeyValue {
        id: "onoal.package.data.key-value";

        api {
            fn get(&self, key: String) -> Result<Option<Vec<u8>>, KeyValueError>;
            fn set(&self, key: String, value: Vec<u8>) -> Result<(), KeyValueError>;
            fn delete(&self, key: String) -> Result<Option<Vec<u8>>, KeyValueError>;
        }
    }
}
