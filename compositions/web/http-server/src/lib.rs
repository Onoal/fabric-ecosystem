//! Reusable local HTTP server Composition artifact.
//!
//! This crate assembles existing TCP and HTTP packages into ordinary Fabric
//! Composition truth. It defines no new Fabric Resource, System, Component, or
//! Adapter.

use fabric::prelude::*;
use fabric_package_networking_tcp::TcpSocketAddress;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpServerCompositionConfig {
    pub transport_name: &'static str,
    pub bind: TcpSocketAddress,
}

impl HttpServerCompositionConfig {
    pub fn local(transport_name: &'static str) -> Self {
        Self {
            transport_name,
            bind: TcpSocketAddress::loopback_ephemeral(),
        }
    }

    pub fn bind(transport_name: &'static str, bind: TcpSocketAddress) -> Self {
        Self {
            transport_name,
            bind,
        }
    }
}

pub fn http_server_stack(config: HttpServerCompositionConfig) -> impl IntoFabricContribution {
    FabricContribution::new()
        .with(fabric_package_networking_tcp::loopback_tcp_transport_on(
            config.transport_name,
            config.bind,
        ))
        .with(fabric_package_networking_tcp::tcp_transport_inspector(
            config.transport_name,
        ))
        .with(fabric_package_networking_http::http_server(
            config.transport_name,
        ))
}

pub fn local_http_server(transport_name: &'static str) -> impl IntoFabricContribution {
    http_server_stack(HttpServerCompositionConfig::local(transport_name))
}

pub fn build_http_server_composition(
    id: &str,
    config: HttpServerCompositionConfig,
) -> Result<Composition, Box<dyn std::error::Error>> {
    Fabric::new(id)
        .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?
        .with(http_server_stack(config))
        .build()
        .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabric_package_networking_http::{
        HttpResponse, HttpServer, HttpServerInstanceApi, HttpVersion,
    };
    use fabric_package_networking_tcp::{
        TcpSocketAddress, TcpTransportInspection, TcpTransportInspector,
        TcpTransportInspectorInstanceApi,
    };
    use futures::executor::block_on;
    use std::io::{Read, Write};
    use std::net::{Shutdown, SocketAddr, TcpStream};
    use std::thread;

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

    fn http_client(address: TcpSocketAddress, target: &'static str) -> thread::JoinHandle<String> {
        thread::spawn(move || {
            let socket: SocketAddr = format!("{}:{}", address.host, address.port)
                .parse()
                .expect("socket addr");
            let mut stream = TcpStream::connect(socket).expect("client connect");
            stream
                .write_all(format!("GET {target} HTTP/1.1\r\nHost: local.test\r\n\r\n").as_bytes())
                .expect("write request");
            stream.shutdown(Shutdown::Write).expect("shutdown write");
            let mut response = Vec::new();
            stream.read_to_end(&mut response).expect("read response");
            String::from_utf8(response).expect("utf8 response")
        })
    }

    #[test]
    fn standalone_composition_declares_tcp_inspector_and_http_server_truth() {
        let composition = build_http_server_composition(
            "onoal.composition.test.http-server.inspect",
            HttpServerCompositionConfig::local("api"),
        )
        .expect("composition");

        assert_eq!(composition.resources().count(), 1);
        assert_eq!(composition.components().count(), 2);
        let resource = composition.resources().next().expect("resource");
        assert_eq!(
            resource.resource_id().as_str(),
            "onoal.package.networking.tcp.byte-stream"
        );
        assert_eq!(resource.name().as_str(), "api");
        assert!(composition
            .components()
            .any(|component| component.component_id().as_str()
                == "onoal.package.networking.tcp.inspector"));
        assert!(composition
            .components()
            .any(|component| component.component_id().as_str()
                == "onoal.package.networking.http.server"));
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "transport"
                && matches!(
                    relation.resolved_target(),
                    SemanticRelationTargetOccurrence::Resource { resource_name, .. }
                        if resource_name.as_str() == "api"
                )));
    }

    #[test]
    fn explicit_bind_address_is_preserved_as_requested_config() {
        let requested = TcpSocketAddress {
            host: "127.0.0.1".to_owned(),
            port: 0,
        };
        let composition = build_http_server_composition(
            "onoal.composition.test.http-server.bind",
            HttpServerCompositionConfig::bind("custom", requested.clone()),
        )
        .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.composition.test.http-server.bind.instance",
        );
        let inspector = activate::<TcpTransportInspector>(&instance);
        let observation = block_on(inspector.inspect_transport()).expect("observe");

        assert_eq!(observation.requested, requested);
        assert_eq!(observation.requested.port, 0);
        assert_ne!(observation.actual.expect("actual").port, 0);
    }

    #[test]
    fn standalone_composition_runs_real_http_exchange() {
        let composition = build_http_server_composition(
            "onoal.composition.test.http-server.runtime",
            HttpServerCompositionConfig::local("api"),
        )
        .expect("composition");
        let instance = started_instance(
            &composition,
            "onoal.composition.test.http-server.runtime.instance",
        );
        let inspector = activate::<TcpTransportInspector>(&instance);
        let server = activate::<HttpServer>(&instance);
        let address = actual_address(&inspector);
        let client = http_client(address, "/composition");

        let request = block_on(
            server.serve_once(
                HttpResponse::new(200, b"composition-http".to_vec())
                    .with_header("X-Composition", "http-server"),
            ),
        )
        .expect("serve")
        .expect("request");
        let response = client.join().expect("client");

        assert_eq!(request.method, "GET");
        assert_eq!(request.target, "/composition");
        assert_eq!(request.version, HttpVersion::Http11);
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("X-Composition: http-server\r\n"));
        assert!(response.ends_with("\r\n\r\ncomposition-http"));
    }

    #[test]
    fn reusable_stack_participates_in_larger_fabric_build() {
        let composition = Fabric::new("onoal.composition.test.http-server.reuse")
            .expect("fabric")
            .with(http_server_stack(HttpServerCompositionConfig::local("api")))
            .with(fabric_package_key_value::memory_key_value("scratch"))
            .build()
            .expect("composition");

        assert_eq!(composition.resources().count(), 2);
        assert!(composition
            .resources()
            .any(|resource| resource.name().as_str() == "scratch"));
        assert!(composition
            .resources()
            .any(|resource| resource.name().as_str() == "api"));
    }

    #[test]
    fn component_identity_currently_allows_one_http_server_definition_per_composition() {
        let result = Fabric::new("onoal.composition.test.http-server.two-stacks")
            .expect("fabric")
            .with(http_server_stack(HttpServerCompositionConfig::local("api")))
            .with(http_server_stack(HttpServerCompositionConfig::local(
                "admin",
            )))
            .build();

        assert!(
            result.is_err(),
            "Fabric v1 Component definition identity prevents two HttpServer occurrences"
        );

        let supported = Fabric::new("onoal.composition.test.http-server.named-transport")
            .expect("fabric")
            .with(fabric_package_networking_tcp::loopback_tcp_transport("api"))
            .with(http_server_stack(HttpServerCompositionConfig::local(
                "admin",
            )))
            .build()
            .expect("composition");
        assert!(supported
            .resources()
            .any(|resource| resource.resource_id().as_str()
                == "onoal.package.networking.tcp.byte-stream"
                && resource.name().as_str() == "admin"));
        assert!(supported
            .resources()
            .any(|resource| resource.resource_id().as_str()
                == "onoal.package.networking.tcp.byte-stream"
                && resource.name().as_str() == "api"));
        assert!(supported
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "transport"
                && matches!(
                    relation.resolved_target(),
                    SemanticRelationTargetOccurrence::Resource { resource_name, .. }
                        if resource_name.as_str() == "admin"
                )));
    }
}
