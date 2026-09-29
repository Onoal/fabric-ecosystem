use crate::KeyValueError;

fabric::resource! {
    pub KeyValue {
        id: "onoal.package.data.key-value";

        api {
            async fn get(&self, key: String) -> Result<Option<Vec<u8>>, KeyValueError>;
            async fn set(&self, key: String, value: Vec<u8>) -> Result<(), KeyValueError>;
            async fn delete(&self, key: String) -> Result<(), KeyValueError>;
        }
    }
}
