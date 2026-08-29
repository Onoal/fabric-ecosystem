use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};

use fabric_component::{
    Component, ComponentError, ComponentId, ComponentRegistry, ComponentRuntime,
    ComponentRuntimeLifecycle, ComponentRuntimeModule, ComponentRuntimeService,
    ComponentRuntimeStatus, Surface, SurfaceId, SurfaceRegistry,
};
use fabric_component_namespace::{Namespace, NamespaceError, NamespaceModule, NamespaceName};
use fabric_component_publication::{PublicationContract, PublicationError, PublicationModule};
use fabric_core::{
    BlockBuilder, BlockId, CompositionBuilder, CompositionId, ContractRequirement, Health,
    Instance, InstanceId, ModuleBindings, ModuleContract, ModuleError, ModuleId, ModuleRuntime,
};

#[derive(Clone)]
struct StaticComponentRuntimeService {
    instance_id: InstanceId,
}

impl ComponentRuntimeService for StaticComponentRuntimeService {
    fn instance_id(&self) -> InstanceId {
        self.instance_id.clone()
    }

    fn current_instance_id(&self) -> Result<InstanceId, ComponentError> {
        Ok(self.instance_id())
    }

    fn current_status(&self) -> ComponentRuntimeStatus {
        ComponentRuntimeStatus::new(
            self.instance_id(),
            None,
            ComponentRuntimeLifecycle::Stopped,
            Health::Unavailable,
        )
    }

    fn current_lifecycle(&self) -> ComponentRuntimeLifecycle {
        ComponentRuntimeLifecycle::Ready
    }

    fn current_health(&self) -> Health {
        Health::Healthy
    }
}

#[derive(Clone)]
struct CapturedRails {
    instance_id: InstanceId,
    registry: Arc<ComponentRegistry>,
    surface_registry: Arc<SurfaceRegistry>,
    namespace: Arc<Namespace>,
    publications: Arc<PublicationContract>,
}

type Capture = Arc<Mutex<Option<CapturedRails>>>;

#[derive(Clone)]
struct RailsCaptureModule {
    module_id: ModuleId,
    runtime_requirement: ContractRequirement<ComponentRuntime>,
    registry_requirement: ContractRequirement<ComponentRegistry>,
    surface_requirement: ContractRequirement<SurfaceRegistry>,
    namespace_requirement: ContractRequirement<Namespace>,
    publication_requirement: ContractRequirement<PublicationContract>,
    capture: Capture,
}

impl RailsCaptureModule {
    fn new(capture: Capture) -> Self {
        Self {
            module_id: ModuleId::new("publication.namespace.capture").expect("module id"),
            runtime_requirement: ContractRequirement::provisional(
                fabric_component::component_runtime_contract_id(),
            ),
            registry_requirement: ContractRequirement::provisional(
                fabric_component::component_registry_contract_id(),
            ),
            surface_requirement: ContractRequirement::provisional(
                fabric_component::surface_contract_id(),
            ),
            namespace_requirement: ContractRequirement::provisional(
                fabric_component_namespace::namespace_contract_id(),
            ),
            publication_requirement: ContractRequirement::provisional(
                fabric_component_publication::publication_contract_id(),
            ),
            capture,
        }
    }
}

impl ModuleRuntime for RailsCaptureModule {
    fn id(&self) -> &ModuleId {
        &self.module_id
    }

    fn provided_contract_declarations(&self) -> Vec<fabric_core::ProvidedContractDeclaration> {
        Vec::new()
            .into_iter()
            .map(fabric_core::ProvidedContractDeclaration::provisional)
            .collect()
    }

    fn required_contract_declarations(&self) -> Vec<fabric_core::ContractRequirementDeclaration> {
        vec![
            self.runtime_requirement.id().clone(),
            self.registry_requirement.id().clone(),
            self.surface_requirement.id().clone(),
            self.namespace_requirement.id().clone(),
            self.publication_requirement.id().clone(),
        ]
        .into_iter()
        .map(fabric_core::ContractRequirementDeclaration::provisional)
        .collect()
    }

    fn export_contracts(&self) -> Result<Vec<ModuleContract>, ModuleError> {
        Ok(Vec::new())
    }

    fn bind(&mut self, bindings: &ModuleBindings) -> Result<(), ModuleError> {
        let runtime = bindings
            .resolve(&self.runtime_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let registry = bindings
            .resolve(&self.registry_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let surfaces = bindings
            .resolve(&self.surface_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let namespace = bindings
            .resolve(&self.namespace_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        let publications = bindings
            .resolve(&self.publication_requirement)
            .map_err(|error| ModuleError::new(error.to_string()))?;
        *self.capture.lock().expect("capture lock") = Some(CapturedRails {
            instance_id: runtime.instance_id(),
            registry,
            surface_registry: surfaces,
            namespace,
            publications,
        });
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

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    struct Parker {
        ready: Mutex<bool>,
        cvar: Condvar,
    }

    impl Wake for Parker {
        fn wake(self: Arc<Self>) {
            let mut ready = self.ready.lock().expect("ready lock");
            *ready = true;
            self.cvar.notify_one();
        }
    }

    let parker = Arc::new(Parker {
        ready: Mutex::new(true),
        cvar: Condvar::new(),
    });
    let waker = Waker::from(Arc::clone(&parker));
    let mut context = Context::from_waker(&waker);
    let mut future = Pin::from(Box::new(future));

    loop {
        {
            let mut ready = parker.ready.lock().expect("ready lock");
            *ready = false;
        }

        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => {
                let mut ready = parker.ready.lock().expect("ready lock");
                while !*ready {
                    ready = parker.cvar.wait(ready).expect("wait ready");
                }
            }
        }
    }
}

fn static_runtime(instance_id: &str) -> ComponentRuntime {
    ComponentRuntime::new(Arc::new(StaticComponentRuntimeService {
        instance_id: InstanceId::new(instance_id).expect("instance id"),
    }))
}

fn participate(rails: &CapturedRails, component_id: &str) -> Component {
    let component = Component::bind(
        ComponentId::new(component_id).expect("component id"),
        &static_runtime(rails.instance_id.as_str()),
    );
    let participation = rails
        .registry
        .register(component.clone(), Health::Healthy)
        .expect("register component")
        .participation()
        .clone();
    rails
        .registry
        .activate(&participation)
        .expect("activate component");
    component
}

fn started_rails(instance_id: &str) -> (Instance, CapturedRails) {
    let capture = Arc::new(Mutex::new(None));
    let block = BlockBuilder::new(BlockId::new("publication.namespace.block").expect("block id"))
        .register_module(ComponentRuntimeModule::new())
        .register_module(NamespaceModule::new())
        .register_module(PublicationModule::new())
        .register_module(RailsCaptureModule::new(Arc::clone(&capture)))
        .build();
    let composition = CompositionBuilder::new(
        CompositionId::new("publication.namespace.instance").expect("composition id"),
    )
    .register_block(block)
    .build()
    .expect("build composition");
    let mut instance = composition
        .materialize(InstanceId::new(instance_id).expect("instance id"))
        .expect("materialize instance");
    instance.start().expect("start instance");
    let rails = capture
        .lock()
        .expect("capture lock")
        .clone()
        .expect("captured rails");
    (instance, rails)
}

#[test]
fn cross_instance_surface_and_publication_ownership_is_rejected() {
    let (_instance, rails) = started_rails("instance.main");
    let local_owner = participate(&rails, "photos");
    let foreign_owner = Component::bind(
        ComponentId::new("photos").expect("component id"),
        &static_runtime("instance.other"),
    );

    let error = rails
        .surface_registry
        .register(
            foreign_owner.clone(),
            SurfaceId::new("photos.main").expect("surface id"),
        )
        .expect_err("foreign owner surface registration must fail");
    assert!(matches!(
        error,
        ComponentError::SurfaceOwnerInstanceMismatch { .. }
    ));

    let claim = rails
        .namespace
        .claim(
            local_owner.clone(),
            NamespaceName::new("photos").expect("namespace name"),
        )
        .expect("allocate namespace");
    rails
        .surface_registry
        .register(
            local_owner,
            SurfaceId::new("photos.main").expect("surface id"),
        )
        .expect("register local surface");
    let foreign_surface = Surface::new(
        foreign_owner,
        SurfaceId::new("photos.main").expect("surface id"),
    );
    let error = rails
        .publications
        .publish(claim, foreign_surface)
        .expect_err("foreign surface publication must fail");
    assert!(matches!(error, PublicationError::UnknownSurface(_)));
}

#[test]
fn namespace_allocation_is_deterministic_and_conflicts_fail() {
    let (_instance, rails) = started_rails("instance.main");
    let first_owner = participate(&rails, "photos");
    let second_owner = participate(&rails, "notes");
    let name = NamespaceName::new("photos").expect("namespace name");

    let claim = rails
        .namespace
        .claim(first_owner.clone(), name.clone())
        .expect("namespace allocation");
    assert_eq!(claim.owner(), &first_owner);
    assert_eq!(claim.name(), &name);
    assert_eq!(rails.namespace.allocation(&name).expect("lookup"), claim);

    let error = rails
        .namespace
        .claim(second_owner, name)
        .expect_err("duplicate namespace allocation must fail");
    assert!(matches!(
        error,
        NamespaceError::NamespaceNameAlreadyAllocated(_)
    ));
}

#[test]
fn surface_existence_does_not_imply_publication() {
    let (_instance, rails) = started_rails("instance.main");
    let owner = participate(&rails, "photos");
    let surface = rails
        .surface_registry
        .register(
            owner.clone(),
            SurfaceId::new("photos.main").expect("surface id"),
        )
        .expect("register surface");
    let name = NamespaceName::new("photos").expect("namespace name");
    let claim = rails
        .namespace
        .claim(owner, name.clone())
        .expect("claim namespace");

    let error = rails
        .publications
        .publication(&name)
        .expect_err("surface existence must not imply publication");
    assert!(matches!(error, PublicationError::UnknownPublication(_)));

    let publication = rails
        .publications
        .publish(claim.clone(), surface.clone())
        .expect("publish surface");
    assert_eq!(publication.claim(), &claim);
    assert_eq!(publication.surface(), &surface);
}

#[test]
fn publication_lookup_is_stable_and_component_agnostic() {
    let (_instance, rails) = started_rails("instance.main");
    let owner = participate(&rails, "photos");
    let surface = rails
        .surface_registry
        .register(
            owner.clone(),
            SurfaceId::new("photos.main").expect("surface id"),
        )
        .expect("register surface");
    let claim = rails
        .namespace
        .claim(
            owner.clone(),
            NamespaceName::new("photos").expect("namespace name"),
        )
        .expect("claim namespace");
    rails
        .publications
        .publish(claim, surface)
        .expect("publish surface");

    let publication = rails
        .publications
        .publication(&NamespaceName::new("photos").expect("namespace name"))
        .expect("lookup publication");

    assert_eq!(
        publication.claim().owner().component_id(),
        owner.component_id()
    );
    assert_eq!(
        publication.claim().owner().instance_id(),
        &rails.instance_id
    );
    assert_eq!(publication.surface().surface_id().as_str(), "photos.main");
    assert_eq!(
        publication.surface().owner().component_id().as_str(),
        "photos"
    );
}

#[test]
fn unknown_publication_fails_semantically() {
    let (_instance, rails) = started_rails("instance.main");
    let error = rails
        .publications
        .publication(&NamespaceName::new("missing").expect("namespace name"))
        .expect_err("unknown publication must fail");
    assert!(matches!(error, PublicationError::UnknownPublication(_)));
}

#[test]
fn publication_requires_registered_surface_and_allocated_name() {
    let (_instance, rails) = started_rails("instance.main");
    let owner = participate(&rails, "photos");

    let claim = rails
        .namespace
        .claim(
            owner.clone(),
            NamespaceName::new("photos").expect("namespace name"),
        )
        .expect("claim namespace");
    let missing_surface = Surface::new(
        owner.clone(),
        SurfaceId::new("photos.main").expect("surface id"),
    );
    let error = rails
        .publications
        .publish(claim, missing_surface)
        .expect_err("unregistered surface must fail publication");
    assert!(matches!(
        error,
        PublicationError::Component(ComponentError::UnknownSurface(_))
    ));

    let surface = rails
        .surface_registry
        .register(
            owner.clone(),
            SurfaceId::new("photos.main").expect("surface id"),
        )
        .expect("register surface");
    let missing_claim = fabric_component_namespace::NamespaceClaim::new(
        owner,
        NamespaceName::new("gallery").expect("namespace name"),
    );
    let error = rails
        .publications
        .publish(missing_claim, surface)
        .expect_err("unallocated name must fail publication");
    assert!(matches!(
        error,
        PublicationError::Namespace(NamespaceError::UnknownNamespaceName(_))
    ));
}

#[test]
fn publication_lookup_roundtrip_is_plain_in_process_semantic_truth() {
    let (_instance, rails) = started_rails("instance.main");
    let owner = participate(&rails, "notes");
    let surface = rails
        .surface_registry
        .register(
            owner.clone(),
            SurfaceId::new("notes.main").expect("surface id"),
        )
        .expect("register surface");
    let claim = rails
        .namespace
        .claim(owner, NamespaceName::new("notes").expect("namespace name"))
        .expect("claim namespace");

    let publication = rails
        .publications
        .publish(claim.clone(), surface.clone())
        .expect("publish notes");
    let resolved = rails
        .publications
        .publication(claim.name())
        .expect("resolve notes publication");

    assert_eq!(publication, resolved);
    assert_eq!(resolved.claim(), &claim);
    assert_eq!(resolved.surface(), &surface);
}

#[test]
fn publication_lookup_is_runtime_state_not_transport_state() {
    let (_instance, rails) = started_rails("instance.main");
    let owner = participate(&rails, "gallery");
    let surface = rails
        .surface_registry
        .register(
            owner.clone(),
            SurfaceId::new("gallery.main").expect("surface id"),
        )
        .expect("register surface");
    let claim = rails
        .namespace
        .claim(
            owner,
            NamespaceName::new("gallery").expect("namespace name"),
        )
        .expect("claim namespace");

    let publication = rails
        .publications
        .publish(claim, surface)
        .expect("publish gallery");
    let resolved = block_on(async {
        Ok::<_, PublicationError>(publication.claim().name().as_str().to_owned())
    })
    .expect("in-process proof");

    assert_eq!(resolved, "gallery");
}
