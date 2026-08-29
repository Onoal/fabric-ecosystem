use std::path::PathBuf;

#[test]
fn server_resource_stays_runtime_only_and_adapter_neutral() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    for relative in [
        "lib.rs",
        "contract.rs",
        "error.rs",
        "http.rs",
        "model.rs",
        "native/adapter.rs",
        "native/module.rs",
    ] {
        let file = std::fs::read_to_string(src.join(relative)).expect("read source");
        for forbidden in [
            "Deno",
            "WorkerAdapter",
            "ProcessAdapter",
            "ServiceId",
            "Ingress",
            "Gateway",
        ] {
            assert!(
                !file.contains(forbidden),
                "{relative} should not contain {forbidden}"
            );
        }
    }
}
