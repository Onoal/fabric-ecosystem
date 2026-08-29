mod contract;
mod error;
mod model;
mod native;

#[cfg(test)]
mod tests;

pub use contract::{
    SecretsContract, secrets_contract_id, secrets_contract_key, secrets_resource_id,
};
pub use error::SecretsError;
pub use model::{
    AuthorizedSecretRef, MaterializedSecret, SecretId, SecretMaterial, SecretRef, SecretVersion,
    StoredSecret, secret_create_action, secret_delete_action, secret_materialize_action,
    secret_rotate_action,
};
pub use native::module::{NativeSecrets, NativeSecretsConfig};
