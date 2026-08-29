use std::fs;

#[test]
fn authority_resource_stays_cedar_free_and_runtime_owned() {
    let manifest =
        fs::read_to_string(format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"))).expect("manifest");
    assert!(!manifest.contains("cedar-policy"));

    for file in [
        "/src/lib.rs",
        "/src/contract.rs",
        "/src/error.rs",
        "/src/model.rs",
        "/src/decision.rs",
        "/src/native/module.rs",
        "/src/native/persistence.rs",
    ] {
        let source =
            fs::read_to_string(format!("{}{}", env!("CARGO_MANIFEST_DIR"), file)).expect("source");
        for forbidden in [
            "cedar_policy",
            "Authorizer",
            "PolicySet",
            "EntityUid",
            "RestrictedExpression",
            "CedarAuthorityDecisionAdapter",
        ] {
            assert!(
                !source.contains(forbidden),
                "{file} leaked cedar machinery into authority resource source via {forbidden}"
            );
        }
    }
}
