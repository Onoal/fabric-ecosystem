use std::fs;

#[test]
fn manifest_depends_only_on_component_and_resource_seams() {
    let manifest =
        fs::read_to_string(format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"))).expect("manifest");
    let dependency_keys: Vec<&str> = manifest
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, _)| name.trim().split_once('.').map_or(name.trim(), |(base, _)| base))
        .collect();

    for required in [
        "fabric-core",
        "fabric-component",
        "fabric-resource-service",
        "fabric-resource-ingress",
        "fabric-resource-connectivity",
    ] {
        assert!(
            dependency_keys.contains(&required),
            "service materialization manifest must depend on {required}"
        );
    }

    for forbidden in [
        "fabric-auth",
        "fabric-apps",
        "fabric-home",
        "fabric-local-host",
        "steld",
        "fabric-client-contracts",
        "fabric-resource-identity",
        "fabric-resource-authority",
        "fabric-resource-secrets",
    ] {
        assert!(
            !dependency_keys.contains(&forbidden),
            "service materialization manifest must not depend on {forbidden}"
        );
    }
}

#[test]
fn source_stays_out_of_apps_auth_and_transport_policy() {
    for file in [
        "/src/contract.rs",
        "/src/error.rs",
        "/src/gateway.rs",
        "/src/lib.rs",
        "/src/local.rs",
        "/src/model.rs",
        "/src/naming.rs",
        "/src/native/mod.rs",
        "/src/native/module.rs",
    ] {
        let source =
            fs::read_to_string(format!("{}{}", env!("CARGO_MANIFEST_DIR"), file)).expect("source");
        for forbidden in [
            "Auth",
            "Apps",
            "Home",
            "LocalHost",
            "steld",
            "Ory",
            "BetterAuth",
            "SocketAddr",
            "TcpStream",
            "TcpListener",
            "Pingora",
            "hostname",
            "dns",
            "mDNS",
            "serde_json",
        ] {
            assert!(
                !source.contains(forbidden),
                "{file} leaked forbidden term {forbidden}"
            );
        }
    }
}
