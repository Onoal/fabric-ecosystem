use std::fs;

#[test]
fn pingora_adapter_keeps_upper_layer_and_public_network_terms_out_of_semantic_seams() {
    let config = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/config.rs"))
        .expect("read config");
    let adapter_module = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/adapter/module.rs"
    ))
    .expect("read adapter module");
    let runtime = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/adapter/runtime.rs"
    ))
    .expect("read runtime");

    for forbidden in [
        "AppId",
        "InstalledAppId",
        "WorkerInstanceId",
        "Gateway",
        "Publication",
        "Namespace",
        "HomeId",
        "PlatformId",
        "0.0.0.0",
    ] {
        assert!(
            !config.contains(forbidden)
                && !adapter_module.contains(forbidden)
                && !runtime.contains(forbidden),
            "pingora adapter seam must not contain {forbidden}"
        );
    }

    assert!(!adapter_module.contains("ResourceRegistry"));
    assert!(!adapter_module.contains(".register("));
    assert!(!adapter_module.contains(".unregister("));
}
