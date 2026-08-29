use fabric_projection::ProjectionError;
use fabric_resource_secrets::{AuthorizedSecretRef, SecretsContract};

use crate::{BindingProjection, WorkloadBinding, WorkloadBindingProjection};

use crate::projection::PreparedWorkloadProjections;

pub(crate) fn project_secret_binding(
    binding: &WorkloadBinding,
    reference: &AuthorizedSecretRef,
    projections: &[WorkloadBindingProjection],
    secrets: &SecretsContract,
    prepared: &mut PreparedWorkloadProjections,
) -> Result<(), ProjectionError> {
    let materialized = secrets.materialize_authorized(reference).map_err(|error| {
        ProjectionError::materialization_failed(format!(
            "materialize secret binding {}: {error}",
            binding.name().as_str()
        ))
    })?;
    for projection in projections {
        match projection.projection() {
            BindingProjection::Structured => {
                return Err(ProjectionError::invalid_input(format!(
                    "secret binding {} requires an environment projection",
                    binding.name().as_str()
                )));
            }
            BindingProjection::Environment(variable) => {
                prepared.insert_sensitive_environment(
                    variable.as_str(),
                    materialized.value.expose().to_owned(),
                );
            }
        }
    }
    Ok(())
}
