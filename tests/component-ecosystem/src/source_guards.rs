use std::fs;
use std::path::Path;

const OLD_KERNEL_SHA: &str = "48e731f6a6bb4f27dfcdc90fce5804e091945251";
const NEW_KERNEL_SHA: &str = "32d087ebd59314fe48e346601c38a6b795debbf2";

#[test]
fn component_ecosystem_crate_documents_cross_package_ownership() {
    let source = fs::read_to_string(format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR")))
        .expect("component ecosystem lib");
    assert!(source.contains("Cross-package ecosystem regressions"));
    assert!(source.contains("generic `fabric-component` kernel crate owns generic component laws"));
}

#[test]
fn root_workspace_repins_generic_fabric_to_rb1_kernel() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root");
    let manifest = fs::read_to_string(repo_root.join("Cargo.toml")).expect("root manifest");

    for required in [
        format!(
            "fabric-core = {{ git = \"https://github.com/Onoal/fabric.git\", rev = \"{NEW_KERNEL_SHA}\" }}"
        ),
        format!(
            "fabric-host = {{ git = \"https://github.com/Onoal/fabric.git\", rev = \"{NEW_KERNEL_SHA}\" }}"
        ),
        format!(
            "fabric-resource = {{ git = \"https://github.com/Onoal/fabric.git\", rev = \"{NEW_KERNEL_SHA}\" }}"
        ),
        format!(
            "fabric-binding = {{ git = \"https://github.com/Onoal/fabric.git\", rev = \"{NEW_KERNEL_SHA}\" }}"
        ),
        format!(
            "fabric-projection = {{ git = \"https://github.com/Onoal/fabric.git\", rev = \"{NEW_KERNEL_SHA}\" }}"
        ),
        format!(
            "fabric-resource-registry = {{ git = \"https://github.com/Onoal/fabric.git\", rev = \"{NEW_KERNEL_SHA}\" }}"
        ),
        format!(
            "fabric-component = {{ git = \"https://github.com/Onoal/fabric.git\", rev = \"{NEW_KERNEL_SHA}\" }}"
        ),
    ] {
        assert!(
            manifest.contains(&required),
            "root workspace must contain {required}"
        );
    }

    assert!(
        !manifest.contains(OLD_KERNEL_SHA),
        "old kernel sha must not remain in active workspace dependencies"
    );
    assert!(!manifest.contains("../fabric"));
}

#[test]
fn child_manifests_do_not_pin_their_own_fabric_kernel_or_use_path_vendoring() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root");
    let members = [
        "resources/identity/Cargo.toml",
        "resources/authority/Cargo.toml",
        "resources/secrets/Cargo.toml",
        "resources/worker/Cargo.toml",
        "resources/server/Cargo.toml",
        "resources/process/Cargo.toml",
        "resources/database/Cargo.toml",
        "resources/kv/Cargo.toml",
        "resources/service/Cargo.toml",
        "resources/ingress/Cargo.toml",
        "resources/connectivity/Cargo.toml",
        "adapters/authority/cedar/Cargo.toml",
        "adapters/worker/deno/Cargo.toml",
        "adapters/server/deno/Cargo.toml",
        "adapters/process/systemd/Cargo.toml",
        "adapters/database/sqlite/Cargo.toml",
        "adapters/kv/fjall/Cargo.toml",
        "adapters/ingress/pingora/Cargo.toml",
        "components/namespace/Cargo.toml",
        "components/publication/Cargo.toml",
        "components/gateway/Cargo.toml",
        "tests/resource-foundation/Cargo.toml",
        "tests/fabric-vertical/Cargo.toml",
        "tests/service-materialization/Cargo.toml",
        "tests/component-ecosystem/Cargo.toml",
    ];

    for member in members {
        let manifest = fs::read_to_string(repo_root.join(member)).expect("member manifest");
        assert!(
            !manifest.contains("git = \"https://github.com/Onoal/fabric.git\""),
            "{member} must inherit generic fabric dependencies from workspace"
        );
        assert!(
            !manifest.contains(OLD_KERNEL_SHA),
            "{member} leaked old kernel sha"
        );
        assert!(
            !manifest.contains("../fabric"),
            "{member} must not vendor the sibling fabric checkout"
        );
    }
}

#[test]
fn active_source_excludes_stel_and_deleted_compute_vocabulary() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root");

    for relative in [
        "resources/worker/src/lib.rs",
        "resources/server/src/lib.rs",
        "resources/process/src/lib.rs",
        "tests/fabric-vertical/src/lib.rs",
        "tests/service-materialization/src/lib.rs",
        "tests/component-ecosystem/src/lib.rs",
    ] {
        let source = fs::read_to_string(repo_root.join(relative)).expect("read source");
        for forbidden in [
            "ComputeContract",
            "NativeCompute",
            "DenoComputeAdapter",
            "fabric.resource.compute",
            "stel-org/stel",
            "platform/apps",
            "composition/home",
            "steld",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} leaked forbidden term {forbidden}"
            );
        }
    }
}
