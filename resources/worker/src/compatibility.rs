use std::collections::BTreeSet;

use crate::{
    WorkerApiVersion, WorkerCapabilities, WorkerError, WorkerFeature, WorkerFeatureSupport,
    WorkloadRequirement,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompatibilityIssue {
    ApiVersionUnavailable { version: WorkerApiVersion },
    FeatureUnavailable { feature: WorkerFeature },
    FeatureUnsupported { feature: WorkerFeature },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompatibilityReport {
    issues: Vec<CompatibilityIssue>,
}

impl CompatibilityReport {
    pub fn is_compatible(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn issues(&self) -> &[CompatibilityIssue] {
        &self.issues
    }
}

pub(crate) fn validate_capabilities(capabilities: &WorkerCapabilities) -> Result<(), WorkerError> {
    let mut versions = BTreeSet::new();
    for version in &capabilities.api_versions {
        if !versions.insert(version.as_semver().to_string()) {
            return Err(WorkerError::ProtocolViolation {
                message: "duplicate worker api version capability".to_owned(),
            });
        }
    }
    let mut features = BTreeSet::new();
    for support in &capabilities.features {
        if !features.insert(support.feature.as_str().to_owned()) {
            return Err(WorkerError::ProtocolViolation {
                message: "duplicate worker feature capability".to_owned(),
            });
        }
    }
    Ok(())
}

pub fn evaluate_compatibility(
    requirement: &WorkloadRequirement,
    capabilities: &WorkerCapabilities,
) -> Result<CompatibilityReport, WorkerError> {
    validate_capabilities(capabilities)?;
    let mut required_features = BTreeSet::new();
    for feature in &requirement.features {
        if !required_features.insert(feature.as_str()) {
            return Err(WorkerError::invalid_input("duplicate workload feature"));
        }
    }
    let mut issues = Vec::new();
    if !capabilities
        .api_versions
        .iter()
        .any(|version| version == &requirement.api_version)
    {
        issues.push(CompatibilityIssue::ApiVersionUnavailable {
            version: requirement.api_version.clone(),
        });
    }
    for feature in &requirement.features {
        match capabilities
            .features
            .iter()
            .find(|support| support.feature == *feature)
        {
            None => issues.push(CompatibilityIssue::FeatureUnavailable {
                feature: feature.clone(),
            }),
            Some(WorkerFeatureSupport {
                supported: false, ..
            }) => {
                issues.push(CompatibilityIssue::FeatureUnsupported {
                    feature: feature.clone(),
                });
            }
            Some(_) => {}
        }
    }
    Ok(CompatibilityReport { issues })
}
