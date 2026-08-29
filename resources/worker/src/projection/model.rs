use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fabric_projection::ProjectionLeases;
use serde_json::Value as JsonValue;

use super::PreparedWorkloadEnvironmentValue;

#[derive(Default)]
pub struct PreparedWorkloadProjections {
    structured: BTreeMap<String, JsonValue>,
    environment: BTreeMap<String, PreparedWorkloadEnvironmentValue>,
    network_authorities: BTreeSet<String>,
    leases: ProjectionLeases,
}

impl fmt::Debug for PreparedWorkloadProjections {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedWorkloadProjections")
            .field("structured", &self.structured)
            .field("environment", &self.environment)
            .field("network_authorities", &self.network_authorities)
            .field("has_leases", &!self.leases.is_empty())
            .finish()
    }
}

impl PreparedWorkloadProjections {
    pub fn is_empty(&self) -> bool {
        self.structured.is_empty()
            && self.environment.is_empty()
            && self.network_authorities.is_empty()
            && self.leases.is_empty()
    }

    pub fn structured(&self) -> &BTreeMap<String, JsonValue> {
        &self.structured
    }

    pub fn environment(&self) -> &BTreeMap<String, PreparedWorkloadEnvironmentValue> {
        &self.environment
    }

    pub fn network_authorities(&self) -> &BTreeSet<String> {
        &self.network_authorities
    }

    pub fn insert_structured(&mut self, binding_name: impl Into<String>, value: JsonValue) {
        self.structured.insert(binding_name.into(), value);
    }

    pub fn insert_public_environment(
        &mut self,
        variable: impl Into<String>,
        value: impl Into<String>,
    ) {
        self.environment.insert(
            variable.into(),
            PreparedWorkloadEnvironmentValue::public(value),
        );
    }

    pub fn insert_sensitive_environment(
        &mut self,
        variable: impl Into<String>,
        value: impl Into<String>,
    ) {
        self.environment.insert(
            variable.into(),
            PreparedWorkloadEnvironmentValue::sensitive(value),
        );
    }

    pub fn allow_network_authority(&mut self, authority: impl Into<String>) {
        self.network_authorities.insert(authority.into());
    }

    pub fn append_leases(&mut self, leases: ProjectionLeases) {
        self.leases.append(leases);
    }

    pub fn into_parts(
        self,
    ) -> (
        BTreeMap<String, JsonValue>,
        BTreeMap<String, PreparedWorkloadEnvironmentValue>,
        BTreeSet<String>,
        ProjectionLeases,
    ) {
        (
            self.structured,
            self.environment,
            self.network_authorities,
            self.leases,
        )
    }
}
