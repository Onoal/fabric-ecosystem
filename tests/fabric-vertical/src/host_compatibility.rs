use fabric_host::{
    HostArchitecture, HostCompatibilityError, HostDescriptor, HostFacilityId, HostOperatingSystem,
    HostRequirement,
};
use fabric_resource::{
    AdapterResourceSchemaSupport, ResourceCompatibilityError, ResourceId, ResourceRequirement,
    ResourceSchemaDescriptor, ResourceSchemaRequirement, ResourceSchemaVersion,
};

struct SyntheticAdapterCandidate {
    host_requirement: HostRequirement,
    schema_support: AdapterResourceSchemaSupport,
}

fn evaluate_candidate(
    resource_requirement: &ResourceRequirement,
    resource_schema: &ResourceSchemaDescriptor,
    adapter: &SyntheticAdapterCandidate,
    host: &HostDescriptor,
) -> Result<(), String> {
    resource_requirement
        .evaluate_compatibility(resource_schema, &adapter.schema_support)
        .map_err(|error| format!("resource compatibility: {error}"))?;
    adapter
        .host_requirement
        .evaluate(host)
        .map_err(|error| format!("host compatibility: {error}"))?;
    Ok(())
}

#[test]
fn same_resource_schema_allows_different_adapters_only_by_host_requirement() {
    let resource_id = ResourceId::new("fabric.test.executor").expect("resource");
    let requirement = ResourceRequirement::versioned(
        resource_id.clone(),
        "executor",
        ResourceSchemaRequirement::parse("^1").expect("requirement"),
    )
    .expect("resource requirement");
    let schema = ResourceSchemaDescriptor::versioned(
        resource_id.clone(),
        ResourceSchemaVersion::parse("1.2.0").expect("schema"),
    );
    let schema_support = AdapterResourceSchemaSupport::versioned(
        resource_id.clone(),
        ResourceSchemaRequirement::parse("^1").expect("support"),
    );
    let facility = HostFacilityId::new("test.fast-runtime").expect("facility");
    let linux_adapter = SyntheticAdapterCandidate {
        schema_support: schema_support.clone(),
        host_requirement: HostRequirement::new()
            .allow_operating_system(HostOperatingSystem::new("linux").expect("os"))
            .require_facility(facility.clone()),
    };
    let macos_adapter = SyntheticAdapterCandidate {
        schema_support,
        host_requirement: HostRequirement::new()
            .allow_operating_system(HostOperatingSystem::new("macos").expect("os"))
            .require_facility(facility.clone()),
    };
    let linux_host = HostDescriptor::new(
        HostOperatingSystem::new("linux").expect("os"),
        HostArchitecture::new("x86_64").expect("arch"),
    )
    .with_facility(facility.clone());
    let macos_host = HostDescriptor::new(
        HostOperatingSystem::new("macos").expect("os"),
        HostArchitecture::new("aarch64").expect("arch"),
    )
    .with_facility(facility);

    evaluate_candidate(&requirement, &schema, &linux_adapter, &linux_host)
        .expect("linux adapter on linux host");
    match evaluate_candidate(&requirement, &schema, &macos_adapter, &linux_host) {
        Err(message) => assert!(message.contains("host compatibility")),
        other => panic!("unexpected linux host / macos adapter result: {other:?}"),
    }

    evaluate_candidate(&requirement, &schema, &macos_adapter, &macos_host)
        .expect("macos adapter on macos host");
    match evaluate_candidate(&requirement, &schema, &linux_adapter, &macos_host) {
        Err(message) => assert!(message.contains("host compatibility")),
        other => panic!("unexpected macos host / linux adapter result: {other:?}"),
    }
}

#[test]
fn host_changes_do_not_change_semantic_resource_identity() {
    let resource_id = ResourceId::new("fabric.test.executor").expect("resource");
    let schema = ResourceSchemaDescriptor::versioned(
        resource_id.clone(),
        ResourceSchemaVersion::parse("1.2.0").expect("schema"),
    );
    let requirement = ResourceRequirement::versioned(
        resource_id.clone(),
        "executor",
        ResourceSchemaRequirement::parse("^1").expect("requirement"),
    )
    .expect("resource requirement");
    let linux_host = HostDescriptor::new(
        HostOperatingSystem::new("linux").expect("os"),
        HostArchitecture::new("x86_64").expect("arch"),
    );
    let macos_host = HostDescriptor::new(
        HostOperatingSystem::new("macos").expect("os"),
        HostArchitecture::new("aarch64").expect("arch"),
    );

    assert_eq!(schema.resource(), &resource_id);
    assert_eq!(requirement.resource(), &resource_id);
    assert_ne!(linux_host.operating_system(), macos_host.operating_system());
}

#[test]
fn host_facilities_remain_open_without_resource_api_branching() {
    let facility =
        HostFacilityId::new("third-party.custom-accelerator").expect("third-party facility");
    let host = HostDescriptor::new(
        HostOperatingSystem::new("linux").expect("os"),
        HostArchitecture::new("aarch64").expect("arch"),
    )
    .with_facility(facility.clone());
    let requirement = HostRequirement::new().require_facility(facility);

    requirement.evaluate(&host).expect("third-party facility");
}

#[test]
fn resource_and_host_compatibility_remain_separate_error_domains() {
    let resource_id = ResourceId::new("fabric.test.executor").expect("resource");
    let requirement = ResourceRequirement::versioned(
        resource_id.clone(),
        "executor",
        ResourceSchemaRequirement::parse("^1").expect("requirement"),
    )
    .expect("resource requirement");
    let schema = ResourceSchemaDescriptor::versioned(
        resource_id.clone(),
        ResourceSchemaVersion::parse("2.0.0").expect("schema"),
    );
    let support = AdapterResourceSchemaSupport::versioned(
        resource_id.clone(),
        ResourceSchemaRequirement::parse("^2").expect("support"),
    );
    let host_requirement = HostRequirement::new()
        .allow_operating_system(HostOperatingSystem::new("linux").expect("os"));
    let host = HostDescriptor::new(
        HostOperatingSystem::new("macos").expect("os"),
        HostArchitecture::new("aarch64").expect("arch"),
    );

    match requirement.evaluate_compatibility(&schema, &support) {
        Err(ResourceCompatibilityError::ResourceRequirementIncompatible { .. }) => {}
        other => panic!("unexpected resource compatibility result: {other:?}"),
    }
    match host_requirement.evaluate(&host) {
        Err(HostCompatibilityError::UnsupportedOperatingSystem { .. }) => {}
        other => panic!("unexpected host compatibility result: {other:?}"),
    }
}
