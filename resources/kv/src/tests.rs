use std::sync::Arc;
use std::{fs, path::Path};

use fabric_binding::BindingName;
use fabric_resource::{ResourceBoundaryId, ResourceContext, ResourceError, ResourceName};
use fabric_resource_registry::ResourceDescriptor;

use crate::{KvAccess, KvContract, KvRef, KvService, PreparedKv, PreparedKvState, kv_resource_id};

struct AssertingKvService;

impl KvService for AssertingKvService {
    fn prepare(
        &self,
        resource_id: &fabric_resource::ResourceInstanceId,
    ) -> Result<PreparedKvState, ResourceError> {
        assert_eq!(resource_id.resource(), kv_resource_id());
        Ok(PreparedKvState { created: false })
    }

    fn cleanup(&self, _prepared: &PreparedKv) {}

    fn resolve(
        &self,
        resource_id: &fabric_resource::ResourceInstanceId,
    ) -> Result<KvAccess, ResourceError> {
        assert_eq!(resource_id.resource(), kv_resource_id());
        Ok(KvAccess::testing_stub(resource_id.clone()))
    }
}

#[test]
fn kv_ref_identity_encoding_remains_stable() {
    let boundary = ResourceBoundaryId::new("apps.notes").expect("boundary");
    let context = ResourceContext::root(boundary);
    let name = ResourceName::new("cache").expect("name");

    let reference = KvRef::parse(format!(
        "fabric-resource-kv-ref-v1:{}",
        fabric_resource::ResourceInstanceId::canonical(&kv_resource_id(), &context, &name).as_str()
    ))
    .expect("kv ref");

    assert_eq!(
        reference.encode(),
        format!(
            "fabric-resource-kv-ref-v1:{}",
            reference.resource_id().as_str()
        )
    );
    assert!(
        reference
            .resource_id()
            .as_str()
            .starts_with("fabric-resource-kv-")
    );
}

#[test]
fn kv_ref_validation_uses_the_same_canonical_resource_identity_as_provisioning() {
    let boundary = ResourceBoundaryId::new("apps.notes").expect("boundary");
    let context = ResourceContext::root(boundary);
    let name = ResourceName::new("cache").expect("name");
    let contract = KvContract::new(Arc::new(AssertingKvService));

    let prepared = contract
        .prepare(context.clone(), name.clone())
        .expect("prepare kv");
    let reference = prepared.kv_ref();
    let parsed = KvRef::parse(reference.encode()).expect("parse kv ref");
    let descriptor = ResourceDescriptor::new(kv_resource_id());

    assert_eq!(reference.resource_id().resource(), kv_resource_id());
    assert_eq!(parsed.resource_id(), reference.resource_id());
    assert_eq!(parsed.resource_id().resource(), kv_resource_id());
    assert_eq!(descriptor.resource_id(), &kv_resource_id());
}

#[test]
fn kv_binding_is_pure_relationship_truth_and_access_stays_separate() {
    let boundary = ResourceBoundaryId::new("apps.notes").expect("boundary");
    let owner = ResourceContext::root(boundary.clone());
    let consumer = ResourceContext::root(boundary)
        .child(fabric_resource::ResourceScope::new("consumer").expect("consumer scope"));
    let name = ResourceName::new("cache").expect("name");
    let contract = KvContract::new(Arc::new(AssertingKvService));

    let prepared = contract
        .prepare(owner.clone(), name.clone())
        .expect("prepare kv");
    let binding = contract
        .bind(
            &consumer,
            BindingName::new("shared-cache").expect("binding name"),
            &prepared.kv_ref(),
        )
        .expect("bind kv ref");

    assert_eq!(binding.kv_ref(), prepared.kv_ref());
    assert_eq!(binding.name().as_str(), "shared-cache");
    let access = contract.access(&binding.kv_ref()).expect("resolve access");
    assert_eq!(access.resource_id(), binding.resource_id());
}

#[test]
fn pre_e3_kv_resource_import_binding_identity_is_preserved_exactly() {
    let context = ResourceContext::root(ResourceBoundaryId::new("apps.notes").expect("boundary"))
        .child(fabric_resource::ResourceScope::new("release").expect("scope"));
    let name = ResourceName::new("primary").expect("name");
    let contract = KvContract::new(Arc::new(AssertingKvService));
    let reference = contract.reference(&context, &name);
    let binding = contract
        .bind(
            &context,
            BindingName::new("shared").expect("binding name"),
            &reference,
        )
        .expect("bind kv ref");

    assert_eq!(
        reference.resource_id().as_str(),
        "fabric-resource-kv-d3f19e9224103f8cc238ebcba28bb8e7f9d90ee53ce7e53776931b8e6ce1f466"
    );
    assert_eq!(
        binding.binding_id().as_str(),
        "fabric-binding-543730d9332734f9cef722dadd3850a1604b1fd3cffa9ee0c15adf897446f27e"
    );
}

#[test]
fn semantic_kv_module_owns_shell_registration_and_resource_crate_stays_adapter_neutral() {
    let native_module =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/module.rs"))
            .expect("read native kv module");
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read kv manifest");
    let production_dependencies = manifest
        .split("[dev-dependencies]")
        .next()
        .expect("production dependency section");

    assert!(native_module.contains("fabric.resource.kv.native"));
    assert!(native_module.contains("register(self, ResourceDescriptor::new(kv_resource_id()))"));
    assert!(
        !production_dependencies.contains("fjall"),
        "kv resource crate must not depend on the concrete fjall adapter"
    );
}
