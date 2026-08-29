#[test]
fn resource_foundation_acceptance_stays_resource_only() {
    let manifest = std::fs::read_to_string(format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR")))
        .expect("read manifest");
    let dependencies = manifest
        .split_once("[dependencies]\n")
        .map(|(_, dependencies)| dependencies)
        .expect("manifest dependencies section");
    let dependency_keys: Vec<&str> = dependencies
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, _)| name.trim())
        .collect();
    for forbidden in [
        "fabric-component",
        "fabric-test-service-materialization",
        "fabric-auth",
        "fabric-apps",
        "fabric-home",
        "fabric-local-home",
        "steld",
        "fabric-client-contracts",
    ] {
        assert!(
            !dependency_keys.contains(&forbidden),
            "resource foundation acceptance must not depend on {forbidden}"
        );
    }
}

#[test]
fn resource_foundation_acceptance_sources_exclude_component_domains() {
    let src_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for relative in [
        "profiles/participation.rs",
        "profiles/extension.rs",
        "profiles/deno.rs",
    ] {
        let source = std::fs::read_to_string(src_root.join(relative)).expect("read source");
        for forbidden in [
            "fabric_component",
            "fabric_component_apps",
            "steld",
            "Publication",
            "Gateway",
            "Namespace",
            "NamespaceName",
            "LocalHome",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} leaked forbidden upper-layer term {forbidden}"
            );
        }
    }
}
