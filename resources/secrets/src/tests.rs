use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use fabric_adapter_authority_cedar::CedarAuthorityDecisionAdapter;
use rusqlite::{Connection, params};
use tempfile::TempDir;

use crate::contract::SecretsContract;
use crate::model::{
    SecretMaterial, secret_delete_action, secret_materialize_action, secret_rotate_action,
};
use crate::native::module::{NativeSecrets, NativeSecretsConfig};
use crate::native::persistence::{PersistenceFailpoint, SecretsDatabase};
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
};
use fabric_resource_authority::{
    ActorRef, AuthorityContract, AuthorityDecision, AuthorityRequest, NativeAuthority,
    NativeAuthorityConfig, RequestContext,
};

struct Harness {
    _tempdir: TempDir,
    authority_db: PathBuf,
    secrets_db: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let tempdir = TempDir::new().expect("tempdir");
        Self {
            authority_db: tempdir.path().join("authority.sqlite"),
            secrets_db: tempdir.path().join("secrets.sqlite"),
            _tempdir: tempdir,
        }
    }

    fn contracts(&self) -> CapturedContracts {
        captured_contracts(&self.authority_db, &self.secrets_db)
    }
}

#[test]
fn authless_authority_and_secrets_composition_survives_rotation_and_delete() {
    let harness = Harness::new();
    let first = harness.contracts();
    let actor_a = ActorRef::new("system:owner-a").expect("actor a");
    let actor_b = ActorRef::new("system:reader-b").expect("actor b");
    let scope = first.authority.create_scope().expect("scope");
    first
        .authority
        .grant_scope_control(&actor_a, &scope)
        .expect("scope control");

    let created = first
        .secrets
        .create(&actor_a, &scope, SecretMaterial::new("top-secret-v1"))
        .expect("create secret");
    let materialized = first
        .secrets
        .materialize(&actor_a, &created.secret)
        .expect("owner materialize");
    assert_eq!(materialized.secret.id, created.secret.id);
    assert_eq!(materialized.version, created.version);
    assert_eq!(materialized.value.expose(), "top-secret-v1");

    assert_eq!(
        first.secrets.materialize(&actor_b, &created.secret),
        Err(crate::SecretsError::AccessDenied)
    );

    first
        .authority
        .grant_action(
            &actor_b,
            &secret_materialize_action(),
            &created
                .secret
                .authority_resource()
                .expect("secret resource"),
        )
        .expect("grant exact materialize");
    assert_eq!(
        first
            .secrets
            .materialize(&actor_b, &created.secret)
            .expect("reader materialize")
            .value
            .expose(),
        "top-secret-v1"
    );
    assert_eq!(
        first
            .secrets
            .rotate(&actor_b, &created.secret, SecretMaterial::new("reader-v2")),
        Err(crate::SecretsError::AccessDenied)
    );
    assert_eq!(
        first.secrets.delete(&actor_b, &created.secret),
        Err(crate::SecretsError::AccessDenied)
    );

    let rotated = first
        .secrets
        .rotate(
            &actor_a,
            &created.secret,
            SecretMaterial::new("top-secret-v2"),
        )
        .expect("owner rotate");
    assert_eq!(created.secret.id, created.secret.id);
    assert!(rotated.as_i64() > created.version.as_i64());
    assert_eq!(
        first
            .secrets
            .materialize(&actor_a, &created.secret)
            .expect("owner materialize after rotate")
            .value
            .expose(),
        "top-secret-v2"
    );
    first.stop();

    let restarted = harness.contracts();
    assert_eq!(
        restarted
            .secrets
            .materialize(&actor_b, &created.secret)
            .expect("reader after restart")
            .value
            .expose(),
        "top-secret-v2"
    );
    restarted
        .secrets
        .delete(&actor_a, &created.secret)
        .expect("owner delete");
    assert_eq!(
        restarted.secrets.materialize(&actor_a, &created.secret),
        Err(crate::SecretsError::NotFound)
    );
    restarted.stop();

    let after_delete = harness.contracts();
    assert_eq!(
        after_delete.secrets.materialize(&actor_b, &created.secret),
        Err(crate::SecretsError::NotFound)
    );
}

#[test]
fn genesis_schema_reopens_and_rejects_incompatible_state() {
    let harness = Harness::new();
    SecretsDatabase::open(&harness.secrets_db).expect("install genesis");
    SecretsDatabase::open(&harness.secrets_db).expect("reopen genesis");

    let legacy_path = harness._tempdir.path().join("legacy-secrets.sqlite");
    Connection::open(&legacy_path)
        .expect("open legacy fixture")
        .execute_batch(
            "CREATE TABLE secrets_schema (singleton_key INTEGER PRIMARY KEY, schema_version INTEGER NOT NULL);
             INSERT INTO secrets_schema VALUES (1, 1);
             CREATE TABLE secrets (secret_id TEXT PRIMARY KEY, scope_id TEXT NOT NULL);",
        )
        .expect("write legacy fixture");
    assert!(matches!(
        SecretsDatabase::open(&legacy_path),
        Err(crate::SecretsError::Integrity { .. })
    ));
}

#[test]
fn secrets_boundary_depends_only_on_authority_and_redacts_material() {
    let public_api = include_str!("lib.rs");
    let model = include_str!("model.rs");
    let manifest = include_str!("../Cargo.toml");

    assert!(!public_api.contains("SecretGrant"));
    assert!(!model.contains("SecretGrant"));
    assert!(manifest.contains("fabric-resource-authority.workspace = true"));
    assert!(manifest.contains("fabric-core.workspace = true"));
    for forbidden in [
        "fabric-resource-identity =",
        "fabric-auth =",
        "fabric-home =",
        "fabric-apps =",
        "foundations =",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "secrets must not depend on {forbidden}"
        );
    }
    let secret = SecretMaterial::new("super-secret");
    assert_eq!(format!("{secret:?}"), "SecretMaterial([REDACTED])");
    assert_eq!(format!("{secret}"), "[REDACTED]");
}

#[test]
fn exact_materialize_grant_does_not_imply_rotate_or_delete() {
    let harness = Harness::new();
    let contracts = harness.contracts();
    let actor_a = ActorRef::new("system:owner-a").expect("actor a");
    let actor_b = ActorRef::new("system:reader-b").expect("actor b");
    let scope = contracts.authority.create_scope().expect("scope");
    contracts
        .authority
        .grant_scope_control(&actor_a, &scope)
        .expect("scope control");
    let created = contracts
        .secrets
        .create(&actor_a, &scope, SecretMaterial::new("v1"))
        .expect("create");
    let resource = created.secret.authority_resource().expect("resource");
    contracts
        .authority
        .grant_action(&actor_b, &secret_materialize_action(), &resource)
        .expect("materialize grant");

    assert_eq!(
        contracts
            .secrets
            .materialize(&actor_b, &created.secret)
            .expect("materialize")
            .value
            .expose(),
        "v1"
    );
    assert_eq!(
        contracts
            .authority
            .authorize(&AuthorityRequest {
                actor: actor_b.clone(),
                action: secret_rotate_action(),
                resource: resource.clone(),
                context: RequestContext::default(),
            })
            .expect("rotate decision"),
        AuthorityDecision::Deny
    );
    assert_eq!(
        contracts
            .authority
            .authorize(&AuthorityRequest {
                actor: actor_b,
                action: secret_delete_action(),
                resource,
                context: RequestContext::default(),
            })
            .expect("delete decision"),
        AuthorityDecision::Deny
    );
}

#[test]
fn create_failure_between_metadata_and_version_is_atomic_and_retryable() {
    let harness = Harness::new();
    let contracts = harness.contracts();
    let actor = ActorRef::new("system:owner-a").expect("actor");
    let scope = contracts.authority.create_scope().expect("scope");
    contracts
        .authority
        .grant_scope_control(&actor, &scope)
        .expect("scope control");
    contracts
        .secrets
        .inject_failpoint(PersistenceFailpoint::CreateAfterMetadataInsert)
        .expect("inject create failpoint");

    let failure = contracts
        .secrets
        .create(&actor, &scope, SecretMaterial::new("top-secret-v1"))
        .expect_err("injected create failure");
    assert!(matches!(failure, crate::SecretsError::Persistence { .. }));
    assert_eq!(count_rows(&harness.secrets_db, "secrets"), 0);
    assert_eq!(count_rows(&harness.secrets_db, "secret_versions"), 0);
    contracts.stop();

    let restarted = harness.contracts();
    assert_eq!(count_rows(&harness.secrets_db, "secrets"), 0);
    assert_eq!(count_rows(&harness.secrets_db, "secret_versions"), 0);
    let created = restarted
        .secrets
        .create(&actor, &scope, SecretMaterial::new("top-secret-v1"))
        .expect("retry create");
    assert_eq!(created.version.as_i64(), 1);
    assert_eq!(count_rows(&harness.secrets_db, "secrets"), 1);
    assert_eq!(count_rows(&harness.secrets_db, "secret_versions"), 1);
    assert_eq!(
        restarted
            .secrets
            .materialize(&actor, &created.secret)
            .expect("materialize after retry")
            .value
            .expose(),
        "top-secret-v1"
    );
}

#[test]
fn rotate_failure_between_version_insert_and_current_pointer_is_atomic_and_retryable() {
    let harness = Harness::new();
    let contracts = harness.contracts();
    let actor = ActorRef::new("system:owner-a").expect("actor");
    let scope = contracts.authority.create_scope().expect("scope");
    contracts
        .authority
        .grant_scope_control(&actor, &scope)
        .expect("scope control");
    let created = contracts
        .secrets
        .create(&actor, &scope, SecretMaterial::new("top-secret-v1"))
        .expect("create");
    contracts
        .secrets
        .inject_failpoint(PersistenceFailpoint::RotateAfterVersionInsert)
        .expect("inject rotate failpoint");

    let failure = contracts
        .secrets
        .rotate(
            &actor,
            &created.secret,
            SecretMaterial::new("top-secret-v2"),
        )
        .expect_err("injected rotate failure");
    assert!(matches!(failure, crate::SecretsError::Persistence { .. }));
    assert_eq!(
        current_version(&harness.secrets_db, &created.secret),
        created.version.as_i64()
    );
    assert_eq!(count_versions(&harness.secrets_db, &created.secret), 1);
    assert_eq!(
        contracts
            .secrets
            .materialize(&actor, &created.secret)
            .expect("old material remains current")
            .value
            .expose(),
        "top-secret-v1"
    );
    contracts.stop();

    let restarted = harness.contracts();
    assert_eq!(
        current_version(&harness.secrets_db, &created.secret),
        created.version.as_i64()
    );
    assert_eq!(count_versions(&harness.secrets_db, &created.secret), 1);
    assert_eq!(
        restarted
            .secrets
            .materialize(&actor, &created.secret)
            .expect("old material after restart")
            .value
            .expose(),
        "top-secret-v1"
    );
    let rotated = restarted
        .secrets
        .rotate(
            &actor,
            &created.secret,
            SecretMaterial::new("top-secret-v2"),
        )
        .expect("retry rotate");
    assert_eq!(rotated.as_i64(), created.version.as_i64() + 1);
    assert_eq!(
        current_version(&harness.secrets_db, &created.secret),
        rotated.as_i64()
    );
    assert_eq!(count_versions(&harness.secrets_db, &created.secret), 2);
    assert_eq!(
        restarted
            .secrets
            .materialize(&actor, &created.secret)
            .expect("new material after retry")
            .value
            .expose(),
        "top-secret-v2"
    );
}

struct CapturedContracts {
    composition: fabric_core::Instance,
    authority: AuthorityContract,
    secrets: SecretsContract,
}

impl CapturedContracts {
    fn stop(mut self) {
        self.composition.stop();
    }
}

fn captured_contracts(authority_path: &Path, secrets_path: &Path) -> CapturedContracts {
    #[derive(Default)]
    struct Slots {
        authority: Mutex<Option<AuthorityContract>>,
        secrets: Mutex<Option<SecretsContract>>,
    }

    #[derive(Clone)]
    struct Capture {
        module_id: ModuleId,
        authority_requirement: ContractRequirement<AuthorityContract>,
        secrets_requirement: ContractRequirement<SecretsContract>,
        slots: Arc<Slots>,
    }

    impl ModuleRuntime for Capture {
        fn id(&self) -> &ModuleId {
            &self.module_id
        }

        fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
            Vec::new()
                .into_iter()
                .map(fabric_core::ProvidedContractDeclaration::provisional)
                .collect()
        }

        fn required_contract_declarations(
            &self,
        ) -> Vec<fabric_core::ContractRequirementDeclaration> {
            vec![
                self.authority_requirement.id().clone(),
                self.secrets_requirement.id().clone(),
            ]
            .into_iter()
            .map(fabric_core::ContractRequirementDeclaration::provisional)
            .collect()
        }

        fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
            Ok(Vec::new())
        }

        fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
            let authority = bindings
                .resolve(&self.authority_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?;
            let secrets = bindings
                .resolve(&self.secrets_requirement)
                .map_err(|error| ModuleError::new(error.to_string()))?;
            self.slots
                .authority
                .lock()
                .expect("authority")
                .replace(authority.as_ref().clone());
            self.slots
                .secrets
                .lock()
                .expect("secrets")
                .replace(secrets.as_ref().clone());
            Ok(())
        }

        fn initialize(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }

        fn start(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }

        fn stop(&mut self) {}

        fn health(&self) -> Health {
            Health::Healthy
        }
    }

    let slots = Arc::new(Slots::default());
    let authority_block = BlockBuilder::new(BlockId::new("test.authority").expect("block"))
        .register_module(NativeAuthority::with_decision_adapter(
            NativeAuthorityConfig {
                database_path: authority_path.to_path_buf(),
            },
            Arc::new(CedarAuthorityDecisionAdapter::new()),
        ))
        .build();
    let secrets_block = BlockBuilder::new(BlockId::new("test.secrets").expect("block"))
        .register_module(NativeSecrets::new(NativeSecretsConfig {
            database_path: secrets_path.to_path_buf(),
        }))
        .build();
    let capture_block = BlockBuilder::new(BlockId::new("test.capture").expect("block"))
        .register_module(Capture {
            module_id: ModuleId::new("test.secrets.consumer").expect("module"),
            authority_requirement: ContractRequirement::provisional(
                fabric_resource_authority::authority_contract_id(),
            ),
            secrets_requirement: ContractRequirement::provisional(crate::secrets_contract_id()),
            slots: Arc::clone(&slots),
        })
        .build();

    let composition =
        CompositionBuilder::new(CompositionId::new("test.secrets.sec0").expect("composition"))
            .register_block(authority_block)
            .register_block(secrets_block)
            .register_block(capture_block)
            .build()
            .expect("build composition");
    let mut composition = composition
        .materialize(fabric_core::InstanceId::new("test.secrets.sec0").expect("instance id"))
        .expect("materialize composition");
    composition.start().expect("start composition");

    CapturedContracts {
        authority: slots
            .authority
            .lock()
            .expect("authority")
            .clone()
            .expect("captured authority"),
        secrets: slots
            .secrets
            .lock()
            .expect("secrets")
            .clone()
            .expect("captured secrets"),
        composition,
    }
}

fn count_rows(database_path: &Path, table: &str) -> i64 {
    let connection = Connection::open(database_path).expect("open secrets database");
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count rows")
}

fn current_version(database_path: &Path, secret: &crate::model::SecretRef) -> i64 {
    let connection = Connection::open(database_path).expect("open secrets database");
    connection
        .query_row(
            "SELECT current_version FROM secrets WHERE secret_id = ?1",
            [secret.id.as_str()],
            |row| row.get(0),
        )
        .expect("load current version")
}

fn count_versions(database_path: &Path, secret: &crate::model::SecretRef) -> i64 {
    let connection = Connection::open(database_path).expect("open secrets database");
    connection
        .query_row(
            "SELECT COUNT(*) FROM secret_versions WHERE secret_id = ?1",
            params![secret.id.as_str()],
            |row| row.get(0),
        )
        .expect("count versions")
}
