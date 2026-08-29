#[test]
fn deno_server_adapter_stays_server_specific() {
    let manifest = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
    )
    .expect("manifest");
    assert!(manifest.contains("fabric-resource-server"));
    assert!(!manifest.contains("fabric-adapter-worker-deno"));
    assert!(!manifest.contains("fabric-resource-worker"));
    assert!(!manifest.contains("fabric-resource-process"));

    for relative in [
        "src/lib.rs",
        "src/adapter/module.rs",
        "src/adapter/runtime.rs",
    ] {
        let file = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative),
        )
        .expect("read source");
        for forbidden in [
            "WorkerAdapter",
            "fabric_resource_worker",
            "ProcessAdapter",
            "bootstrap.ts",
            "module.fetch",
            "module.default",
            "__fabric_ready",
            "staged_root.join(\"main.ts\")",
        ] {
            assert!(
                !file.contains(forbidden),
                "{relative} should not contain {forbidden}"
            );
        }
    }
}
