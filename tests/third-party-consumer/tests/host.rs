#[cfg(target_os = "linux")]
#[test]
fn external_consumer_uses_linux_host_descriptor_and_requirements() {
    let descriptor = fabric_host_linux::detect_linux_host().expect("linux host detection");

    assert_eq!(
        descriptor.operating_system(),
        &fabric_host_linux::linux_operating_system()
    );
    assert_eq!(
        descriptor.architecture(),
        &fabric_host_linux::native_architecture()
    );
    fabric_host_linux::linux_requirement()
        .evaluate(&descriptor)
        .expect("linux requirement");

    let facilities = descriptor.facilities();
    if facilities
        .iter()
        .any(|facility| facility == &fabric_host_linux::procfs_facility())
    {
        fabric_host_linux::linux_procfs_requirement()
            .evaluate(&descriptor)
            .expect("procfs requirement");
    }
    if facilities
        .iter()
        .any(|facility| facility == &fabric_host_linux::cgroup_v2_facility())
    {
        fabric_host_linux::linux_cgroup_v2_requirement()
            .evaluate(&descriptor)
            .expect("cgroup v2 requirement");
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn linux_host_detection_fails_explicitly_on_non_linux_targets() {
    let error = fabric_host_linux::detect_linux_host().expect_err("unsupported platform");
    assert_eq!(
        error.kind,
        fabric_host_linux::LinuxHostErrorKind::UnsupportedPlatform
    );
}
