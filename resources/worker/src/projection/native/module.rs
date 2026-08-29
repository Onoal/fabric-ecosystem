use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fabric_core::{
    ContractRequirement, Health, Module, ModuleBindings, ModuleContract, ModuleError, ModuleId,
    ModuleRuntime,
};
use fabric_projection::ProjectionError;
use fabric_resource_database::{
    SqliteDatabaseCompatibilityContract, sqlite_database_compatibility_contract_id,
};
use fabric_resource_kv::{KvContract, kv_contract_id};
use fabric_resource_secrets::{SecretsContract, secrets_contract_id};
use fabric_resource_service::{ServiceContract, service_contract_id};

use crate::{
    BindingTarget, WorkloadBinding, WorkloadBindingProjection, validate_workload_bindings,
};

use super::database::project_database_binding;
use super::kv::project_kv_binding;
use super::secrets::project_secret_binding;
use super::service::project_service_binding;
use crate::projection::{
    PreparedWorkloadProjections, WorkloadProjectionContract, WorkloadProjectionService,
    workload_projection_contract_key,
};

pub struct NativeWorkloadProjection {
    module_id: ModuleId,
    database_requirement: ContractRequirement<SqliteDatabaseCompatibilityContract>,
    kv_requirement: ContractRequirement<KvContract>,
    secrets_requirement: ContractRequirement<SecretsContract>,
    service_requirement: ContractRequirement<ServiceContract>,
    shared: Arc<SharedProjectionState>,
}

struct SharedProjectionState {
    inner: Mutex<State>,
}

struct State {
    health: Health,
    started: bool,
    contracts: ProjectionContracts,
}

#[derive(Default, Clone)]
struct ProjectionContracts {
    database: Option<SqliteDatabaseCompatibilityContract>,
    kv: Option<KvContract>,
    secrets: Option<SecretsContract>,
    service: Option<ServiceContract>,
}

impl NativeWorkloadProjection {
    pub fn new() -> Self {
        Self {
            module_id: ModuleId::new("fabric.projection.workload.native")
                .expect("static workload projection module id"),
            database_requirement: ContractRequirement::provisional(
                sqlite_database_compatibility_contract_id(),
            ),
            kv_requirement: ContractRequirement::provisional(kv_contract_id()),
            secrets_requirement: ContractRequirement::provisional(secrets_contract_id()),
            service_requirement: ContractRequirement::provisional(service_contract_id()),
            shared: Arc::new(SharedProjectionState {
                inner: Mutex::new(State {
                    health: Health::Unavailable,
                    started: false,
                    contracts: ProjectionContracts::default(),
                }),
            }),
        }
    }
}

impl Default for NativeWorkloadProjection {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleRuntime for NativeWorkloadProjection {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        vec![workload_projection_contract_key().declaration()]
    }

    fn optional_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![
            self.database_requirement.declaration().clone(),
            self.kv_requirement.declaration().clone(),
            self.secrets_requirement.declaration().clone(),
            self.service_requirement.declaration().clone(),
        ]
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        let service: Arc<dyn WorkloadProjectionService> =
            Arc::clone(&self.shared) as Arc<dyn WorkloadProjectionService>;
        Ok(vec![ModuleContract::new(
            &workload_projection_contract_key(),
            Arc::new(WorkloadProjectionContract::new(service)),
        )])
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let database = bindings
            .resolve_optional(&self.database_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let kv = bindings
            .resolve_optional(&self.kv_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let secrets = bindings
            .resolve_optional(&self.secrets_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let service = bindings
            .resolve_optional(&self.service_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let mut state = self.shared.inner.lock().expect("projection state lock");
        state.contracts = ProjectionContracts {
            database: database.as_deref().cloned(),
            kv: kv.as_deref().cloned(),
            secrets: secrets.as_deref().cloned(),
            service: service.as_deref().cloned(),
        };
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), ModuleError> {
        let mut state = self.shared.inner.lock().expect("projection state lock");
        state.started = false;
        state.health = Health::Unavailable;
        Ok(())
    }

    fn start(&mut self) -> Result<(), ModuleError> {
        let mut state = self.shared.inner.lock().expect("projection state lock");
        state.started = true;
        state.health = Health::Healthy;
        Ok(())
    }

    fn stop(&mut self) {
        let mut state = self.shared.inner.lock().expect("projection state lock");
        state.started = false;
        state.health = Health::Unavailable;
    }

    fn health(&self) -> Health {
        self.shared
            .inner
            .lock()
            .expect("projection state lock")
            .health
    }
}

impl Module for NativeWorkloadProjection {
    fn materialize(&self) -> Box<dyn ModuleRuntime> {
        Box::new(Self::new())
    }
}

impl WorkloadProjectionService for SharedProjectionState {
    fn prepare_workload_projections(
        &self,
        bindings: &[WorkloadBinding],
        projections: &[WorkloadBindingProjection],
    ) -> Result<PreparedWorkloadProjections, ProjectionError> {
        let state = self.inner.lock().expect("projection state lock");
        if !state.started {
            return Err(ProjectionError::Unavailable);
        }
        let mut by_binding = BTreeMap::<_, Vec<_>>::new();
        for projection in projections {
            by_binding
                .entry(projection.binding_id().clone())
                .or_default()
                .push(projection.clone());
        }
        let mut prepared = PreparedWorkloadProjections::default();
        for binding in bindings {
            let binding_projections = by_binding.remove(binding.binding_id()).unwrap_or_default();
            if binding_projections.is_empty() {
                continue;
            }
            validate_workload_bindings(
                binding.consumer(),
                std::slice::from_ref(binding),
                &binding_projections,
            )
            .map_err(|error| ProjectionError::invalid_input(error.to_string()))?;
            match binding.target() {
                BindingTarget::Database(reference) => {
                    let database = state
                        .contracts
                        .database
                        .as_ref()
                        .ok_or(ProjectionError::Unavailable)?;
                    project_database_binding(
                        binding,
                        reference,
                        &binding_projections,
                        database,
                        &mut prepared,
                    )?;
                }
                BindingTarget::Kv(reference) => {
                    let kv = state
                        .contracts
                        .kv
                        .as_ref()
                        .ok_or(ProjectionError::Unavailable)?;
                    project_kv_binding(binding, reference, &binding_projections, kv, &mut prepared)?
                }
                BindingTarget::Secret(reference) => {
                    let secrets = state
                        .contracts
                        .secrets
                        .as_ref()
                        .ok_or(ProjectionError::Unavailable)?;
                    project_secret_binding(
                        binding,
                        reference,
                        &binding_projections,
                        secrets,
                        &mut prepared,
                    )?;
                }
                BindingTarget::Service(service_id) => {
                    let service = state
                        .contracts
                        .service
                        .as_ref()
                        .ok_or(ProjectionError::Unavailable)?;
                    project_service_binding(
                        binding,
                        service_id,
                        &binding_projections,
                        service,
                        &mut prepared,
                    )?;
                }
            }
        }
        Ok(prepared)
    }
}
