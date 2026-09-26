//! Linux Host artifact for Fabric.
//!
//! This crate returns ordinary Fabric `HostDescriptor` and `HostRequirement`
//! values. It does not define a Linux Resource, System, Component, Adapter, or
//! parallel Host model.

use std::fmt;

use fabric::prelude::{
    HostArchitecture, HostDescriptor, HostFacilityId, HostOperatingSystem, HostRequirement,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinuxHostErrorKind {
    UnsupportedPlatform,
    InvalidHostIdentifier,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinuxHostError {
    pub kind: LinuxHostErrorKind,
    pub detail: String,
}

impl LinuxHostError {
    pub fn new(kind: LinuxHostErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    #[cfg(test)]
    fn invalid_identifier(detail: impl Into<String>) -> Self {
        Self::new(LinuxHostErrorKind::InvalidHostIdentifier, detail)
    }

    #[cfg(not(target_os = "linux"))]
    fn unsupported_platform() -> Self {
        Self::new(
            LinuxHostErrorKind::UnsupportedPlatform,
            "linux host detection is only supported on target_os = linux",
        )
    }
}

impl fmt::Display for LinuxHostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for LinuxHostError {}

pub fn linux_operating_system() -> HostOperatingSystem {
    HostOperatingSystem::new("linux").expect("static linux operating system id")
}

pub fn native_architecture() -> HostArchitecture {
    HostArchitecture::new(std::env::consts::ARCH).expect("native architecture id")
}

pub fn procfs_facility() -> HostFacilityId {
    HostFacilityId::new("linux.procfs").expect("static linux procfs facility id")
}

pub fn cgroup_v2_facility() -> HostFacilityId {
    HostFacilityId::new("linux.cgroup-v2").expect("static linux cgroup v2 facility id")
}

pub fn linux_requirement() -> HostRequirement {
    HostRequirement::new().allow_operating_system(linux_operating_system())
}

pub fn linux_procfs_requirement() -> HostRequirement {
    linux_requirement().require_facility(procfs_facility())
}

pub fn linux_cgroup_v2_requirement() -> HostRequirement {
    linux_requirement().require_facility(cgroup_v2_facility())
}

pub fn procfs_available() -> bool {
    procfs_available_impl()
}

pub fn cgroup_v2_available() -> bool {
    cgroup_v2_available_impl()
}

pub fn detect_linux_host() -> Result<HostDescriptor, LinuxHostError> {
    detect_linux_host_impl()
}

#[cfg(target_os = "linux")]
fn detect_linux_host_impl() -> Result<HostDescriptor, LinuxHostError> {
    let mut descriptor = HostDescriptor::new(linux_operating_system(), native_architecture());
    if procfs_available_impl() {
        descriptor = descriptor.with_facility(procfs_facility());
    }
    if cgroup_v2_available_impl() {
        descriptor = descriptor.with_facility(cgroup_v2_facility());
    }
    Ok(descriptor)
}

#[cfg(not(target_os = "linux"))]
fn detect_linux_host_impl() -> Result<HostDescriptor, LinuxHostError> {
    Err(LinuxHostError::unsupported_platform())
}

#[cfg(target_os = "linux")]
fn procfs_available_impl() -> bool {
    std::path::Path::new("/proc/self/stat").is_file()
}

#[cfg(not(target_os = "linux"))]
fn procfs_available_impl() -> bool {
    false
}

#[cfg(target_os = "linux")]
fn cgroup_v2_available_impl() -> bool {
    std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").is_file()
}

#[cfg(not(target_os = "linux"))]
fn cgroup_v2_available_impl() -> bool {
    false
}

#[cfg(test)]
fn host_descriptor(
    os: &str,
    arch: &str,
    facilities: impl IntoIterator<Item = HostFacilityId>,
) -> Result<HostDescriptor, LinuxHostError> {
    let os = HostOperatingSystem::new(os)
        .map_err(|error| LinuxHostError::invalid_identifier(error.to_string()))?;
    let arch = HostArchitecture::new(arch)
        .map_err(|error| LinuxHostError::invalid_identifier(error.to_string()))?;
    Ok(HostDescriptor::new(os, arch).with_facilities(facilities))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabric::prelude::*;

    #[cfg(target_os = "linux")]
    fabric::resource! {
        TestHostMaterializationResource {
            id: "onoal.host.linux.test.materialization-resource";

            api {
                fn value(&self) -> String;
            }
        }
    }

    #[cfg(target_os = "linux")]
    fabric::adapter! {
        TestHostMaterializationAdapter for TestHostMaterializationResource {
            id: "onoal.host.linux.test.materialization-adapter";

            host: linux_requirement();

            runtime {
                fn value(&self) -> String {
                    "linux-host".to_owned()
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn native_linux_detection_returns_fabric_descriptor() {
        let descriptor = detect_linux_host().expect("linux host");
        assert_eq!(descriptor.operating_system(), &linux_operating_system());
        assert_eq!(descriptor.architecture(), &native_architecture());
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_detection_is_explicit() {
        let error = detect_linux_host().expect_err("unsupported platform");
        assert_eq!(error.kind, LinuxHostErrorKind::UnsupportedPlatform);
        assert!(!procfs_available());
        assert!(!cgroup_v2_available());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn procfs_facility_matches_probe_evidence() {
        let descriptor = detect_linux_host().expect("linux host");
        assert_eq!(
            descriptor.facilities().contains(&procfs_facility()),
            procfs_available()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cgroup_v2_facility_matches_probe_evidence() {
        let descriptor = detect_linux_host().expect("linux host");
        assert_eq!(
            descriptor.facilities().contains(&cgroup_v2_facility()),
            cgroup_v2_available()
        );
    }

    #[test]
    fn linux_requirement_accepts_linux_and_rejects_non_linux() {
        let linux = host_descriptor("linux", "x86_64", []).expect("linux descriptor");
        let macos = host_descriptor("macos", "x86_64", []).expect("macos descriptor");
        assert!(linux_requirement().evaluate(&linux).is_ok());
        assert!(linux_requirement().evaluate(&macos).is_err());
    }

    #[test]
    fn facility_requirement_requires_declared_facility() {
        let with_procfs =
            host_descriptor("linux", "x86_64", [procfs_facility()]).expect("linux procfs");
        let without_procfs = host_descriptor("linux", "x86_64", []).expect("linux no procfs");
        assert!(linux_procfs_requirement().evaluate(&with_procfs).is_ok());
        assert!(linux_procfs_requirement()
            .evaluate(&without_procfs)
            .is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn detected_host_descriptor_materializes_fabric_normally() {
        let composition = Fabric::new("onoal.host.linux.test.materialization")
            .expect("fabric")
            .resource(
                TestHostMaterializationResource::select("primary")
                    .expect("resource")
                    .using(TestHostMaterializationAdapter::new())
                    .expect("adapter"),
            )
            .build()
            .expect("composition");
        let mut instance = composition
            .materialize_on(
                "onoal.host.linux.test.materialization.instance",
                &detect_linux_host().expect("linux host"),
            )
            .expect("instance");
        instance.start().expect("start");
        // Resource invocation is intentionally represented through materialization success here:
        // the test proves the detected HostDescriptor directly satisfies Fabric host checks.
        assert_eq!(
            composition
                .resources()
                .next()
                .expect("resource")
                .name()
                .as_str(),
            "primary"
        );
        instance.stop().expect("stop");
    }
}
