use fabric::prelude::*;
use fabric_package_networking_http::{
    http_server, HttpError, HttpErrorKind, HttpResponse, HttpServer, HttpServerInstanceApi,
    DEFAULT_MAX_BODY_BYTES,
};
use fabric_package_networking_tcp::{
    loopback_tcp_transport, tcp_transport_inspector, TcpSocketAddress, TcpTransportInspection,
    TcpTransportInspector, TcpTransportInspectorInstanceApi,
};
use futures::executor::block_on;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::thread;

fn composition(id: &str) -> Composition {
    Fabric::new(id)
        .expect("fabric")
        .with(loopback_tcp_transport("api"))
        .with(tcp_transport_inspector("api"))
        .with(http_server("api"))
        .build()
        .expect("composition")
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

fn to_socket(address: &TcpSocketAddress) -> SocketAddr {
    format!("{}:{}", address.host, address.port)
        .parse()
        .expect("socket addr")
}

fn client_exchange(address: TcpSocketAddress, request: Vec<u8>) -> Vec<u8> {
    let mut stream = TcpStream::connect(to_socket(&address)).expect("client connect");
    stream.write_all(&request).expect("write request");
    stream.shutdown(Shutdown::Write).expect("shutdown write");
    let mut response = Vec::new();
    stream.read_to_end(&mut response).expect("read response");
    response
}

#[test]
fn get_response_depends_on_request_target() {
    let composition = composition("onoal.package.test.http.exchange.get");
    let instance = started_instance(
        &composition,
        "onoal.package.test.http.exchange.get.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let server = activate::<HttpServer>(&instance);
    let address = actual_address(&inspector);
    let client = thread::spawn(move || {
        client_exchange(
            address,
            b"GET /alpha HTTP/1.1\r\nHost: example.test\r\nAccept: text/plain\r\n\r\n".to_vec(),
        )
    });

    let exchange = block_on(server.accept_exchange())
        .expect("accept")
        .expect("exchange");
    let target = exchange.request().target.clone();
    assert_eq!(exchange.request().method, "GET");
    assert!(exchange.request().body.is_empty());
    exchange
        .respond(
            HttpResponse::new(200, format!("target={target}").into_bytes())
                .with_header("Content-Type", "text/plain")
                .with_header("X-Package", "fabric-http"),
        )
        .expect("respond");
    let response = String::from_utf8(client.join().expect("client")).expect("utf8 response");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("Content-Type: text/plain\r\n"));
    assert!(response.contains("X-Package: fabric-http\r\n"));
    assert!(response.contains("Content-Length: 13\r\n"));
    assert!(response.ends_with("\r\n\r\ntarget=/alpha"));
}

#[test]
fn post_response_depends_on_request_body_without_client_eof() {
    let composition = composition("onoal.package.test.http.exchange.post");
    let instance = started_instance(
        &composition,
        "onoal.package.test.http.exchange.post.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let server = activate::<HttpServer>(&instance);
    let address = actual_address(&inspector);
    let (response_tx, response_rx) = std::sync::mpsc::channel();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(to_socket(&address)).expect("client connect");
        stream
            .write_all(
                b"POST /submit HTTP/1.1\r\nHost: example.test\r\nContent-Length: 11\r\nX-Mode: test\r\n\r\nhello=world",
            )
            .expect("write request");
        let mut response = [0; 256];
        let count = stream.read(&mut response).expect("read response");
        response_tx
            .send(response[..count].to_vec())
            .expect("send response");
        stream.shutdown(Shutdown::Both).expect("shutdown");
    });

    let exchange = block_on(server.accept_exchange())
        .expect("accept")
        .expect("exchange");
    assert_eq!(exchange.request().target, "/submit");
    assert_eq!(exchange.request().body, b"hello=world");
    let body = format!(
        "method={}, body={}",
        exchange.request().method,
        String::from_utf8(exchange.request().body.clone()).expect("body")
    );
    exchange
        .respond(HttpResponse::new(201, body.into_bytes()))
        .expect("respond");
    let response = String::from_utf8(response_rx.recv().expect("response")).expect("utf8");
    client.join().expect("client");

    assert!(response.starts_with("HTTP/1.1 201 Created\r\n"));
    assert!(response.contains("Content-Length: 29\r\n"));
    assert!(response.ends_with("\r\n\r\nmethod=POST, body=hello=world"));
}

#[test]
fn response_header_injection_is_rejected() {
    let composition = composition("onoal.package.test.http.exchange.validation");
    let instance = started_instance(
        &composition,
        "onoal.package.test.http.exchange.validation.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let server = activate::<HttpServer>(&instance);
    let address = actual_address(&inspector);
    let client =
        thread::spawn(move || client_exchange(address, b"GET / HTTP/1.1\r\n\r\n".to_vec()));

    let exchange = block_on(server.accept_exchange())
        .expect("accept")
        .expect("exchange");
    let error = exchange
        .respond(HttpResponse::new(200, b"bad".to_vec()).with_header("X-Test", "ok\r\nBad: yes"))
        .expect_err("invalid response");
    assert_eq!(error.kind, HttpErrorKind::InvalidResponse);
    let _ = client.join().expect("client");
}

#[test]
fn conflicting_content_length_is_rejected() {
    let composition = composition("onoal.package.test.http.exchange.length");
    let instance = started_instance(
        &composition,
        "onoal.package.test.http.exchange.length.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let server = activate::<HttpServer>(&instance);
    let address = actual_address(&inspector);
    let client = thread::spawn(move || {
        client_exchange(
            address,
            b"POST / HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\nab".to_vec(),
        )
    });

    let result = block_on(server.accept_exchange()).expect("accept result");
    assert!(matches!(
        result,
        Err(HttpError {
            kind: HttpErrorKind::MalformedRequest,
            ..
        })
    ));
    let _ = client.join().expect("client");
}

#[test]
fn unsupported_chunked_transfer_encoding_is_bounded() {
    let composition = composition("onoal.package.test.http.exchange.chunked");
    let instance = started_instance(
        &composition,
        "onoal.package.test.http.exchange.chunked.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let server = activate::<HttpServer>(&instance);
    let address = actual_address(&inspector);
    let client = thread::spawn(move || {
        client_exchange(
            address,
            b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n".to_vec(),
        )
    });

    let result = block_on(server.accept_exchange()).expect("accept result");
    assert!(matches!(
        result,
        Err(HttpError {
            kind: HttpErrorKind::UnsupportedTransferEncoding,
            ..
        })
    ));
    let _ = client.join().expect("client");
}

#[test]
fn request_size_bound_is_enforced() {
    let composition = composition("onoal.package.test.http.exchange.bounds");
    let instance = started_instance(
        &composition,
        "onoal.package.test.http.exchange.bounds.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let server = activate::<HttpServer>(&instance);
    let address = actual_address(&inspector);
    let oversized = format!(
        "POST /too-large HTTP/1.1\r\nHost: example.test\r\nContent-Length: {}\r\n\r\n",
        DEFAULT_MAX_BODY_BYTES + 1
    );
    let client = thread::spawn(move || client_exchange(address, oversized.into_bytes()));

    let result = block_on(server.accept_exchange()).expect("accept result");
    assert!(matches!(
        result,
        Err(HttpError {
            kind: HttpErrorKind::RequestTooLarge,
            ..
        })
    ));
    let _ = client.join().expect("client");
}

#[test]
fn stopped_generation_rejects_exchange_response() {
    let composition = composition("onoal.package.test.http.exchange.lifecycle");
    let mut instance = started_instance(
        &composition,
        "onoal.package.test.http.exchange.lifecycle.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let server = activate::<HttpServer>(&instance);
    let address = actual_address(&inspector);
    let client =
        thread::spawn(move || client_exchange(address, b"GET / HTTP/1.1\r\n\r\n".to_vec()));

    let exchange = block_on(server.accept_exchange())
        .expect("accept")
        .expect("exchange");
    instance.stop().expect("stop");
    let error = exchange
        .respond(HttpResponse::new(200, b"too-late".to_vec()))
        .expect_err("stopped response");
    assert!(matches!(
        error.kind,
        HttpErrorKind::WriteResponseFailed | HttpErrorKind::Transport
    ));
    let _ = client.join().expect("client");
}

#[test]
fn dropping_exchange_without_response_closes_without_fabricating_response() {
    let composition = composition("onoal.package.test.http.exchange.drop");
    let instance = started_instance(
        &composition,
        "onoal.package.test.http.exchange.drop.instance",
    );
    let inspector = activate::<TcpTransportInspector>(&instance);
    let server = activate::<HttpServer>(&instance);
    let address = actual_address(&inspector);
    let client =
        thread::spawn(move || client_exchange(address, b"GET /drop HTTP/1.1\r\n\r\n".to_vec()));

    let exchange = block_on(server.accept_exchange())
        .expect("accept")
        .expect("exchange");
    assert_eq!(exchange.request().target, "/drop");
    drop(exchange);

    let response = client.join().expect("client");
    assert!(
        response.is_empty(),
        "dropping an exchange should close, not fabricate a response: {response:?}"
    );
}

#[test]
fn composition_binds_http_server_to_named_transport() {
    let composition = Fabric::new("onoal.package.test.http.binding")
        .expect("fabric")
        .with(loopback_tcp_transport("api"))
        .with(loopback_tcp_transport("admin"))
        .with(http_server("admin"))
        .build()
        .expect("composition");

    assert_eq!(composition.resources().count(), 2);
    assert!(composition.relations().iter().any(|relation| {
        relation.role().as_str() == "transport"
            && matches!(
                relation.resolved_target(),
                SemanticRelationTargetOccurrence::Resource { resource_name, .. }
                    if resource_name.as_str() == "admin"
            )
    }));
}

#[test]
fn stopped_instance_rejects_accept_exchange() {
    let composition = composition("onoal.package.test.http.stopped");
    let mut instance = started_instance(&composition, "onoal.package.test.http.stopped.instance");
    activate::<HttpServer>(&instance);
    instance.stop().expect("stop");
    let server = instance.component::<HttpServer>().expect("server");
    assert!(block_on(server.accept_exchange()).is_err());
}
