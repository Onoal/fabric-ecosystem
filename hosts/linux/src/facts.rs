use fabric::prelude::{HostArchitecture, HostFacilityId, HostOperatingSystem};

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
