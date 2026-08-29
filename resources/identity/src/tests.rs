use std::path::PathBuf;

use rusqlite::Connection;
use tempfile::TempDir;

use crate::native::persistence::IdentityDatabase;

struct Harness {
    _tempdir: TempDir,
    database_path: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let tempdir = TempDir::new().expect("tempdir");
        Self {
            database_path: tempdir.path().join("identity.sqlite"),
            _tempdir: tempdir,
        }
    }
}

#[test]
fn principal_exists_without_authentication() {
    let harness = Harness::new();
    let mut database = IdentityDatabase::open(&harness.database_path).expect("open");

    let principal = database.create_principal().expect("create principal");

    let loaded = database
        .get_principal(&principal.id)
        .expect("load principal")
        .expect("principal exists");
    assert_eq!(loaded, principal);
}

#[test]
fn principal_persists_across_reopen() {
    let harness = Harness::new();
    let principal = {
        let mut database = IdentityDatabase::open(&harness.database_path).expect("open");
        database.create_principal().expect("create principal")
    };

    let reopened = IdentityDatabase::open(&harness.database_path).expect("reopen");
    let loaded = reopened
        .get_principal(&principal.id)
        .expect("load principal")
        .expect("principal exists");
    assert_eq!(loaded, principal);
}

#[test]
fn genesis_schema_reopens_and_rejects_pre_ia0_metadata() {
    let harness = Harness::new();
    let principal = {
        let mut database = IdentityDatabase::open(&harness.database_path).expect("install genesis");
        database.create_principal().expect("create principal")
    };
    let reopened = IdentityDatabase::open(&harness.database_path).expect("reopen genesis");
    assert!(
        reopened
            .get_principal(&principal.id)
            .expect("load")
            .is_some()
    );

    let legacy_path = harness._tempdir.path().join("legacy-identity.sqlite");
    Connection::open(&legacy_path)
        .expect("open legacy fixture")
        .execute_batch(
            "CREATE TABLE identity_schema (singleton_key INTEGER PRIMARY KEY, schema_version INTEGER NOT NULL);
             INSERT INTO identity_schema VALUES (1, 1);
             CREATE TABLE principals (principal_id TEXT PRIMARY KEY, display_name TEXT NOT NULL);",
        )
        .expect("write legacy fixture");
    assert!(matches!(
        IdentityDatabase::open(&legacy_path),
        Err(crate::IdentityError::Integrity { .. })
    ));
}

#[test]
fn caller_cannot_invent_principal_id() {
    let harness = Harness::new();
    let database = IdentityDatabase::open(&harness.database_path).expect("open");
    let fabricated = crate::PrincipalId::parse("invented-principal").expect("principal id");
    assert_eq!(database.get_principal(&fabricated).expect("lookup"), None);
}

#[test]
fn identity_model_and_dependencies_remain_product_and_credential_free() {
    let model = include_str!("model.rs");
    let manifest = include_str!("../Cargo.toml");

    for forbidden in ["username", "password", "session"] {
        assert!(
            !model.contains(forbidden),
            "identity model must not contain {forbidden} semantics"
        );
    }
    for forbidden in ["fabric-home", "fabric-apps", "fabric-devices"] {
        assert!(
            !manifest.contains(forbidden),
            "identity must not depend on {forbidden}"
        );
    }
}
