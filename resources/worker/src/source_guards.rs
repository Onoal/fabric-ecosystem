use std::fs;

#[test]
fn canonical_worker_source_excludes_provider_and_product_material() {
    for file in [
        "/src/bindings/model.rs",
        "/src/compatibility.rs",
        "/src/contract.rs",
        "/src/model.rs",
        "/src/error.rs",
    ] {
        let source =
            fs::read_to_string(format!("{}{}", env!("CARGO_MANIFEST_DIR"), file)).expect("source");
        for forbidden in [
            "SqliteDatabaseMaterialization",
            "KvAccess",
            "KvRuntimeHandle",
            "ObjectStorageAccess",
            "DenoWorkerConfig",
            "SystemdProcessAdapter",
            "SystemdProcessSupervisor",
            "RuntimeKind",
            "workerd",
            "nucleus",
            "AppId",
            "InstalledAppId",
            "HomeId",
            "PathBuf",
        ] {
            assert!(
                !source.contains(forbidden),
                "{file} leaked non-canonical worker detail: {forbidden}"
            );
        }
    }
}

#[test]
fn worker_adapter_trait_does_not_author_prepared_worker_truth() {
    let source = fs::read_to_string(format!(
        "{}/src/native/adapter.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("source");
    assert!(
        !source.contains("-> Result<PreparedWorker, WorkerError>"),
        "worker adapter prepare must not return canonical PreparedWorker",
    );
}

#[test]
fn worker_candidate_source_no_longer_declares_process_feature_semantics() {
    for file in [
        "/ARCHITECTURE.md",
        "/src/lib.rs",
        "/src/model.rs",
        "/src/compatibility.rs",
        "/src/tests.rs",
    ] {
        let source =
            fs::read_to_string(format!("{}{}", env!("CARGO_MANIFEST_DIR"), file)).expect("source");
        assert!(
            !source.contains("process.exec"),
            "{file} must not retain process-specific worker feature residue",
        );
    }
}
