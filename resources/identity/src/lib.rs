mod contract;
mod error;
mod model;
mod native;

#[cfg(test)]
mod tests;

pub use contract::{
    IdentityContract, identity_contract_id, identity_contract_key, identity_resource_id,
};
pub use error::IdentityError;
pub use model::{Principal, PrincipalId};
pub use native::module::{NativeIdentity, NativeIdentityConfig};
