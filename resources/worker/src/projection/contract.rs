use std::sync::Arc;

use fabric_core::{ContractId, ContractKey};
use fabric_projection::ProjectionError;

use crate::{WorkloadBinding, WorkloadBindingProjection};

use super::PreparedWorkloadProjections;

const WORKLOAD_PROJECTION_CONTRACT_ID: &str = "fabric.projection.workload";

pub fn workload_projection_contract_id() -> ContractId {
    ContractId::new(WORKLOAD_PROJECTION_CONTRACT_ID)
        .expect("static workload projection contract id")
}

pub fn workload_projection_contract_key() -> ContractKey<WorkloadProjectionContract> {
    ContractKey::provisional(workload_projection_contract_id())
}

pub trait WorkloadProjectionService: Send + Sync {
    fn prepare_workload_projections(
        &self,
        bindings: &[WorkloadBinding],
        projections: &[WorkloadBindingProjection],
    ) -> Result<PreparedWorkloadProjections, ProjectionError>;
}

#[derive(Clone)]
pub struct WorkloadProjectionContract {
    inner: Arc<dyn WorkloadProjectionService>,
}

impl WorkloadProjectionContract {
    pub fn new(inner: Arc<dyn WorkloadProjectionService>) -> Self {
        Self { inner }
    }

    pub fn prepare(
        &self,
        bindings: &[WorkloadBinding],
        projections: &[WorkloadBindingProjection],
    ) -> Result<PreparedWorkloadProjections, ProjectionError> {
        self.inner
            .prepare_workload_projections(bindings, projections)
    }
}
