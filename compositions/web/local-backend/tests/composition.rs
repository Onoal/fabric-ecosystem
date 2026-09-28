mod support;

use fabric::prelude::*;
use fabric_composition_local_backend::{local_backend_stack, LocalBackendCompositionConfig};
use fabric_package_networking_http::{HttpResponse, HttpServer, HttpServerInstanceApi};
use fabric_package_networking_tcp::{
    TcpSocketAddress, TcpTransportInspection, TcpTransportInspector,
    TcpTransportInspectorInstanceApi,
};
use futures::executor::block_on;
use support::app::{test_app, TestLocalBackendApp, TestLocalBackendAppInstanceApi};
use support::client::http_client;
use support::temp_db::TempDatabase;

fn composition(id: &str, config: LocalBackendCompositionConfig) -> Composition {
    Fabric::new(id)
        .expect("fabric")
        .with(local_backend_stack(config))
        .build()
        .expect("composition")
}

fn config(path: impl Into<std::path::PathBuf>) -> LocalBackendCompositionConfig {
    LocalBackendCompositionConfig::local("api", "primary", path, "application")
}

fn started_instance(composition: &Composition, id: &str) -> Instance {
    let mut instance = composition
        .materialize_on(id, &HostDescriptor::native())
        .expect("instance");
    instance.start().expect("start");
    instance
}

fn activate<C: fabric::authoring::ComponentDefinition>(
    instance: &Instance,
) -> BoundComponent<'_, C> {
    let component = instance.component::<C>().expect("component");
    component.reconcile().expect("reconcile");
    component
}

fn actual_address(inspector: &BoundComponent<'_, TcpTransportInspector>) -> TcpSocketAddress {
    let observation: TcpTransportInspection =
        block_on(inspector.inspect_transport()).expect("observe");
    observation.actual.expect("actual address")
}

#[test]
fn standalone_composition_declares_expected_semantic_graph() {
    let database = TempDatabase::new("inspect");
    let composition = composition(
        "onoal.composition.test.local-backend.inspect",
        config(database.path_buf()),
    );

    assert_eq!(composition.resources().count(), 3);
    assert_eq!(composition.components().count(), 2);
    assert!(composition.resources().any(|resource| {
        resource.resource_id().as_str() == "onoal.package.networking.tcp.byte-stream"
            && resource.name().as_str() == "api"
            && resource
                .realization()
                .adapter_definition_id()
                .expect("tcp adapter")
                .as_str()
                == "onoal.package.networking.tcp.loopback"
    }));
    assert!(composition.resources().any(|resource| {
        resource.resource_id().as_str() == "onoal.package.data.relational-database"
            && resource.name().as_str() == "primary"
            && resource
                .realization()
                .adapter_definition_id()
                .expect("sqlite adapter")
                .as_str()
                == "onoal.package.data.sqlite.relational-database"
    }));
    assert!(composition.resources().any(|resource| {
        resource.resource_id().as_str() == "onoal.package.observability.logging.sink"
            && resource.name().as_str() == "application"
            && resource
                .realization()
                .adapter_definition_id()
                .expect("console adapter")
                .as_str()
                == "onoal.package.observability.logging.console"
    }));
    assert!(composition.components().any(|component| {
        component.component_id().as_str() == "onoal.package.networking.tcp.inspector"
    }));
    assert!(composition.components().any(|component| {
        component.component_id().as_str() == "onoal.package.networking.http.server"
    }));
    assert!(composition.relations().iter().any(|relation| {
        relation.role().as_str() == "transport"
            && matches!(
                relation.resolved_target(),
                SemanticRelationTargetOccurrence::Resource { resource_name, .. }
                    if resource_name.as_str() == "api"
            )
    }));
}

#[test]
fn custom_names_are_preserved() {
    let database = TempDatabase::new("names");
    let composition = composition(
        "onoal.composition.test.local-backend.names",
        LocalBackendCompositionConfig::local("admin", "state", database.path_buf(), "backend-log"),
    );

    assert!(composition
        .resources()
        .any(|resource| resource.name().as_str() == "admin"));
    assert!(composition
        .resources()
        .any(|resource| resource.name().as_str() == "state"));
    assert!(composition
        .resources()
        .any(|resource| resource.name().as_str() == "backend-log"));
}

#[test]
fn in_memory_config_uses_sqlite_in_memory_realization() {
    let composition = composition(
        "onoal.composition.test.local-backend.in-memory",
        LocalBackendCompositionConfig::in_memory("api", "primary", "application"),
    );

    assert!(composition.resources().any(|resource| {
        resource.resource_id().as_str() == "onoal.package.data.relational-database"
            && resource.name().as_str() == "primary"
            && resource
                .realization()
                .adapter_definition_id()
                .expect("sqlite adapter")
                .as_str()
                == "onoal.package.data.sqlite.relational-database"
    }));
}

#[test]
fn local_backend_runs_real_http_exchange() {
    let database = TempDatabase::new("http");
    let composition = composition(
        "onoal.composition.test.local-backend.http",
        config(database.path_buf()),
    );
    let instance = started_instance(
        &composition,
        "onoal.composition.test.local-backend.http.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let server = activate::<HttpServer>(&instance);
    let address = actual_address(&inspector);
    let client = http_client(address, "/backend");

    let exchange = block_on(server.accept_exchange())
        .expect("accept")
        .expect("exchange");
    let request = exchange.request().clone();
    exchange
        .respond(HttpResponse::new(
            200,
            format!("local-backend:{}", request.target).into_bytes(),
        ))
        .expect("respond");
    let response = client.join().expect("client");

    assert_eq!(request.target, "/backend");
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.ends_with("\r\n\r\nlocal-backend:/backend"));
}

#[test]
fn consumer_owned_app_uses_database_and_logging() {
    let database = TempDatabase::new("app");
    let composition = Fabric::new("onoal.composition.test.local-backend.app")
        .expect("fabric")
        .with(local_backend_stack(config(database.path_buf())))
        .with(test_app("primary", "application"))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.composition.test.local-backend.app.instance",
    );
    let app = activate::<TestLocalBackendApp>(&instance);

    let result = block_on(app.write_read_log("stored-value".to_owned()))
        .expect("app")
        .expect("result");
    assert_eq!(result.value.as_deref(), Some("stored-value"));
    assert_eq!(result.logged.target.as_deref(), Some("local-backend-test"));
}

#[test]
fn database_persistence_survives_fresh_generation() {
    let database = TempDatabase::new("persist");
    let composition = Fabric::new("onoal.composition.test.local-backend.persist")
        .expect("fabric")
        .with(local_backend_stack(config(database.path_buf())))
        .with(test_app("primary", "application"))
        .build()
        .expect("composition");
    {
        let mut instance = started_instance(
            &composition,
            "onoal.composition.test.local-backend.persist.a",
        );
        let app = activate::<TestLocalBackendApp>(&instance);
        let result = block_on(app.write_read_log("durable-value".to_owned()))
            .expect("app")
            .expect("result");
        assert_eq!(result.value.as_deref(), Some("durable-value"));
        instance.stop().expect("stop");
    }
    {
        let mut instance = started_instance(
            &composition,
            "onoal.composition.test.local-backend.persist.b",
        );
        let app = activate::<TestLocalBackendApp>(&instance);
        let value = block_on(app.read_value())
            .expect("read")
            .expect("read result");
        assert_eq!(value.as_deref(), Some("durable-value"));
        instance.stop().expect("stop");
    }
}

#[test]
fn application_data_can_be_used_as_http_response_body_without_direct_component_relation() {
    let database = TempDatabase::new("http-app");
    let composition = Fabric::new("onoal.composition.test.local-backend.http-app")
        .expect("fabric")
        .with(local_backend_stack(config(database.path_buf())))
        .with(test_app("primary", "application"))
        .build()
        .expect("composition");
    let instance = started_instance(
        &composition,
        "onoal.composition.test.local-backend.http-app.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let server = activate::<HttpServer>(&instance);
    let app = activate::<TestLocalBackendApp>(&instance);
    let address = actual_address(&inspector);
    let client = http_client(address, "/from-app");

    let exchange = block_on(server.accept_exchange())
        .expect("accept")
        .expect("exchange");
    let request = exchange.request().clone();
    let app_result = block_on(app.write_read_log(format!("response{}", request.target)))
        .expect("app")
        .expect("result");
    let body = app_result.value.expect("value").into_bytes();
    exchange
        .respond(HttpResponse::new(200, body))
        .expect("respond");
    let response = client.join().expect("client");

    assert_eq!(request.target, "/from-app");
    assert!(response.ends_with("\r\n\r\nresponse/from-app"));
}

#[test]
fn multiple_local_backend_stacks_hit_current_component_identity_law() {
    let first = TempDatabase::new("multi-a");
    let second = TempDatabase::new("multi-b");
    let result = Fabric::new("onoal.composition.test.local-backend.multi")
        .expect("fabric")
        .with(local_backend_stack(LocalBackendCompositionConfig::local(
            "api",
            "primary",
            first.path_buf(),
            "application",
        )))
        .with(local_backend_stack(LocalBackendCompositionConfig::local(
            "admin",
            "state",
            second.path_buf(),
            "backend-log",
        )))
        .build();

    assert!(
        result.is_err(),
        "Fabric v1 Component definition identity prevents two HttpServer/TcpTransportInspector occurrences"
    );
}
