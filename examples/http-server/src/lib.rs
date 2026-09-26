//! HTTP server Composition example for Fabric Ecosystem.

use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::thread;

use fabric::prelude::*;
use fabric_composition_http_server::{build_http_server_composition, HttpServerCompositionConfig};
use fabric_package_networking_http::{HttpResponse, HttpServer, HttpServerInstanceApi};
use fabric_package_networking_tcp::{
    TcpSocketAddress, TcpTransportInspector, TcpTransportInspectorInstanceApi,
};
use futures::executor::block_on;

pub fn run() -> Result<String, Box<dyn std::error::Error>> {
    let composition = build_http_server_composition(
        "fabric.ecosystem.example.http-server",
        HttpServerCompositionConfig::local("api"),
    )?;
    let mut instance = composition.materialize_on(
        "fabric.ecosystem.example.http-server.local",
        &HostDescriptor::native(),
    )?;
    instance.start()?;

    let inspector = instance.component::<TcpTransportInspector>()?;
    inspector.reconcile()?;
    let address = block_on(inspector.inspect_transport())?
        .actual
        .ok_or("http server did not bind a TCP address")?;
    let server = instance.component::<HttpServer>()?;
    server.reconcile()?;
    let client = http_client(address);

    let exchange = block_on(server.accept_exchange())??;
    let request = exchange.request().clone();
    exchange.respond(HttpResponse::new(
        200,
        format!("hello from {}", request.target).into_bytes(),
    ))?;
    let response = client.join().expect("client");
    instance.stop()?;

    Ok(format!("{} {}", request.method, response))
}

fn http_client(address: TcpSocketAddress) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let socket: SocketAddr = format!("{}:{}", address.host, address.port)
            .parse()
            .expect("socket addr");
        let mut stream = TcpStream::connect(socket).expect("client connect");
        stream
            .write_all(b"GET /example HTTP/1.1\r\nHost: example.local\r\n\r\n")
            .expect("write request");
        stream.shutdown(Shutdown::Write).expect("shutdown write");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read response");
        String::from_utf8(response).expect("utf8 response")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_uses_reusable_http_server_composition() {
        let output = run().expect("example");
        assert!(output.starts_with("GET HTTP/1.1 200 OK\r\n"));
        assert!(output.ends_with("\r\n\r\nhello from /example"));
    }
}
