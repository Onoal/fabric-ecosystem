use std::fs;

#[test]
fn process_resource_stays_systemd_and_worker_free() {
    let manifest =
        fs::read_to_string(format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"))).expect("manifest");
    assert!(!manifest.contains("fabric-host"));
    assert!(!manifest.contains("fabric-resource-worker"));

    for file in [
        "/src/lib.rs",
        "/src/contract.rs",
        "/src/error.rs",
        "/src/model.rs",
        "/src/native/adapter.rs",
        "/src/native/module.rs",
    ] {
        let source =
            fs::read_to_string(format!("{}{}", env!("CARGO_MANIFEST_DIR"), file)).expect("source");
        for forbidden in [
            "systemd",
            "Deno",
            "WorkerAdapter",
            "WorkerContract",
            "WorkerError",
            "PreparedWorkloadProjections",
        ] {
            assert!(
                !source.contains(forbidden),
                "{file} leaked removed process donor vocabulary: {forbidden}"
            );
        }
    }
}
