use std::sync::Arc;

use fabric_component::Component;
use fabric_core::{ContractId, ContractKey};

use crate::{NamespaceClaim, NamespaceError, NamespaceName};

const NAMESPACE_CONTRACT_ID: &str = "fabric.component.namespace";

pub fn namespace_contract_id() -> ContractId {
    ContractId::new(NAMESPACE_CONTRACT_ID).expect("static namespace contract id")
}

pub fn namespace_contract_key() -> ContractKey<Namespace> {
    ContractKey::provisional(namespace_contract_id())
}

pub trait NamespaceService: Send + Sync {
    fn claim(
        &self,
        owner: Component,
        name: NamespaceName,
    ) -> Result<NamespaceClaim, NamespaceError>;

    fn allocation(&self, name: &NamespaceName) -> Result<NamespaceClaim, NamespaceError>;
}

#[derive(Clone)]
pub struct Namespace {
    inner: Arc<dyn NamespaceService>,
}

impl Namespace {
    pub fn new(inner: Arc<dyn NamespaceService>) -> Self {
        Self { inner }
    }

    pub fn claim(
        &self,
        owner: Component,
        name: NamespaceName,
    ) -> Result<NamespaceClaim, NamespaceError> {
        self.inner.claim(owner, name)
    }

    pub fn allocation(&self, name: &NamespaceName) -> Result<NamespaceClaim, NamespaceError> {
        self.inner.allocation(name)
    }
}
