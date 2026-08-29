use std::fs;
use std::path::PathBuf;

use fabric_binding::BindingName;
use fabric_resource::{ResourceBoundaryId, ResourceContext, ResourceInstanceId, ResourceName};
use fabric_resource_kv::{KvRef, kv_resource_id};
use fabric_resource_worker::{
    BindingProjection, BindingTarget, WorkloadBinding, WorkloadBindingEnv,
    WorkloadBindingProjection, WorkloadId,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../")
}

#[test]
fn witness_workloads_require_explicit_resource_bindings_without_ambient_registry_access() {
    let source = fs::read_to_string(repo_root().join("resources/worker/src/tests.rs"))
        .expect("read workload binding witness");

    for required in [
        "BindingTarget::database",
        "BindingTarget::secret",
        "WorkloadBinding::new",
    ] {
        assert!(
            source.contains(required),
            "workload binding path lost explicit resource binding step {required}"
        );
    }

    for forbidden in [
        "ResourceRegistry",
        "ComponentScope",
        "ComponentRegistry",
        "fabric_adapter_",
    ] {
        assert!(
            !source.contains(forbidden),
            "workload binding path leaked ambient authority via {forbidden}"
        );
    }
}

#[test]
fn workload_projection_consumes_bindings_and_projections_without_component_authority() {
    let source =
        fs::read_to_string(repo_root().join("resources/worker/src/projection/native/module.rs"))
            .expect("read workload projection module");

    for required in [
        "prepare_workload_projections",
        "validate_workload_bindings",
        "BindingTarget::",
    ] {
        assert!(
            source.contains(required),
            "workload projection lost explicit binding/projection flow {required}"
        );
    }

    for forbidden in [
        "ComponentScope",
        "ComponentRegistry",
        "ResourceRegistry",
        "fabric_adapter_",
    ] {
        assert!(
            !source.contains(forbidden),
            "workload projection gained non-canonical authority via {forbidden}"
        );
    }
}

#[test]
fn workload_binding_projection_identity_stays_resource_facing_and_projection_distinct() {
    let consumer = WorkloadId::new("apps.worker").expect("workload id");
    let resource_context =
        ResourceContext::root(ResourceBoundaryId::new("fc5.workload.binding").expect("boundary"));
    let resource_name = ResourceName::new("cache").expect("resource name");
    let resource_id =
        ResourceInstanceId::canonical(&kv_resource_id(), &resource_context, &resource_name);
    let reference = KvRef::parse(format!(
        "fabric-resource-kv-ref-v1:{}",
        resource_id.as_str()
    ))
    .expect("kv reference");
    let binding = WorkloadBinding::new(
        consumer.clone(),
        BindingName::new("CACHE").expect("binding name"),
        BindingTarget::kv(reference.clone()),
    );
    let environment = WorkloadBindingEnv::new("CACHE_URL").expect("binding env");
    let projection =
        WorkloadBindingProjection::environment(binding.binding_id(), environment.clone());

    assert_eq!(reference.resource_id(), &resource_id);
    assert_eq!(reference.resource_id().resource(), kv_resource_id());
    assert_eq!(binding.consumer(), &consumer);
    assert_eq!(binding.name().as_str(), "CACHE");
    assert!(binding.binding_id().as_str().starts_with("fabric-binding-"));
    assert_eq!(projection.binding_id(), binding.binding_id());
    assert_eq!(
        projection.projection(),
        &BindingProjection::Environment(environment)
    );
}
