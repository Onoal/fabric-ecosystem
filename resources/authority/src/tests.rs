use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tempfile::TempDir;

use crate::contract::AuthorityContract;
use crate::decision::{AuthorityDecisionAdapter, AuthorityEvaluationInput, AuthorityGrant};
use crate::model::{
    ActionId, ActorRef, AuthorityDecision, AuthorityRequest, RequestContext, ResourceRef,
};
use crate::native::module::{NativeAuthority, NativeAuthorityConfig};
use crate::native::persistence::AuthorityDatabase;
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
};

struct Harness {
    _tempdir: TempDir,
    authority_db: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let tempdir = TempDir::new().expect("tempdir");
        Self {
            authority_db: tempdir.path().join("authority.sqlite"),
            _tempdir: tempdir,
        }
    }

    fn contracts(&self) -> CapturedContracts {
        captured_contracts(&self.authority_db, Arc::new(FakeDecisionAdapter))
    }
}

#[test]
fn fake_decision_adapter_preserves_default_deny_and_scope_bounded_semantics() {
    let harness = Harness::new();
    let contracts = harness.contracts();
    let actor_a = ActorRef::new("system:test-a").expect("actor a");
    let actor_b = ActorRef::new("system:test-b").expect("actor b");
    let scope_x = contracts.authority.create_scope().expect("scope x");
    let scope_y = contracts.authority.create_scope().expect("scope y");
    let resource_x = ResourceRef::new(scope_x.clone(), "home", "resource-x").expect("resource x");
    let resource_y = ResourceRef::new(scope_y.clone(), "home", "resource-y").expect("resource y");
    contracts
        .authority
        .grant_scope_control(&actor_a, &scope_x)
        .expect("grant scope control");

    let read = ActionId::new("test.read".to_owned()).expect("action");
    let write = ActionId::new("test.write".to_owned()).expect("action");
    assert_eq!(
        contracts
            .authority
            .authorize(&AuthorityRequest {
                actor: actor_a.clone(),
                action: read.clone(),
                resource: resource_x.clone(),
                context: RequestContext::default(),
            })
            .expect("check"),
        AuthorityDecision::Allow
    );
    assert_eq!(
        contracts
            .authority
            .authorize(&AuthorityRequest {
                actor: actor_a,
                action: write,
                resource: resource_x,
                context: RequestContext::default(),
            })
            .expect("check"),
        AuthorityDecision::Allow
    );
    assert_eq!(
        contracts
            .authority
            .authorize(&AuthorityRequest {
                actor: ActorRef::new("system:test-a").expect("actor"),
                action: read.clone(),
                resource: resource_y,
                context: RequestContext::default(),
            })
            .expect("check"),
        AuthorityDecision::Deny
    );
    assert_eq!(
        contracts
            .authority
            .authorize(&AuthorityRequest {
                actor: actor_b,
                action: read,
                resource: ResourceRef::new(scope_x, "home", "resource-x").expect("resource"),
                context: RequestContext::default(),
            })
            .expect("check"),
        AuthorityDecision::Deny
    );
}

#[test]
fn fake_decision_adapter_preserves_exact_grant_semantics() {
    let harness = Harness::new();
    let contracts = harness.contracts();
    let actor_a = ActorRef::new("system:test-a").expect("actor a");
    let actor_b = ActorRef::new("system:test-b").expect("actor b");
    let scope_x = contracts.authority.create_scope().expect("scope x");
    let resource_x = ResourceRef::new(scope_x.clone(), "home", "resource-x").expect("resource x");
    contracts
        .authority
        .grant_scope_control(&actor_a, &scope_x)
        .expect("grant owner");
    let read = ActionId::new("test.read".to_owned()).expect("read");
    let write = ActionId::new("test.write".to_owned()).expect("write");
    contracts
        .authority
        .grant_action(&actor_b, &read, &resource_x)
        .expect("grant exact action");

    assert_eq!(
        contracts
            .authority
            .authorize(&AuthorityRequest {
                actor: actor_b.clone(),
                action: read,
                resource: resource_x.clone(),
                context: RequestContext::default(),
            })
            .expect("check"),
        AuthorityDecision::Allow
    );
    assert_eq!(
        contracts
            .authority
            .authorize(&AuthorityRequest {
                actor: actor_b,
                action: write,
                resource: resource_x,
                context: RequestContext::default(),
            })
            .expect("check"),
        AuthorityDecision::Deny
    );
}

#[test]
fn native_authority_runtime_works_with_fake_non_cedar_decision_adapter() {
    let harness = Harness::new();
    let contracts = harness.contracts();
    let actor = ActorRef::new("system:fake").expect("actor");
    let scope = contracts.authority.create_scope().expect("scope");
    let resource = ResourceRef::new(scope.clone(), "service", "resource").expect("resource");
    let action = ActionId::new("test.read").expect("action");

    contracts
        .authority
        .grant_scope_control(&actor, &scope)
        .expect("grant scope control");
    contracts
        .authority
        .grant_action(&actor, &action, &resource)
        .expect("grant exact action");

    assert_eq!(
        contracts
            .authority
            .authorize(&AuthorityRequest {
                actor,
                action,
                resource,
                context: RequestContext::default(),
            })
            .expect("authorize through fake adapter"),
        AuthorityDecision::Allow
    );
}

#[test]
fn authority_starts_without_identity_and_persists_grants_across_restart() {
    let harness = Harness::new();
    let first = harness.contracts();
    let actor_a = ActorRef::new("system:test-a").expect("actor a");
    let actor_b = ActorRef::new("system:test-b").expect("actor b");
    let scope = first.authority.create_scope().expect("scope");
    let resource = ResourceRef::new(scope.clone(), "service", "resource").expect("resource");
    let read = ActionId::new("test.read").expect("read");
    let write = ActionId::new("test.write").expect("write");

    first
        .authority
        .grant_scope_control(&actor_a, &scope)
        .expect("scope control");
    first
        .authority
        .grant_action(&actor_b, &read, &resource)
        .expect("exact grant");

    assert_eq!(
        first
            .authority
            .authorize(&AuthorityRequest {
                actor: actor_a.clone(),
                action: write.clone(),
                resource: resource.clone(),
                context: RequestContext::default(),
            })
            .expect("a allow"),
        AuthorityDecision::Allow
    );
    assert_eq!(
        first
            .authority
            .authorize(&AuthorityRequest {
                actor: actor_b.clone(),
                action: write.clone(),
                resource: resource.clone(),
                context: RequestContext::default(),
            })
            .expect("b write deny"),
        AuthorityDecision::Deny
    );
    assert_eq!(
        first
            .authority
            .authorize(&AuthorityRequest {
                actor: actor_b.clone(),
                action: read.clone(),
                resource: resource.clone(),
                context: RequestContext::default(),
            })
            .expect("b read allow"),
        AuthorityDecision::Allow
    );
    first.stop();

    let restarted = harness.contracts();
    assert_eq!(
        restarted
            .authority
            .authorize(&AuthorityRequest {
                actor: actor_a,
                action: write,
                resource: resource.clone(),
                context: RequestContext::default(),
            })
            .expect("a allow after restart"),
        AuthorityDecision::Allow
    );
    assert_eq!(
        restarted
            .authority
            .authorize(&AuthorityRequest {
                actor: actor_b.clone(),
                action: read,
                resource: resource.clone(),
                context: RequestContext::default(),
            })
            .expect("b read allow after restart"),
        AuthorityDecision::Allow
    );
    assert_eq!(
        restarted
            .authority
            .authorize(&AuthorityRequest {
                actor: actor_b,
                action: ActionId::new("test.delete").expect("delete"),
                resource,
                context: RequestContext::default(),
            })
            .expect("b delete deny after restart"),
        AuthorityDecision::Deny
    );
}

#[test]
fn genesis_schema_reopens_and_rejects_pre_sec0_metadata() {
    let harness = Harness::new();
    let scope = {
        let mut database = AuthorityDatabase::open(&harness.authority_db).expect("install genesis");
        database.create_scope().expect("create scope")
    };
    let reopened = AuthorityDatabase::open(&harness.authority_db).expect("reopen genesis");
    assert!(
        reopened
            .authorize(
                &AuthorityRequest {
                    actor: ActorRef::new("system:schema-proof").expect("actor"),
                    action: ActionId::new("test.read".to_owned()).expect("action"),
                    resource: ResourceRef::new(scope, "test", "resource").expect("resource"),
                    context: RequestContext::default(),
                },
                &FakeDecisionAdapter,
            )
            .is_ok()
    );

    let legacy_path = harness._tempdir.path().join("legacy-authority.sqlite");
    Connection::open(&legacy_path)
        .expect("open legacy fixture")
        .execute_batch(
            "CREATE TABLE authority_schema (singleton_key INTEGER PRIMARY KEY, schema_version INTEGER NOT NULL);
             INSERT INTO authority_schema VALUES (1, 1);
             CREATE TABLE scopes (scope_id TEXT PRIMARY KEY);",
        )
        .expect("write legacy fixture");
    assert!(matches!(
        AuthorityDatabase::open(&legacy_path),
        Err(crate::AuthorityError::Integrity { .. })
    ));
}

struct FakeDecisionAdapter;

impl AuthorityDecisionAdapter for FakeDecisionAdapter {
    fn authorize(
        &self,
        input: AuthorityEvaluationInput<'_>,
    ) -> Result<AuthorityDecision, crate::AuthorityError> {
        for grant in input.grants() {
            match grant {
                AuthorityGrant::ScopeControl { actor, scope_id }
                    if actor == &input.request().actor
                        && scope_id == &input.request().resource.scope_id =>
                {
                    return Ok(AuthorityDecision::Allow);
                }
                AuthorityGrant::Exact {
                    actor,
                    action,
                    resource,
                } if actor == &input.request().actor
                    && action == &input.request().action
                    && resource == &input.request().resource =>
                {
                    return Ok(AuthorityDecision::Allow);
                }
                _ => {}
            }
        }
        Ok(AuthorityDecision::Deny)
    }
}

struct CapturedContracts {
    composition: fabric_core::Instance,
    authority: AuthorityContract,
}

impl CapturedContracts {
    fn stop(mut self) {
        self.composition.stop();
    }
}

fn captured_contracts(
    authority_path: &Path,
    decision_adapter: Arc<dyn AuthorityDecisionAdapter>,
) -> CapturedContracts {
    #[derive(Default)]
    struct Slots {
        authority: Mutex<Option<AuthorityContract>>,
    }

    #[derive(Clone)]
    struct Capture {
        module_id: ModuleId,
        authority_requirement: ContractRequirement<AuthorityContract>,
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
            vec![self.authority_requirement.id().clone()]
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
            self.slots
                .authority
                .lock()
                .expect("authority")
                .replace(authority.as_ref().clone());
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
            decision_adapter,
        ))
        .build();
    let capture_block = BlockBuilder::new(BlockId::new("test.capture").expect("block"))
        .register_module(Capture {
            module_id: ModuleId::new("test.authority.consumer").expect("module"),
            authority_requirement: ContractRequirement::provisional(crate::authority_contract_id()),
            slots: Arc::clone(&slots),
        })
        .build();

    let composition =
        CompositionBuilder::new(CompositionId::new("test.authority.sec0").expect("composition"))
            .register_block(authority_block)
            .register_block(capture_block)
            .build()
            .expect("build composition");
    let mut composition = composition
        .materialize(fabric_core::InstanceId::new("test.authority.sec0").expect("instance id"))
        .expect("materialize composition");
    composition.start().expect("start composition");

    CapturedContracts {
        authority: slots
            .authority
            .lock()
            .expect("authority")
            .clone()
            .expect("captured authority"),
        composition,
    }
}
