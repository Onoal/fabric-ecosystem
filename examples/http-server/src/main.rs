mod client;

use std::io::Error;

use fabric::prelude::*;
use fabric_composition_http_server::{http_server_stack, HttpServerCompositionConfig};
use fabric_package_networking_http::{HttpResponse, HttpServer, HttpServerInstanceApi};
use fabric_package_networking_tcp::{TcpTransportInspector, TcpTransportInspectorInstanceApi};
use futures::executor::block_on;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let composition = Fabric::new("fabric.ecosystem.example.http-server")?
        .with(http_server_stack(HttpServerCompositionConfig::local("api")))
        .build()?;
    let mut instance = composition.materialize_on(
        "fabric.ecosystem.example.http-server.local",
        &HostDescriptor::native(),
    )?;
    instance.start()?;

    let inspector = instance.component::<TcpTransportInspector>()?;
    inspector.reconcile()?;
    let address = block_on(inspector.inspect_transport())?
        .actual
        .ok_or_else(|| Error::other("HTTP server did not bind a TCP address"))?;
    let server = instance.component::<HttpServer>()?;
    server.reconcile()?;

    let target = "/example";
    let client = client::get(address.clone(), target);
    let exchange = block_on(server.accept_exchange())??;
    let request = exchange.request().clone();
    let body = format!("hello from {}", request.target);
    exchange.respond(HttpResponse::new(200, body.clone().into_bytes()))?;
    let response = join_client(client)?;

    instance.stop()?;

    println!("HTTP server bound to {}:{}", address.host, address.port);
    println!("request: {} {}", request.method, request.target);
    println!("response: 200 {body}");
    println!("raw response bytes: {}", response.len());

    Ok(())
}

fn join_client(
    client: std::thread::JoinHandle<std::io::Result<String>>,
) -> Result<String, Box<dyn std::error::Error>> {
    client
        .join()
        .map_err(|_| Error::other("HTTP client thread panicked"))?
        .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
}
