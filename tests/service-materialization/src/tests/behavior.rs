use fabric_component::{ComponentError, Surface, SurfaceId};
use fabric_component_gateway::GatewayError;
use fabric_component_namespace::NamespaceName;
use fabric_component_publication::PublicationError;
use fabric_resource_connectivity::ConnectivityScope;
use fabric_resource_ingress::IngressError;
use fabric_resource_service::ServiceProtocol;

use crate::{GatewayExposureAvailability, LocalNetworkName, ServiceMaterializationError};

use super::harness::{
    MaterializationHarness, drain_and_withdraw_target, publish_surface,
    register_ready_loopback_target, register_ready_target, register_surface,
    replace_with_new_loopback_target, spawn_loopback_server,
};

#[test]
fn service_backed_surface_preserves_distinct_instance_and_service_identities() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let service = harness.ensure_http_service("service.notes.web", "web");

    let backing = harness
        .materialization
        .bind_service(surface.clone(), service.id.clone(), ServiceProtocol::Http)
        .expect("bind service backing");

    assert_eq!(backing.surface(), &surface);
    assert_eq!(backing.service_id(), &service.id);
    assert_ne!(
        backing.surface().surface_id().as_str(),
        backing.service_id().as_str()
    );
}

#[test]
fn binding_requires_registered_surface() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let service = harness.ensure_http_service("service.notes.web", "web");
    let surface = Surface::new(
        component,
        SurfaceId::new("notes.unregistered").expect("surface id"),
    );

    let error = harness
        .materialization
        .bind_service(surface, service.id.clone(), ServiceProtocol::Http)
        .expect_err("binding unregistered surface must fail");

    assert_eq!(
        error,
        ServiceMaterializationError::Component(ComponentError::UnknownSurface(
            SurfaceId::new("notes.unregistered").expect("surface id")
        ))
    );
}

#[test]
fn cross_instance_surface_binding_is_rejected() {
    let local = MaterializationHarness::new("instance.local");
    let foreign = MaterializationHarness::new("instance.foreign");
    let foreign_component = foreign.component("notes");
    let foreign_surface = register_surface(&foreign, &foreign_component, "notes.main");
    let local_service = local.ensure_http_service("service.notes.web", "web");

    let error = local
        .materialization
        .bind_service(
            foreign_surface.clone(),
            local_service.id.clone(),
            ServiceProtocol::Http,
        )
        .expect_err("foreign surface binding must fail");

    assert_eq!(
        error,
        ServiceMaterializationError::Component(ComponentError::SurfaceOwnerInstanceMismatch {
            surface_id: foreign_surface.surface_id().clone(),
            owner_instance_id: foreign_surface.owner().instance_id().clone(),
            instance_id: local.component_runtime.instance_id(),
        })
    );
}

#[test]
fn service_backing_does_not_publish_surface() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let service = harness.ensure_http_service("service.notes.web", "web");

    harness
        .materialization
        .bind_service(surface, service.id.clone(), ServiceProtocol::Http)
        .expect("bind service backing");

    let error = harness
        .materialization
        .materialized_publication(&NamespaceName::new("notes").expect("name"))
        .expect_err("binding alone must not materialize publication");

    assert_eq!(
        error,
        ServiceMaterializationError::UnknownMaterializedPublication(
            NamespaceName::new("notes").expect("name")
        )
    );

    let exposure = harness
        .exposure
        .entry(&NamespaceName::new("notes").expect("name"))
        .expect_err("unpublished surface must not resolve through gateway exposure");
    assert_eq!(
        exposure,
        ServiceMaterializationError::Gateway(GatewayError::Publication(Box::new(
            PublicationError::UnknownPublication(NamespaceName::new("notes").expect("name"),)
        ),))
    );
}

#[test]
fn publication_without_service_backing_does_not_materialize_ingress() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let publication = publish_surface(&harness, &component, "notes", &surface);

    let error = harness
        .materialization
        .materialize_publication(publication.clone())
        .expect_err("publication without backing must fail");

    assert_eq!(
        error,
        ServiceMaterializationError::UnknownServiceBacking(
            SurfaceId::new("notes.main").expect("surface id")
        )
    );

    let gateway_entry = harness
        .gateway
        .entry(publication.claim().name())
        .expect("gateway publication entry");
    let exposure = harness
        .exposure
        .entry(publication.claim().name())
        .expect("gateway exposure");

    assert_eq!(exposure.entry(), &gateway_entry);
    assert_eq!(
        exposure.availability(),
        GatewayExposureAvailability::PublishedOnly
    );
}

#[test]
fn service_backed_but_unmaterialized_publication_is_distinct_from_available_entry() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let publication = publish_surface(&harness, &component, "notes", &surface);
    let service = harness.ensure_http_service("service.notes.web", "web");

    harness
        .materialization
        .bind_service(surface, service.id.clone(), ServiceProtocol::Http)
        .expect("bind service backing");

    let exposure = harness
        .exposure
        .entry(publication.claim().name())
        .expect("gateway exposure");

    assert_eq!(
        exposure.availability(),
        GatewayExposureAvailability::ServiceBackedUnmaterialized
    );
}

#[test]
fn explicit_materialization_creates_ingress_route_to_service_id() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let publication = publish_surface(&harness, &component, "notes", &surface);
    let service = harness.ensure_http_service("service.notes.web", "web");

    let backing = harness
        .materialization
        .bind_service(surface.clone(), service.id.clone(), ServiceProtocol::Http)
        .expect("bind service backing");
    let materialized = harness
        .materialization
        .materialize_publication(publication.clone())
        .expect("materialize publication");

    assert_eq!(materialized.publication(), &publication);
    assert_eq!(materialized.backing(), &backing);
    assert_eq!(materialized.route().target.service_id, service.id);
    assert_eq!(
        materialized.route().target.protocol,
        service.requirement.protocol
    );

    let resolved = harness
        .ingress
        .resolve_route(&service.id)
        .expect("resolve route");
    assert_eq!(resolved.id, materialized.route().id);

    let exposure = harness
        .exposure
        .entry(publication.claim().name())
        .expect("gateway exposure");
    assert_eq!(
        exposure.availability(),
        GatewayExposureAvailability::MaterializedUnavailable
    );
}

#[test]
fn materialized_route_survives_service_endpoint_replacement() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let publication = publish_surface(&harness, &component, "notes", &surface);
    let service = harness.ensure_http_service("service.notes.web", "web");

    let first_server = spawn_loopback_server(200, "server-a");
    let first_target = register_ready_loopback_target(&harness, &service, "notes_a", &first_server);
    let materialized = harness
        .materialization
        .bind_service(surface.clone(), service.id.clone(), ServiceProtocol::Http)
        .and_then(|_| {
            harness
                .materialization
                .materialize_publication(publication.clone())
        })
        .expect("materialize service-backed publication");
    let first_entry = harness
        .exposure
        .entry(publication.claim().name())
        .expect("first gateway exposure");
    let access = harness
        .local_http_access
        .materialize_http_access(&materialized.route().id)
        .expect("materialize ingress access");

    let first_response = ureq::get(&access.url).call().expect("first request");
    assert_eq!(
        first_response
            .into_body()
            .read_to_string()
            .expect("first body"),
        "server-a"
    );

    let second_server = spawn_loopback_server(200, "server-b");
    replace_with_new_loopback_target(&harness, &service, &first_target, "notes_b", &second_server);
    let second_entry = harness
        .exposure
        .entry(publication.claim().name())
        .expect("second gateway exposure");
    let second_response = ureq::get(&access.url).call().expect("second request");
    assert_eq!(
        second_response
            .into_body()
            .read_to_string()
            .expect("second body"),
        "server-b"
    );

    let resolved_materialization = harness
        .materialization
        .materialized_publication(publication.claim().name())
        .expect("materialized publication lookup");
    assert_eq!(resolved_materialization.route().id, materialized.route().id);
    assert_eq!(
        resolved_materialization.backing().service_id(),
        materialized.backing().service_id()
    );
    assert_eq!(first_entry.entry(), second_entry.entry());
    assert_eq!(
        second_entry.availability(),
        GatewayExposureAvailability::Available
    );
}

#[test]
fn unavailable_service_returns_current_ingress_unavailable_semantics() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let publication = publish_surface(&harness, &component, "notes", &surface);
    let service = harness.ensure_http_service("service.notes.web", "web");
    let target_id = register_ready_target(&harness, &service, "notes_a", 200, "server-a");
    let materialized = harness
        .materialization
        .bind_service(surface, service.id.clone(), ServiceProtocol::Http)
        .and_then(|_| {
            harness
                .materialization
                .materialize_publication(publication.clone())
        })
        .expect("materialize publication");
    let access = harness
        .local_http_access
        .materialize_http_access(&materialized.route().id)
        .expect("local access");

    drain_and_withdraw_target(&harness, &service, &target_id);

    let exposure = harness
        .exposure
        .entry(publication.claim().name())
        .expect("gateway exposure after withdrawal");
    assert_eq!(
        exposure.availability(),
        GatewayExposureAvailability::MaterializedUnavailable
    );

    match ureq::get(&access.url).call() {
        Err(ureq::Error::StatusCode(503)) => {}
        other => panic!("expected unavailable service status code 503, got {other:?}"),
    }
}

#[test]
fn conflicting_service_backings_fail_deterministically() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let first = harness.ensure_http_service("service.notes.web", "web");
    let second = harness.ensure_http_service("service.notes.api", "api");

    harness
        .materialization
        .bind_service(surface.clone(), first.id.clone(), ServiceProtocol::Http)
        .expect("bind first backing");
    let error = harness
        .materialization
        .bind_service(surface.clone(), second.id.clone(), ServiceProtocol::Http)
        .expect_err("conflicting backing must fail");

    assert_eq!(
        error,
        ServiceMaterializationError::ConflictingServiceBacking {
            surface_id: surface.surface_id().clone(),
            existing_service_id: first.id,
            requested_service_id: second.id,
        }
    );
}

#[test]
fn route_resolution_still_fails_for_unknown_service() {
    let harness = MaterializationHarness::new("instance.local");
    let error = harness
        .ingress
        .resolve_route(
            &fabric_resource_service::ServiceId::parse("svc_unknown_route").expect("service id"),
        )
        .expect_err("unknown service route must fail");

    assert_eq!(error, IngressError::NotFound);
}

#[test]
fn service_backing_lookup_is_stable_by_surface_id() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let service = harness.ensure_http_service("service.notes.web", "web");

    let backing = harness
        .materialization
        .bind_service(surface.clone(), service.id.clone(), ServiceProtocol::Http)
        .expect("bind service backing");
    let resolved = harness
        .materialization
        .service_backing(surface.surface_id())
        .expect("lookup service backing");

    assert_eq!(resolved, backing);
}

#[test]
fn unknown_namespace_name_fails_through_gateway_exposure_with_component_semantics() {
    let harness = MaterializationHarness::new("instance.local");
    let error = harness
        .exposure
        .entry(&NamespaceName::new("missing").expect("namespace name"))
        .expect_err("unknown gateway exposure name must fail");

    assert_eq!(
        error,
        ServiceMaterializationError::Gateway(GatewayError::Publication(Box::new(
            PublicationError::UnknownPublication(
                NamespaceName::new("missing").expect("namespace name"),
            )
        ),))
    );
}

#[test]
fn gateway_lists_published_entries_deterministically() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");

    let alpha_surface = register_surface(&harness, &component, "alpha.main");
    let zeta_surface = register_surface(&harness, &component, "zeta.main");
    let _zeta = publish_surface(&harness, &component, "zeta", &zeta_surface);
    let _alpha = publish_surface(&harness, &component, "alpha", &alpha_surface);

    let names: Vec<String> = harness
        .exposure
        .entries()
        .expect("list gateway entries")
        .into_iter()
        .map(|entry| entry.entry().name().as_str().to_owned())
        .collect();

    assert_eq!(names, vec!["alpha".to_owned(), "zeta".to_owned()]);
}

#[test]
fn publication_alone_does_not_create_local_placement_or_reachability() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let publication = publish_surface(&harness, &component, "notes", &surface);

    let exposure = harness
        .local_exposure
        .entry(publication.claim().name())
        .expect("local exposure for published-only surface");

    assert_eq!(
        exposure.exposure().availability(),
        GatewayExposureAvailability::PublishedOnly
    );
    assert_eq!(exposure.reachability(), None);
    assert_eq!(exposure.local_access(), None);
}

#[test]
fn available_materialization_remains_unreachable_until_local_placement_is_activated() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let publication = publish_surface(&harness, &component, "notes", &surface);
    let service = harness.ensure_http_service("service.notes.web", "web");
    let _target_id = register_ready_target(&harness, &service, "notes_a", 200, "server-a");
    let materialized = harness
        .materialization
        .bind_service(surface, service.id.clone(), ServiceProtocol::Http)
        .and_then(|_| {
            harness
                .materialization
                .materialize_publication(publication.clone())
        })
        .expect("materialize publication");

    let before = harness
        .local_exposure
        .entry(publication.claim().name())
        .expect("local exposure before placement");
    assert_eq!(
        before.exposure().availability(),
        GatewayExposureAvailability::Available
    );
    assert_eq!(before.reachability(), None);
    assert_eq!(before.local_access(), None);
    assert_eq!(
        harness
            .connectivity
            .resolve_reachability(&materialized.route().id, ConnectivityScope::Local)
            .expect_err("route should not be placed yet"),
        fabric_resource_connectivity::ConnectivityError::NotFound
    );

    let placed = harness
        .local_exposure
        .ensure_local_placement(publication.claim().name())
        .expect("ensure local placement");
    let reachability = placed.reachability().expect("reachability after placement");
    assert_eq!(reachability.ingress_route_id, materialized.route().id);
    assert_eq!(placed.local_access(), None);

    let active = harness
        .local_exposure
        .activate_local_access(publication.claim().name())
        .expect("activate local access");
    let local_access = active.local_access().expect("active local access");
    assert_eq!(active.reachability(), placed.reachability());
    assert!(
        !local_access
            .url
            .contains(publication.claim().name().as_str()),
        "technical local access must not be derived from NamespaceName"
    );
}

#[test]
fn local_access_survives_endpoint_replacement_without_identity_changes() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let publication = publish_surface(&harness, &component, "notes", &surface);
    let service = harness.ensure_http_service("service.notes.web", "web");

    let first_server = spawn_loopback_server(200, "server-a");
    let first_target = register_ready_loopback_target(&harness, &service, "notes_a", &first_server);
    let materialized = harness
        .materialization
        .bind_service(surface.clone(), service.id.clone(), ServiceProtocol::Http)
        .and_then(|_| {
            harness
                .materialization
                .materialize_publication(publication.clone())
        })
        .expect("materialize service-backed publication");
    let first = harness
        .local_exposure
        .activate_local_access(publication.claim().name())
        .expect("activate local access");
    let first_entry = first.exposure().entry().clone();
    let first_reachability = first.reachability().expect("first reachability").clone();
    let first_access = first.local_access().expect("first local access").clone();

    let first_response = ureq::get(&first_access.url).call().expect("first request");
    assert_eq!(
        first_response
            .into_body()
            .read_to_string()
            .expect("first body"),
        "server-a"
    );

    let second_server = spawn_loopback_server(200, "server-b");
    replace_with_new_loopback_target(&harness, &service, &first_target, "notes_b", &second_server);

    let second = harness
        .local_exposure
        .entry(publication.claim().name())
        .expect("second local exposure");
    let second_response = ureq::get(&first_access.url).call().expect("second request");
    assert_eq!(
        second_response
            .into_body()
            .read_to_string()
            .expect("second body"),
        "server-b"
    );

    assert_eq!(second.exposure().entry(), &first_entry);
    assert_eq!(second.reachability(), Some(&first_reachability));
    assert_eq!(second.local_access(), Some(&first_access));
    assert_eq!(
        second.exposure().entry().publication().claim().name(),
        publication.claim().name()
    );
    assert_eq!(
        harness
            .materialization
            .materialized_publication(publication.claim().name())
            .expect("materialized publication lookup")
            .route()
            .id,
        materialized.route().id
    );
}

#[test]
fn local_placement_persists_while_unavailable_service_returns_503() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let publication = publish_surface(&harness, &component, "notes", &surface);
    let service = harness.ensure_http_service("service.notes.web", "web");
    let target_id = register_ready_target(&harness, &service, "notes_a", 200, "server-a");
    let _materialized = harness
        .materialization
        .bind_service(surface, service.id.clone(), ServiceProtocol::Http)
        .and_then(|_| {
            harness
                .materialization
                .materialize_publication(publication.clone())
        })
        .expect("materialize publication");
    let active = harness
        .local_exposure
        .activate_local_access(publication.claim().name())
        .expect("activate local access");
    let expected_reachability = active
        .reachability()
        .expect("reachability before withdrawal")
        .clone();
    let expected_access = active
        .local_access()
        .expect("local access before withdrawal")
        .clone();

    drain_and_withdraw_target(&harness, &service, &target_id);

    let after = harness
        .local_exposure
        .entry(publication.claim().name())
        .expect("local exposure after withdrawal");
    assert_eq!(
        after.exposure().availability(),
        GatewayExposureAvailability::MaterializedUnavailable
    );
    assert_eq!(after.reachability(), Some(&expected_reachability));
    assert_eq!(after.local_access(), Some(&expected_access));

    match ureq::get(&expected_access.url).call() {
        Err(ureq::Error::StatusCode(503)) => {}
        other => panic!("expected unavailable service status code 503, got {other:?}"),
    }
}

#[test]
fn unknown_namespace_name_fails_through_local_gateway_exposure_with_component_semantics() {
    let harness = MaterializationHarness::new("instance.local");
    let error = harness
        .local_exposure
        .entry(&NamespaceName::new("missing").expect("namespace name"))
        .expect_err("unknown local gateway exposure name must fail");

    assert_eq!(
        error,
        ServiceMaterializationError::Gateway(GatewayError::Publication(Box::new(
            PublicationError::UnknownPublication(
                NamespaceName::new("missing").expect("namespace name"),
            )
        ),))
    );
}

#[test]
fn published_but_unplaced_entry_is_not_locally_name_resolvable() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let publication = publish_surface(&harness, &component, "notes", &surface);
    let local_name = LocalNetworkName::new("notes.stel").expect("local network name");

    let error = harness
        .local_names
        .assign_local_name(publication.claim().name(), local_name)
        .expect_err("published but unplaced entry must not be nameable");

    assert_eq!(
        error,
        ServiceMaterializationError::LocalNameRequiresLocalPlacement(
            publication.claim().name().clone()
        )
    );
}

#[test]
fn placed_but_unnamed_entry_does_not_magically_resolve_through_local_names() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let publication = publish_surface(&harness, &component, "notes", &surface);
    let service = harness.ensure_http_service("service.notes.web", "web");
    let _target_id = register_ready_target(&harness, &service, "notes_a", 200, "server-a");

    harness
        .materialization
        .bind_service(surface, service.id.clone(), ServiceProtocol::Http)
        .and_then(|_| {
            harness
                .materialization
                .materialize_publication(publication.clone())
        })
        .expect("materialize publication");

    let active = harness
        .local_exposure
        .activate_local_access(publication.claim().name())
        .expect("activate local access");
    assert!(active.local_access().is_some());

    let error = harness
        .local_names
        .resolve_local_name(&LocalNetworkName::new("notes.stel").expect("local network name"))
        .expect_err("unnamed entry must not resolve through local names");

    assert_eq!(
        error,
        ServiceMaterializationError::UnknownLocalNetworkName(
            LocalNetworkName::new("notes.stel").expect("local network name")
        )
    );
}

#[test]
fn duplicate_local_name_claims_fail_deterministically() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");

    let alpha_surface = register_surface(&harness, &component, "alpha.main");
    let alpha_publication = publish_surface(&harness, &component, "alpha", &alpha_surface);
    let alpha_service = harness.ensure_http_service("service.alpha.web", "web");
    let _alpha_target = register_ready_target(&harness, &alpha_service, "alpha_a", 200, "alpha");
    harness
        .materialization
        .bind_service(
            alpha_surface,
            alpha_service.id.clone(),
            ServiceProtocol::Http,
        )
        .and_then(|_| {
            harness
                .materialization
                .materialize_publication(alpha_publication.clone())
        })
        .expect("materialize alpha");
    harness
        .local_exposure
        .activate_local_access(alpha_publication.claim().name())
        .expect("activate alpha");

    let beta_surface = register_surface(&harness, &component, "beta.main");
    let beta_publication = publish_surface(&harness, &component, "beta", &beta_surface);
    let beta_service = harness.ensure_http_service("service.beta.web", "web");
    let _beta_target = register_ready_target(&harness, &beta_service, "beta_a", 200, "beta");
    harness
        .materialization
        .bind_service(beta_surface, beta_service.id.clone(), ServiceProtocol::Http)
        .and_then(|_| {
            harness
                .materialization
                .materialize_publication(beta_publication.clone())
        })
        .expect("materialize beta");
    harness
        .local_exposure
        .activate_local_access(beta_publication.claim().name())
        .expect("activate beta");

    let local_name = LocalNetworkName::new("notes.stel").expect("local network name");
    harness
        .local_names
        .assign_local_name(alpha_publication.claim().name(), local_name.clone())
        .expect("assign alpha local name");
    let error = harness
        .local_names
        .assign_local_name(beta_publication.claim().name(), local_name.clone())
        .expect_err("duplicate local name must fail");

    assert_eq!(
        error,
        ServiceMaterializationError::LocalNetworkNameAlreadyAllocated(local_name)
    );
}

#[test]
fn local_name_resolution_reaches_current_workload_and_preserves_distinct_identities() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let publication = publish_surface(&harness, &component, "notes", &surface);
    let service = harness.ensure_http_service("service.notes.web", "web");
    let first_server = spawn_loopback_server(200, "server-a");
    let first_target = register_ready_loopback_target(&harness, &service, "notes_a", &first_server);

    let materialized = harness
        .materialization
        .bind_service(surface.clone(), service.id.clone(), ServiceProtocol::Http)
        .and_then(|_| {
            harness
                .materialization
                .materialize_publication(publication.clone())
        })
        .expect("materialize publication");
    let active = harness
        .local_exposure
        .activate_local_access(publication.claim().name())
        .expect("activate local access");
    let local_name = LocalNetworkName::new("notes.stel").expect("local network name");
    let first = harness
        .local_names
        .assign_local_name(publication.claim().name(), local_name.clone())
        .expect("assign local name");

    assert_eq!(
        first.projection().namespace_name(),
        publication.claim().name()
    );
    assert_eq!(first.projection().local_name(), &local_name);
    assert_eq!(
        first.projection().reachability_id(),
        &active.reachability().expect("active reachability").id
    );
    assert_ne!(
        first.projection().namespace_name().as_str(),
        first.projection().local_name().as_str()
    );

    let first_access = first
        .exposure()
        .local_access()
        .expect("resolved local access")
        .clone();
    let first_response = ureq::get(&first_access.url).call().expect("first request");
    assert_eq!(
        first_response
            .into_body()
            .read_to_string()
            .expect("first body"),
        "server-a"
    );

    let second_server = spawn_loopback_server(200, "server-b");
    replace_with_new_loopback_target(&harness, &service, &first_target, "notes_b", &second_server);

    let second = harness
        .local_names
        .resolve_local_name(&local_name)
        .expect("resolve same local name");
    let second_response = ureq::get(
        &second
            .exposure()
            .local_access()
            .expect("second local access")
            .url,
    )
    .call()
    .expect("second request");
    assert_eq!(
        second_response
            .into_body()
            .read_to_string()
            .expect("second body"),
        "server-b"
    );

    assert_eq!(second.projection(), first.projection());
    assert_eq!(
        second.exposure().exposure().entry(),
        first.exposure().exposure().entry()
    );
    assert_eq!(
        second
            .exposure()
            .reachability()
            .expect("second reachability")
            .id,
        first.projection().reachability_id().clone()
    );
    assert_eq!(
        harness
            .materialization
            .materialized_publication(publication.claim().name())
            .expect("materialized publication lookup")
            .route()
            .id,
        materialized.route().id
    );
}

#[test]
fn unavailable_service_keeps_local_name_and_returns_503() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");
    let surface = register_surface(&harness, &component, "notes.main");
    let publication = publish_surface(&harness, &component, "notes", &surface);
    let service = harness.ensure_http_service("service.notes.web", "web");
    let target_id = register_ready_target(&harness, &service, "notes_a", 200, "server-a");

    harness
        .materialization
        .bind_service(surface, service.id.clone(), ServiceProtocol::Http)
        .and_then(|_| {
            harness
                .materialization
                .materialize_publication(publication.clone())
        })
        .expect("materialize publication");
    harness
        .local_exposure
        .activate_local_access(publication.claim().name())
        .expect("activate local access");

    let local_name = LocalNetworkName::new("notes.stel").expect("local network name");
    let named = harness
        .local_names
        .assign_local_name(publication.claim().name(), local_name.clone())
        .expect("assign local name");

    drain_and_withdraw_target(&harness, &service, &target_id);

    let resolved = harness
        .local_names
        .resolve_local_name(&local_name)
        .expect("resolve local name after withdrawal");
    assert_eq!(resolved.projection(), named.projection());
    assert_eq!(
        resolved.exposure().exposure().availability(),
        GatewayExposureAvailability::MaterializedUnavailable
    );

    match ureq::get(
        &resolved
            .exposure()
            .local_access()
            .expect("local access remains stable")
            .url,
    )
    .call()
    {
        Err(ureq::Error::StatusCode(503)) => {}
        other => panic!("expected unavailable service status code 503, got {other:?}"),
    }
}

#[test]
fn local_name_list_is_deterministic() {
    let harness = MaterializationHarness::new("instance.local");
    let component = harness.component("notes");

    for (surface_id, publication_name, service_scope, local_name) in [
        ("zeta.main", "zeta", "service.zeta.web", "zeta.stel"),
        ("alpha.main", "alpha", "service.alpha.web", "alpha.stel"),
    ] {
        let surface = register_surface(&harness, &component, surface_id);
        let publication = publish_surface(&harness, &component, publication_name, &surface);
        let service = harness.ensure_http_service(service_scope, "web");
        let _target = register_ready_target(
            &harness,
            &service,
            &format!("{publication_name}_a"),
            200,
            publication_name,
        );
        harness
            .materialization
            .bind_service(surface, service.id.clone(), ServiceProtocol::Http)
            .and_then(|_| {
                harness
                    .materialization
                    .materialize_publication(publication.clone())
            })
            .expect("materialize publication");
        harness
            .local_exposure
            .activate_local_access(publication.claim().name())
            .expect("activate local access");
        harness
            .local_names
            .assign_local_name(
                publication.claim().name(),
                LocalNetworkName::new(local_name).expect("local network name"),
            )
            .expect("assign local name");
    }

    let local_names: Vec<String> = harness
        .local_names
        .local_names()
        .expect("list local names")
        .into_iter()
        .map(|resolution| resolution.projection().local_name().as_str().to_owned())
        .collect();

    assert_eq!(
        local_names,
        vec!["alpha.stel".to_owned(), "zeta.stel".to_owned()]
    );
}
