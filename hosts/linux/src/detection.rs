use fabric::prelude::HostDescriptor;

use crate::error::LinuxHostError;
use crate::facts::{
    cgroup_v2_facility, linux_operating_system, native_architecture, procfs_facility,
};

pub fn detect_linux_host() -> Result<HostDescriptor, LinuxHostError> {
    detect_linux_host_impl()
}

#[cfg(target_os = "linux")]
fn detect_linux_host_impl() -> Result<HostDescriptor, LinuxHostError> {
    let mut descriptor = HostDescriptor::new(linux_operating_system(), native_architecture());
    if procfs_available() {
        descriptor = descriptor.with_facility(procfs_facility());
    }
    if cgroup_v2_available() {
        descriptor = descriptor.with_facility(cgroup_v2_facility());
    }
    Ok(descriptor)
}

#[cfg(not(target_os = "linux"))]
fn detect_linux_host_impl() -> Result<HostDescriptor, LinuxHostError> {
    Err(LinuxHostError::unsupported_platform())
}

#[cfg(target_os = "linux")]
fn procfs_available() -> bool {
    std::path::Path::new("/proc/self/stat").is_file()
}

#[cfg(target_os = "linux")]
fn cgroup_v2_available() -> bool {
    std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").is_file()
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests {
    use super::*;

    #[test]
    fn procfs_facility_matches_probe_evidence() {
        let descriptor = detect_linux_host().expect("linux host");
        assert_eq!(
            descriptor.facilities().contains(&procfs_facility()),
            procfs_available()
        );
    }

    #[test]
    fn cgroup_v2_facility_matches_probe_evidence() {
        let descriptor = detect_linux_host().expect("linux host");
        assert_eq!(
            descriptor.facilities().contains(&cgroup_v2_facility()),
            cgroup_v2_available()
        );
    }
}
