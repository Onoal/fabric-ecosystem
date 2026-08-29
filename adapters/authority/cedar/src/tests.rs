use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
};
use fabric_resource_authority::{
    ActionId, ActorRef, AuthorityContract, AuthorityDecision, AuthorityDecisionAdapter,
    AuthorityRequest, NativeAuthority, NativeAuthorityConfig, RequestContext, ResourceRef,
    authority_contract_id,
};
use tempfile::TempDir;

use crate::CedarAuthorityDecisionAdapter;
use crate::adapter::context_attribute_reaches_cedar;

#[test]
fn default_deny_and_scope_control_are_scope_bounded() {
    let harness = Harness::new();
    let contracts = harness.captured_contracts(Arc::new(CedarAuthorityDecisionAdapter::new()));
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
fn exact_action_grants_remain_exact() {
    let harness = Harness::new();
    let contracts = harness.captured_contracts(Arc::new(CedarAuthorityDecisionAdapter::new()));
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
fn request_context_reaches_cedar_evaluation() {
    assert!(context_attribute_reaches_cedar("allow").expect("context allow"));
    assert!(!context_attribute_reaches_cedar("deny").expect("context deny"));
}

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

    fn captured_contracts(
        &self,
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

            fn provided_contract_declarations(
                &self,
            ) -> Vec<fabric_core::ProvidedContractDeclaration> {
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
                    database_path: self.authority_db.clone(),
                },
                decision_adapter,
            ))
            .build();
        let capture_block = BlockBuilder::new(BlockId::new("test.capture").expect("block"))
            .register_module(Capture {
                module_id: ModuleId::new("test.authority.consumer").expect("module"),
                authority_requirement: ContractRequirement::provisional(authority_contract_id()),
                slots: Arc::clone(&slots),
            })
            .build();

        let composition = CompositionBuilder::new(
            CompositionId::new("test.authority.sec0").expect("composition"),
        )
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
            _composition: composition,
        }
    }
}

struct CapturedContracts {
    _composition: fabric_core::Instance,
    authority: AuthorityContract,
}
