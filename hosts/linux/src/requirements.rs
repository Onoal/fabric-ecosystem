use fabric::prelude::HostRequirement;

use crate::facts::{cgroup_v2_facility, linux_operating_system, procfs_facility};

pub fn linux_requirement() -> HostRequirement {
    HostRequirement::new().allow_operating_system(linux_operating_system())
}

pub fn linux_procfs_requirement() -> HostRequirement {
    linux_requirement().require_facility(procfs_facility())
}

pub fn linux_cgroup_v2_requirement() -> HostRequirement {
    linux_requirement().require_facility(cgroup_v2_facility())
}
