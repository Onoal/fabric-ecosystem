use std::fs;

#[test]
fn cedar_adapter_depends_on_authority_without_redefining_semantic_ids() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root");
    let manifest =
        fs::read_to_string(format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"))).expect("manifest");
    let lib = fs::read_to_string(format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR")))
        .expect("adapter lib source");
    let adapter = fs::read_to_string(format!("{}/src/adapter.rs", env!("CARGO_MANIFEST_DIR")))
        .expect("adapter source");
    let root_manifest = fs::read_to_string(repo_root.join("Cargo.toml")).expect("root manifest");
    let worker_manifest =
        fs::read_to_string(repo_root.join("resources/worker/Cargo.toml")).expect("worker manifest");
    let process_manifest = fs::read_to_string(repo_root.join("resources/process/Cargo.toml"))
        .expect("process manifest");

    assert!(manifest.contains("cedar-policy"));
    assert!(manifest.contains("fabric-resource-authority"));
    assert!(
        root_manifest.contains("fabric-core = { git = \"https://github.com/Onoal/fabric.git\"")
    );
    assert!(!worker_manifest.contains("cedar-policy"));
    assert!(!process_manifest.contains("cedar-policy"));
    assert!(lib.contains("CedarAuthorityDecisionAdapter"));
    assert!(!adapter.contains("fabric.resource.authority\""));
    assert!(!adapter.contains("authority_contract_id("));
}
