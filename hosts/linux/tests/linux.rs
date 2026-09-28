use fabric::prelude::*;
use fabric_host_linux::{
    cgroup_v2_facility, detect_linux_host, linux_cgroup_v2_requirement, linux_operating_system,
    linux_procfs_requirement, linux_requirement, native_architecture, procfs_facility,
};

#[cfg(not(target_os = "linux"))]
use fabric_host_linux::LinuxHostErrorKind;

#[cfg(target_os = "linux")]
fabric::resource! {
    TestHostMaterializationResource {
        id: "onoal.host.linux.test.materialization-resource";

        api {
            async fn value(&self) -> String;
        }
    }
}

#[cfg(target_os = "linux")]
fabric::adapter! {
    TestHostMaterializationAdapter for TestHostMaterializationResource {
        id: "onoal.host.linux.test.materialization-adapter";

        host: linux_requirement();

        runtime {
            async fn value(&self) -> String {
                "linux-host".to_owned()
            }
        }
    }
}

fn host_descriptor(
    os: &str,
    arch: &str,
    facilities: impl IntoIterator<Item = HostFacilityId>,
) -> HostDescriptor {
    let os = HostOperatingSystem::new(os).expect("test operating system id");
    let arch = HostArchitecture::new(arch).expect("test architecture id");
    HostDescriptor::new(os, arch).with_facilities(facilities)
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
}

#[test]
fn linux_requirement_accepts_linux_and_rejects_non_linux() {
    let linux = host_descriptor("linux", "x86_64", []);
    let macos = host_descriptor("macos", "x86_64", []);
    assert!(linux_requirement().evaluate(&linux).is_ok());
    assert!(linux_requirement().evaluate(&macos).is_err());
}

#[test]
fn procfs_requirement_requires_declared_facility() {
    let with_procfs = host_descriptor("linux", "x86_64", [procfs_facility()]);
    let without_procfs = host_descriptor("linux", "x86_64", []);
    assert!(linux_procfs_requirement().evaluate(&with_procfs).is_ok());
    assert!(linux_procfs_requirement()
        .evaluate(&without_procfs)
        .is_err());
}

#[test]
fn cgroup_v2_requirement_requires_declared_facility() {
    let with_cgroup_v2 = host_descriptor("linux", "x86_64", [cgroup_v2_facility()]);
    let without_cgroup_v2 = host_descriptor("linux", "x86_64", []);
    assert!(linux_cgroup_v2_requirement()
        .evaluate(&with_cgroup_v2)
        .is_ok());
    assert!(linux_cgroup_v2_requirement()
        .evaluate(&without_cgroup_v2)
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
