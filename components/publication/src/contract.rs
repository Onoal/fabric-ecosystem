use std::sync::Arc;

use fabric_component::Surface;
use fabric_component_namespace::{NamespaceClaim, NamespaceName};
use fabric_core::{ContractId, ContractKey};

use crate::{Publication, PublicationError};

const PUBLICATION_CONTRACT_ID: &str = "fabric.component.publication";

pub fn publication_contract_id() -> ContractId {
    ContractId::new(PUBLICATION_CONTRACT_ID).expect("static publication contract id")
}

pub fn publication_contract_key() -> ContractKey<PublicationContract> {
    ContractKey::provisional(publication_contract_id())
}

pub trait PublicationService: Send + Sync {
    fn publish(
        &self,
        claim: NamespaceClaim,
        surface: Surface,
    ) -> Result<Publication, PublicationError>;

    fn publication(&self, name: &NamespaceName) -> Result<Publication, PublicationError>;

    fn publications(&self) -> Vec<Publication>;
}

#[derive(Clone)]
pub struct PublicationContract {
    inner: Arc<dyn PublicationService>,
}

impl PublicationContract {
    pub fn new(inner: Arc<dyn PublicationService>) -> Self {
        Self { inner }
    }

    pub fn publish(
        &self,
        claim: NamespaceClaim,
        surface: Surface,
    ) -> Result<Publication, PublicationError> {
        self.inner.publish(claim, surface)
    }

    pub fn publication(&self, name: &NamespaceName) -> Result<Publication, PublicationError> {
        self.inner.publication(name)
    }

    pub fn publications(&self) -> Vec<Publication> {
        self.inner.publications()
    }
}
