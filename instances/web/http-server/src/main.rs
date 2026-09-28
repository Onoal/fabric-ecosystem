use std::io::{self, Write};

use fabric::prelude::*;
use fabric_composition_http_server::{http_server_stack, HttpServerCompositionConfig};
use fabric_package_networking_http::{HttpResponse, HttpServer, HttpServerInstanceApi};
use fabric_package_networking_tcp::{
    TcpSocketAddress, TcpTransportInspector, TcpTransportInspectorInstanceApi,
};
use futures::executor::block_on;

const COMPOSITION_ID: &str = "fabric.ecosystem.instance.http-server";
const INSTANCE_ID: &str = "fabric.ecosystem.instance.http-server.local";
const TRANSPORT_NAME: &str = "api";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let composition = Fabric::new(COMPOSITION_ID)?
        .with(http_server_stack(HttpServerCompositionConfig::bind(
            TRANSPORT_NAME,
            requested_bind(),
        )))
        .build()?;
    let mut instance = composition.materialize_on(INSTANCE_ID, &HostDescriptor::native())?;
    instance.start()?;

    let inspector = instance.component::<TcpTransportInspector>()?;
    inspector.reconcile()?;
    let address = block_on(inspector.inspect_transport())?
        .actual
        .ok_or_else(|| io::Error::other("HTTP server did not bind a TCP address"))?;

    println!(
        "HTTP server listening on http://{}:{}",
        address.host, address.port
    );
    println!("Press Ctrl+C to stop.");
    io::stdout().flush()?;

    let server = instance.component::<HttpServer>()?;
    server.reconcile()?;
    loop {
        let exchange = block_on(server.accept_exchange())??;
        let request = exchange.request().clone();
        println!("request: {} {}", request.method, request.target);
        io::stdout().flush()?;
        let body = format!("hello from {}", request.target);
        exchange.respond(HttpResponse::new(200, body.into_bytes()))?;
    }
}

fn requested_bind() -> TcpSocketAddress {
    TcpSocketAddress {
        host: "127.0.0.1".to_owned(),
        port: 8080,
    }
}
