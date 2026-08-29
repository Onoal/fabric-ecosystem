use std::fs;

#[test]
fn workload_projection_specialization_stays_worker_owned_and_runtime_agnostic() {
    for file in [
        "/src/projection/contract.rs",
        "/src/projection/environment.rs",
        "/src/projection/model.rs",
    ] {
        let source =
            fs::read_to_string(format!("{}{}", env!("CARGO_MANIFEST_DIR"), file)).expect("source");
        for forbidden in ["Deno", "Command::new", "std::process::Command"] {
            assert!(
                !source.contains(forbidden),
                "{file} leaked runtime-specific term: {forbidden}"
            );
        }
    }
}

#[test]
fn service_projection_stays_service_dispatch_based() {
    let source = fs::read_to_string(format!(
        "{}/src/projection/native/service.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("service projection source");
    for forbidden in [
        "resolve_live_endpoint(",
        "IngressContract",
        "Reachability",
        "Publication",
    ] {
        assert!(
            !source.contains(forbidden),
            "service projection leaked exposure-specific term: {forbidden}"
        );
    }
}
