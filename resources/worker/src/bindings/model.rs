use std::collections::{BTreeMap, BTreeSet};

use fabric_binding::{
    BindingConsumer, BindingConsumerId, BindingConsumerKind, BindingId, BindingName,
    BindingTargetId, BindingTargetKind,
};
use fabric_resource_database::DatabaseRef;
use fabric_resource_kv::KvRef;
use fabric_resource_secrets::AuthorizedSecretRef;
use fabric_resource_service::ServiceId;

use crate::{WorkerError, WorkloadId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkloadBinding {
    binding_id: BindingId,
    consumer: WorkloadId,
    name: BindingName,
    target: BindingTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkloadBindingProjection {
    binding_id: BindingId,
    projection: BindingProjection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingProjection {
    Structured,
    Environment(WorkloadBindingEnv),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkloadBindingEnv(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingTarget {
    Database(DatabaseRef),
    Kv(KvRef),
    Secret(AuthorizedSecretRef),
    Service(ServiceId),
}

impl WorkloadBinding {
    pub fn new(consumer: WorkloadId, name: BindingName, target: BindingTarget) -> Self {
        let binding_id = BindingId::canonical(
            &binding_consumer(&consumer),
            &name,
            &target_kind(&target),
            &target_id(&target),
        );
        Self {
            binding_id,
            consumer,
            name,
            target,
        }
    }

    pub fn binding_id(&self) -> &BindingId {
        &self.binding_id
    }

    pub fn consumer(&self) -> &WorkloadId {
        &self.consumer
    }

    pub fn name(&self) -> &BindingName {
        &self.name
    }

    pub fn target(&self) -> &BindingTarget {
        &self.target
    }

    pub fn validate_consumer(&self, workload_id: &WorkloadId) -> Result<(), WorkerError> {
        if &self.consumer == workload_id {
            Ok(())
        } else {
            Err(WorkerError::invalid_input(format!(
                "binding {} belongs to workload {} and cannot be used by {}",
                self.name.as_str(),
                self.consumer.as_str(),
                workload_id.as_str()
            )))
        }
    }
}

impl WorkloadBindingProjection {
    pub fn new(binding_id: BindingId, projection: BindingProjection) -> Self {
        Self {
            binding_id,
            projection,
        }
    }

    pub fn structured(binding_id: &BindingId) -> Self {
        Self::new(binding_id.clone(), BindingProjection::Structured)
    }

    pub fn environment(binding_id: &BindingId, variable: WorkloadBindingEnv) -> Self {
        Self::new(binding_id.clone(), BindingProjection::Environment(variable))
    }

    pub fn binding_id(&self) -> &BindingId {
        &self.binding_id
    }

    pub fn projection(&self) -> &BindingProjection {
        &self.projection
    }
}

impl WorkloadBindingEnv {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkerError> {
        let value = value.into();
        validate_binding_env(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl BindingTarget {
    pub fn database(reference: DatabaseRef) -> Self {
        Self::Database(reference)
    }

    pub fn kv(reference: KvRef) -> Self {
        Self::Kv(reference)
    }

    pub fn secret(reference: AuthorizedSecretRef) -> Self {
        Self::Secret(reference)
    }

    pub fn service(service_id: ServiceId) -> Self {
        Self::Service(service_id)
    }
}

impl BindingProjection {
    pub fn structured() -> Self {
        Self::Structured
    }

    pub fn environment(variable: WorkloadBindingEnv) -> Self {
        Self::Environment(variable)
    }
}

pub fn validate_workload_bindings(
    workload_id: &WorkloadId,
    bindings: &[WorkloadBinding],
    projections: &[WorkloadBindingProjection],
) -> Result<(), WorkerError> {
    let mut binding_ids = BTreeSet::new();
    let mut bindings_by_id = BTreeMap::new();
    for binding in bindings {
        binding.validate_consumer(workload_id)?;
        if !binding_ids.insert(binding.binding_id().clone()) {
            return Err(WorkerError::invalid_input(format!(
                "duplicate workload binding {}",
                binding.name().as_str()
            )));
        }
        bindings_by_id.insert(binding.binding_id().clone(), binding);
    }

    let mut structured = BTreeSet::new();
    let mut environment = BTreeSet::new();
    for projection in projections {
        let Some(binding) = bindings_by_id.get(projection.binding_id()) else {
            return Err(WorkerError::invalid_input(format!(
                "binding projection references unknown binding {}",
                projection.binding_id().as_str()
            )));
        };
        match projection.projection() {
            BindingProjection::Structured => {
                if !structured.insert(binding.binding_id().clone()) {
                    return Err(WorkerError::invalid_input(format!(
                        "duplicate structured binding projection for {}",
                        binding.name().as_str()
                    )));
                }
            }
            BindingProjection::Environment(variable) => {
                if !environment.insert(variable.clone()) {
                    return Err(WorkerError::invalid_input(format!(
                        "duplicate environment binding projection for {}",
                        variable.as_str()
                    )));
                }
            }
        }
    }
    Ok(())
}

fn binding_consumer(workload_id: &WorkloadId) -> BindingConsumer {
    BindingConsumer::new(
        BindingConsumerKind::new("worker.workload").expect("static binding consumer kind"),
        BindingConsumerId::new(workload_id.as_str())
            .expect("workload id is valid binding consumer id"),
    )
}

fn target_kind(target: &BindingTarget) -> BindingTargetKind {
    match target {
        BindingTarget::Database(_) => {
            BindingTargetKind::new("database-ref").expect("static database target kind")
        }
        BindingTarget::Kv(_) => BindingTargetKind::new("kv-ref").expect("static kv target kind"),
        BindingTarget::Secret(_) => {
            BindingTargetKind::new("authorized-secret-ref").expect("static secret target kind")
        }
        BindingTarget::Service(_) => {
            BindingTargetKind::new("service-id").expect("static service target kind")
        }
    }
}

fn target_id(target: &BindingTarget) -> BindingTargetId {
    match target {
        BindingTarget::Database(reference) => {
            BindingTargetId::new(reference.encode()).expect("database target id")
        }
        BindingTarget::Kv(reference) => {
            BindingTargetId::new(reference.encode()).expect("kv target id")
        }
        BindingTarget::Secret(reference) => BindingTargetId::new(format!(
            "{}:{}:{}",
            reference.actor().as_str(),
            reference.secret().scope_id.as_str(),
            reference.secret().id.as_str()
        ))
        .expect("secret target id"),
        BindingTarget::Service(service_id) => {
            BindingTargetId::new(service_id.as_str()).expect("service target id")
        }
    }
}

fn validate_binding_env(value: &str) -> Result<(), WorkerError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_uppercase() || byte.is_ascii_digit() && index > 0 || byte == b'_'
        })
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_uppercase() || byte == b'_');
    if valid {
        Ok(())
    } else {
        Err(WorkerError::invalid_input(
            "invalid workload binding environment variable",
        ))
    }
}
