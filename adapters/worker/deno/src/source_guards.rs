use std::fs;

#[test]
fn deno_runtime_source_no_longer_resolves_or_materializes_resources_directly() {
    for file in ["/src/adapter/module.rs", "/src/adapter/runtime.rs"] {
        let source =
            fs::read_to_string(format!("{}{}", env!("CARGO_MANIFEST_DIR"), file)).expect("source");
        for forbidden in [
            "PreparedWorker::new",
            "SqliteDatabaseCompatibilityContract",
            "materialize_sqlite",
            "KvContract",
            "KvAccess",
            "SecretsContract",
            "materialize_authorized",
        ] {
            assert!(
                !source.contains(forbidden),
                "{file} retained direct resource-resolution term: {forbidden}"
            );
        }
    }
}
