use std::fs;

#[test]
fn process_runtime_source_stays_resource_independent() {
    for file in [
        "/src/adapter/module.rs",
        "/src/adapter/runtime.rs",
        "/src/adapter/systemd.rs",
    ] {
        let source =
            fs::read_to_string(format!("{}{}", env!("CARGO_MANIFEST_DIR"), file)).expect("source");
        for forbidden in [
            "DatabaseRef",
            "KvRef",
            "SecretRef",
            "ServiceId",
            "ResourceDescriptor",
            "WorkerContract",
            "WorkerAdapter",
            "PreparedWorker::new",
            "process.exec",
        ] {
            assert!(
                !source.contains(forbidden),
                "{file} leaked non-process semantic detail: {forbidden}"
            );
        }
    }
}

#[test]
fn process_adapter_declares_host_requirement_without_host_service_locator() {
    let module = fs::read_to_string(format!(
        "{}/src/adapter/module.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("module source");
    let manifest =
        fs::read_to_string(format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"))).expect("manifest");

    assert!(manifest.contains("fabric-host"));
    assert!(manifest.contains("fabric-resource-process"));
    assert!(!manifest.contains("fabric-resource-worker"));
    assert!(module.contains("pub fn host_requirement() -> HostRequirement"));
    assert!(module.contains("pub fn for_host("));
    assert!(
        !module.contains("pub fn new("),
        "process adapter must not expose an unsatisfiable native-host constructor"
    );
    assert!(
        !module.contains("HostDescriptor::native()"),
        "process adapter construction must require an explicitly enriched host descriptor"
    );
    for forbidden in ["HostRegistry", "HostContext::get", "dyn Any"] {
        assert!(
            !module.contains(forbidden),
            "process host boundary must not recreate ambient host access via {forbidden}"
        );
    }
}
