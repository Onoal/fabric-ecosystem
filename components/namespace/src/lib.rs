#![forbid(unsafe_code)]

mod contract;
mod error;
mod model;
mod native;

pub use contract::{Namespace, NamespaceService, namespace_contract_id, namespace_contract_key};
pub use error::NamespaceError;
pub use model::{NamespaceClaim, NamespaceName};
pub use native::NamespaceModule;
