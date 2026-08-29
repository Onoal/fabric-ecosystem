#[test]
fn fv0_harness_stays_fabric_only_and_excludes_product_domains() {
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
        "fabric-auth",
        "fabric-home",
        "fabric-apps",
        "steld",
        "fabric-client",
        "host",
        "fabric-worker",
        "nucleus",
        "workerd",
        "rustfs",
        "object-storage",
        "pingora",
    ] {
        assert!(
            !dependency_keys.contains(&forbidden),
            "fv0 harness manifest must not depend on {forbidden}"
        );
    }

    let source = std::fs::read_to_string(format!("{}/src/scenario.rs", env!("CARGO_MANIFEST_DIR")))
        .expect("read scenario");
    for forbidden in [
        "use fabric_component_auth::",
        "use fabric_home::",
        "use fabric_component_apps::",
        "use steld::",
        "use fabric_client::",
        "Namespace",
        "Publication",
        "Gateway",
        "ObjectStorage",
        "RustFS",
        "workerd",
        "Nucleus",
        "LocalHost",
    ] {
        assert!(
            !source.contains(forbidden),
            "fv0 scenario must not depend on forbidden upper-layer or deferred term {forbidden}"
        );
    }
}

#[test]
fn foundation_manifests_do_not_depend_upward_or_on_removed_resource_plane() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root");

    for relative in [
        "resources/identity/Cargo.toml",
        "resources/authority/Cargo.toml",
        "resources/secrets/Cargo.toml",
        "resources/worker/Cargo.toml",
        "adapters/worker/deno/Cargo.toml",
        "resources/service/Cargo.toml",
        "resources/ingress/Cargo.toml",
        "adapters/ingress/pingora/Cargo.toml",
        "resources/connectivity/Cargo.toml",
    ] {
        let manifest =
            std::fs::read_to_string(repo_root.join(relative)).expect("read foundation manifest");
        let dependency_keys: Vec<&str> = manifest
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(name, _)| name.trim())
            .collect();
        for forbidden in [
            "fabric-auth",
            "fabric-apps",
            "fabric-home",
            "fabric-local-host",
            "steld",
            "fabric-client-contracts",
            "fabric-resource-plane",
        ] {
            assert!(
                !dependency_keys.contains(&forbidden),
                "{relative} must not depend upward on {forbidden}"
            );
        }
    }
}

#[test]
fn generic_fabric_kernel_stays_external_to_fabric_packages() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root");

    for forbidden in [
        "core",
        "host",
        "sdk",
        "components/component",
        "resources/resource",
        "resources/binding",
        "resources/projection",
        "resources/registry",
    ] {
        assert!(
            !repo_root.join(forbidden).exists(),
            "fabric-packages must not copy generic kernel source at {forbidden}"
        );
    }

    let root_manifest =
        std::fs::read_to_string(repo_root.join("Cargo.toml")).expect("read root manifest");
    for required in [
        "git = \"https://github.com/Onoal/fabric.git\"",
        "fabric-core = { git = \"https://github.com/Onoal/fabric.git\"",
        "fabric-resource = { git = \"https://github.com/Onoal/fabric.git\"",
        "fabric-component = { git = \"https://github.com/Onoal/fabric.git\"",
    ] {
        assert!(
            root_manifest.contains(required),
            "root workspace must source generic kernel dependency {required}"
        );
    }
}
