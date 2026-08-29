mod contract;
mod decision;
mod error;
mod model;
mod native;
#[cfg(test)]
mod source_guards;

#[cfg(test)]
mod tests;

pub use contract::{
    AuthorityContract, authority_contract_id, authority_contract_key, authority_resource_id,
};
pub use decision::{AuthorityDecisionAdapter, AuthorityEvaluationInput, AuthorityGrant};
pub use error::AuthorityError;
pub use model::{
    ActionId, ActorRef, AuthorityDecision, AuthorityRequest, AuthorityScopeId, RequestContext,
    ResourceRef,
};
#[cfg(feature = "test-support")]
pub use native::module::NativeAuthorityFailurePoint;
pub use native::module::{NativeAuthority, NativeAuthorityConfig};
