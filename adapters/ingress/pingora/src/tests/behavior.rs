use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use fabric_resource_ingress::IngressAdapter;
use fabric_resource_service::{MarkServiceTargetDrainingRequest, WithdrawServiceTargetRequest};

use crate::tests::harness::{
    PingoraHarness, ensure_route_and_access, ensure_service, register_ready_loopback_target,
    register_ready_target, spawn_loopback_server,
};
use crate::{PingoraIngressAdapter, PingoraIngressConfig};

#[test]
fn pingora_ingress_rejects_non_loopback_bind_addresses() {
    let provider = PingoraIngressAdapter::new(PingoraIngressConfig::new(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        0,
    )));
    let error = provider
        .initialize()
        .expect_err("non-loopback bind must fail");
    assert!(
        error
            .to_string()
            .contains("local ingress must bind only to loopback"),
        "unexpected error: {error}"
    );
}

#[test]
fn pingora_ingress_routes_real_loopback_requests_to_service_endpoints() {
    let harness = PingoraHarness::new();
    let service = ensure_service(&harness);
    register_ready_target(&harness, &service, "pingora_target", 202, "pingora-ok");
    let (route, access) = ensure_route_and_access(&harness, &service);

    let response = ureq::get(&format!(
        "{}/notes?tag=blue&tag=green&encoded=a%2Fb&space=hello%20world",
        access.url
    ))
    .header("x-fabric-proof", "pingora")
    .call()
    .expect("request");

    assert_eq!(route.target.service_id, service.id);
    assert_eq!(response.status(), 202);
    assert_eq!(
        response.into_body().read_to_string().expect("body"),
        "pingora-ok"
    );

    let requests = harness.probe.requests.lock().expect("probe");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].endpoint_id, service.endpoint.id);
    assert_eq!(requests[0].request.method, "GET");
    assert_eq!(
        requests[0].request.url,
        "http://service.local/notes?tag=blue&tag=green&encoded=a%2Fb&space=hello%20world"
    );
}

#[test]
fn pingora_ingress_route_survives_live_endpoint_replacement_without_recreation() {
    let harness = PingoraHarness::new();
    let service = ensure_service(&harness);
    let first_server = spawn_loopback_server(200, "server-a");
    let first_target_id =
        register_ready_loopback_target(&harness, &service, "loopback_a", &first_server);
    let (route, access) = ensure_route_and_access(&harness, &service);
    let first_live_endpoint = harness
        .service
        .resolve_live_endpoint(&service.id)
        .expect("resolve first live endpoint");

    let first_response = ureq::get(&access.url).call().expect("first request");
    assert_eq!(
        first_response
            .into_body()
            .read_to_string()
            .expect("first body"),
        "server-a"
    );

    harness
        .service
        .mark_target_draining(MarkServiceTargetDrainingRequest {
            service_id: service.id.clone(),
            target_id: first_target_id.clone(),
        })
        .expect("drain first target");
    harness
        .service
        .withdraw_target(WithdrawServiceTargetRequest {
            service_id: service.id.clone(),
            target_id: first_target_id,
        })
        .expect("withdraw first target");

    let second_server = spawn_loopback_server(200, "server-b");
    register_ready_loopback_target(&harness, &service, "loopback_b", &second_server);
    let second_live_endpoint = harness
        .service
        .resolve_live_endpoint(&service.id)
        .expect("resolve second live endpoint");
    let resolved_route = harness
        .ingress
        .resolve_route(&service.id)
        .expect("resolve route after replacement");

    assert_eq!(resolved_route.id, route.id);
    assert_eq!(resolved_route.target.service_id, route.target.service_id);
    assert_eq!(access.route_id, route.id);
    assert_ne!(
        first_live_endpoint.target_id,
        second_live_endpoint.target_id
    );

    let second_response = ureq::get(&access.url).call().expect("second request");
    assert_eq!(
        second_response
            .into_body()
            .read_to_string()
            .expect("second body"),
        "server-b"
    );
}

#[test]
fn pingora_ingress_does_not_route_to_stale_endpoint_after_service_becomes_unavailable() {
    let harness = PingoraHarness::new();
    let service = ensure_service(&harness);
    let target_id = register_ready_target(&harness, &service, "stale_target", 200, "stale-ok");
    let (_route, access) = ensure_route_and_access(&harness, &service);

    harness
        .service
        .mark_target_draining(MarkServiceTargetDrainingRequest {
            service_id: service.id.clone(),
            target_id: target_id.clone(),
        })
        .expect("drain target");
    harness
        .service
        .withdraw_target(WithdrawServiceTargetRequest {
            service_id: service.id.clone(),
            target_id,
        })
        .expect("withdraw target");

    match ureq::get(&access.url).call() {
        Err(ureq::Error::StatusCode(503)) => {}
        other => panic!("expected unavailable service status code 503, got {other:?}"),
    }
}

#[test]
fn pingora_ingress_preserves_service_response_status_codes() {
    let harness = PingoraHarness::new();
    let service = ensure_service(&harness);
    register_ready_target(&harness, &service, "pingora_target", 503, "upstream-busy");
    let (_route, access) = ensure_route_and_access(&harness, &service);

    match ureq::get(&access.url).call() {
        Err(ureq::Error::StatusCode(503)) => {}
        other => panic!("expected service status code to remain 503, got {other:?}"),
    }
}

#[test]
fn pingora_ingress_route_identity_is_stable_across_restart_while_local_access_changes() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let root = tempdir.path().to_path_buf();

    let (first_route_id, first_url) = {
        let harness = PingoraHarness::from_existing_root(&root);
        let service = ensure_service(&harness);
        register_ready_target(&harness, &service, "restart_target", 200, "restart");
        let (route, access) = ensure_route_and_access(&harness, &service);
        (route.id, access.url)
    };

    let harness = PingoraHarness::from_existing_root(&root);
    let service = ensure_service(&harness);
    register_ready_target(&harness, &service, "restart_target_two", 200, "restart-two");
    let (second_route, second_access) = ensure_route_and_access(&harness, &service);

    assert_eq!(second_route.id, first_route_id);
    assert_ne!(second_access.url, first_url);

    let response = ureq::get(&second_access.url)
        .call()
        .expect("request after restart");
    assert_eq!(
        response.into_body().read_to_string().expect("body"),
        "restart-two"
    );

    let old_access = ureq::get(&first_url).call();
    assert!(
        old_access.is_err(),
        "old loopback access should be stale after provider restart"
    );
}
