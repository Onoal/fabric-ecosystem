//! HTTP/1 server behavior package for Fabric.
//!
//! HTTP owns protocol behavior over TCP byte streams. TCP remains the transport
//! capability and this package does not introduce routing, TLS, HTTP/2, logging,
//! database access, or a web framework.

use std::fmt;

use fabric::prelude::*;
use fabric_package_networking_tcp::{
    TcpAcceptResult, TcpByteStreamTransport, TcpConnection, TcpTransportError,
};

pub const DEFAULT_MAX_HEAD_BYTES: usize = 8 * 1024;
pub const DEFAULT_MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpVersion {
    Http10,
    Http11,
}

impl HttpVersion {
    fn from_httparse(version: u8) -> Result<Self, HttpError> {
        match version {
            0 => Ok(Self::Http10),
            1 => Ok(Self::Http11),
            _ => Err(HttpError::new(
                HttpErrorKind::UnsupportedVersion,
                format!("unsupported HTTP/1.{version}"),
            )),
        }
    }

    fn status_line_prefix(self) -> &'static str {
        match self {
            Self::Http10 => "HTTP/1.0",
            Self::Http11 => "HTTP/1.1",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

impl HttpHeader {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub target: String,
    pub version: HttpVersion,
    pub headers: Vec<HttpHeader>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub version: HttpVersion,
    pub status_code: u16,
    pub reason: Option<String>,
    pub headers: Vec<HttpHeader>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status_code: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            version: HttpVersion::Http11,
            status_code,
            reason: None,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push(HttpHeader::new(name, value));
        self
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpErrorKind {
    Transport,
    MalformedRequest,
    RequestTooLarge,
    UnsupportedVersion,
    IncompleteRequest,
    WriteResponseFailed,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpError {
    pub kind: HttpErrorKind,
    pub detail: String,
}

impl HttpError {
    pub fn new(kind: HttpErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    fn transport(error: TcpTransportError) -> Self {
        Self::new(HttpErrorKind::Transport, error.detail)
    }
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for HttpError {}

fabric::component! {
    pub HttpServer {
        id: "onoal.package.networking.http.server";

        relations {
            requires {
                transport: TcpByteStreamTransport;
            }
        }

        api {
            fn serve_once(&self, response: HttpResponse) -> Result<HttpRequest, HttpError>;
        }

        runtime {
            fn serve_once(&self, response: HttpResponse) -> Result<HttpRequest, HttpError> {
                let connection = match self.relations().transport.accept() {
                    TcpAcceptResult::Accepted(connection) => connection,
                    TcpAcceptResult::Stopped => {
                        return Err(HttpError::new(
                            HttpErrorKind::Stopped,
                            "tcp transport stopped before accepting an HTTP request",
                        ))
                    }
                    TcpAcceptResult::Failed(error) => return Err(HttpError::transport(error)),
                };
                let request = read_http_request(
                    &connection,
                    DEFAULT_MAX_HEAD_BYTES,
                    DEFAULT_MAX_BODY_BYTES,
                )?;
                write_http_response(&connection, &response)?;
                let _ = connection.shutdown_both();
                Ok(request)
            }
        }
    }
}

pub fn http_server(transport_name: &'static str) -> impl IntoFabricContribution {
    let transport =
        TcpByteStreamTransport::select(transport_name).expect("valid TCP transport name");
    let component = HttpServer::define().select_named_resource_provider(
        &fabric::authoring::ComponentResourceRequirement::new(
            fabric::component::ComponentRelationName::new("transport").expect("role"),
            fabric::authoring::Requires::<TcpByteStreamTransport>::provisional(),
        ),
        &transport,
    );
    FabricContribution::new().component(component)
}

fn read_http_request(
    connection: &TcpConnection,
    max_head_bytes: usize,
    max_body_bytes: usize,
) -> Result<HttpRequest, HttpError> {
    let mut buffer = Vec::new();
    let header_len = loop {
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut request = httparse::Request::new(&mut headers);
        match request.parse(&buffer) {
            Ok(httparse::Status::Complete(header_len)) => break header_len,
            Ok(httparse::Status::Partial) => {
                if buffer.len() >= max_head_bytes {
                    return Err(HttpError::new(
                        HttpErrorKind::RequestTooLarge,
                        format!("HTTP request head exceeded {max_head_bytes} bytes"),
                    ));
                }
                let remaining = max_head_bytes - buffer.len();
                let bytes = connection
                    .read_some(remaining.min(1024))
                    .map_err(HttpError::transport)?;
                if bytes.is_empty() {
                    return Err(HttpError::new(
                        HttpErrorKind::IncompleteRequest,
                        "connection closed before complete HTTP request head",
                    ));
                }
                buffer.extend(bytes);
            }
            Err(error) => {
                return Err(HttpError::new(
                    HttpErrorKind::MalformedRequest,
                    format!("malformed HTTP request: {error:?}"),
                ))
            }
        }
    };

    let (method, target, version, headers, content_length) = {
        let mut parsed_headers = [httparse::EMPTY_HEADER; 64];
        let mut request = httparse::Request::new(&mut parsed_headers);
        request
            .parse(&buffer)
            .map_err(|error| {
                HttpError::new(
                    HttpErrorKind::MalformedRequest,
                    format!("malformed HTTP request: {error:?}"),
                )
            })?
            .unwrap();
        let method = request
            .method
            .ok_or_else(|| HttpError::new(HttpErrorKind::MalformedRequest, "missing method"))?
            .to_owned();
        let target = request
            .path
            .ok_or_else(|| HttpError::new(HttpErrorKind::MalformedRequest, "missing target"))?
            .to_owned();
        let version = HttpVersion::from_httparse(request.version.ok_or_else(|| {
            HttpError::new(HttpErrorKind::MalformedRequest, "missing HTTP version")
        })?)?;
        let mut headers = Vec::new();
        let mut content_length = 0usize;
        for header in request.headers.iter() {
            if header.name.is_empty() {
                continue;
            }
            let value = std::str::from_utf8(header.value).map_err(|_| {
                HttpError::new(
                    HttpErrorKind::MalformedRequest,
                    format!("header {} is not valid UTF-8", header.name),
                )
            })?;
            if header.name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().map_err(|_| {
                    HttpError::new(
                        HttpErrorKind::MalformedRequest,
                        "Content-Length is not a valid non-negative length",
                    )
                })?;
            }
            if header.name.eq_ignore_ascii_case("transfer-encoding")
                && value
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case("chunked"))
            {
                return Err(HttpError::new(
                    HttpErrorKind::MalformedRequest,
                    "chunked transfer decoding is not supported by this HTTP package",
                ));
            }
            headers.push(HttpHeader::new(header.name, value));
        }
        (method, target, version, headers, content_length)
    };

    if content_length > max_body_bytes {
        return Err(HttpError::new(
            HttpErrorKind::RequestTooLarge,
            format!("HTTP request body exceeded {max_body_bytes} bytes"),
        ));
    }

    let needed = header_len + content_length;
    while buffer.len() < needed {
        let remaining = needed - buffer.len();
        let bytes = connection
            .read_some(remaining.min(8192))
            .map_err(HttpError::transport)?;
        if bytes.is_empty() {
            return Err(HttpError::new(
                HttpErrorKind::IncompleteRequest,
                "connection closed before complete HTTP request body",
            ));
        }
        buffer.extend(bytes);
    }

    Ok(HttpRequest {
        method,
        target,
        version,
        headers,
        body: buffer[header_len..needed].to_vec(),
    })
}

fn write_http_response(
    connection: &TcpConnection,
    response: &HttpResponse,
) -> Result<(), HttpError> {
    let reason = response
        .reason
        .as_deref()
        .unwrap_or_else(|| default_reason_phrase(response.status_code));
    let mut bytes = format!(
        "{} {} {}\r\n",
        response.version.status_line_prefix(),
        response.status_code,
        reason
    )
    .into_bytes();
    let mut has_connection = false;
    for header in &response.headers {
        if header.name.eq_ignore_ascii_case("content-length") {
            continue;
        }
        if header.name.eq_ignore_ascii_case("connection") {
            has_connection = true;
        }
        bytes.extend_from_slice(header.name.as_bytes());
        bytes.extend_from_slice(b": ");
        bytes.extend_from_slice(header.value.as_bytes());
        bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(format!("Content-Length: {}\r\n", response.body.len()).as_bytes());
    if !has_connection {
        bytes.extend_from_slice(b"Connection: close\r\n");
    }
    bytes.extend_from_slice(b"\r\n");
    bytes.extend_from_slice(&response.body);
    connection.write_all(&bytes).map_err(|error| {
        HttpError::new(
            HttpErrorKind::WriteResponseFailed,
            format!("failed to write HTTP response: {}", error.detail),
        )
    })
}

fn default_reason_phrase(status_code: u16) -> &'static str {
    match status_code {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        _ => "Status",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fabric_package_networking_tcp::{
        loopback_tcp_transport, tcp_transport_probe, TcpProbeObservation, TcpTransportProbe,
        TcpTransportProbeInstanceApi,
    };
    use futures::executor::block_on;
    use std::io::{Read, Write};
    use std::net::{Shutdown, SocketAddr, TcpStream};
    use std::thread;

    fn composition(id: &str) -> Composition {
        Fabric::new(id)
            .expect("fabric")
            .with(loopback_tcp_transport("api"))
            .with(tcp_transport_probe("api"))
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

    fn actual_address(probe: &BoundComponent<'_, TcpTransportProbe>) -> TcpSocketAddress {
        let observation: TcpProbeObservation =
            block_on(probe.observe_transport()).expect("observe");
        observation.actual.expect("actual address")
    }

    use fabric_package_networking_tcp::TcpSocketAddress;

    fn client_exchange(address: TcpSocketAddress, request: Vec<u8>) -> Vec<u8> {
        let socket: SocketAddr = format!("{}:{}", address.host, address.port)
            .parse()
            .expect("socket addr");
        let mut stream = TcpStream::connect(socket).expect("client connect");
        stream.write_all(&request).expect("write request");
        stream.shutdown(Shutdown::Write).expect("shutdown write");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read response");
        response
    }

    #[test]
    fn get_request_and_response_work_over_real_tcp() {
        let composition = composition("onoal.package.test.http.get");
        let instance = started_instance(&composition, "onoal.package.test.http.get.instance");
        let probe = activate::<TcpTransportProbe>(&instance);
        let server = activate::<HttpServer>(&instance);
        let address = actual_address(&probe);
        let client = thread::spawn(move || {
            client_exchange(
                address,
                b"GET /hello HTTP/1.1\r\nHost: example.test\r\nAccept: text/plain\r\n\r\n".to_vec(),
            )
        });

        let request = block_on(
            server.serve_once(
                HttpResponse::new(200, b"hello-http".to_vec())
                    .with_header("Content-Type", "text/plain")
                    .with_header("X-Package", "fabric-http"),
            ),
        )
        .expect("serve")
        .expect("http request");
        let response = String::from_utf8(client.join().expect("client")).expect("utf8 response");

        assert_eq!(request.method, "GET");
        assert_eq!(request.target, "/hello");
        assert_eq!(request.version, HttpVersion::Http11);
        assert!(request.body.is_empty());
        assert!(request.headers.iter().any(
            |header| header.name.eq_ignore_ascii_case("host") && header.value == "example.test"
        ));
        assert!(request.headers.iter().any(
            |header| header.name.eq_ignore_ascii_case("accept") && header.value == "text/plain"
        ));
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("Content-Type: text/plain\r\n"));
        assert!(response.contains("X-Package: fabric-http\r\n"));
        assert!(response.contains("Content-Length: 10\r\n"));
        assert!(response.ends_with("\r\n\r\nhello-http"));
    }

    #[test]
    fn post_body_is_read_without_client_eof_before_response() {
        let composition = composition("onoal.package.test.http.post");
        let instance = started_instance(&composition, "onoal.package.test.http.post.instance");
        let probe = activate::<TcpTransportProbe>(&instance);
        let server = activate::<HttpServer>(&instance);
        let address = actual_address(&probe);
        let (response_read_tx, response_read_rx) = std::sync::mpsc::channel();
        let client = thread::spawn(move || {
            let socket: SocketAddr = format!("{}:{}", address.host, address.port)
                .parse()
                .expect("socket addr");
            let mut stream = TcpStream::connect(socket).expect("client connect");
            stream
                .write_all(
                    b"POST /submit HTTP/1.1\r\nHost: example.test\r\nContent-Length: 11\r\nX-Mode: test\r\n\r\nhello=world",
                )
                .expect("write request");
            let mut response = [0; 128];
            let count = stream.read(&mut response).expect("read response");
            response_read_tx
                .send(response[..count].to_vec())
                .expect("send response");
            stream.shutdown(Shutdown::Both).expect("shutdown");
        });

        let request = block_on(server.serve_once(HttpResponse::new(201, b"created".to_vec())))
            .expect("serve")
            .expect("request");
        let response = String::from_utf8(response_read_rx.recv().expect("response")).expect("utf8");
        client.join().expect("client");

        assert_eq!(request.method, "POST");
        assert_eq!(request.target, "/submit");
        assert_eq!(request.body, b"hello=world");
        assert!(request
            .headers
            .iter()
            .any(|header| header.name.eq_ignore_ascii_case("x-mode") && header.value == "test"));
        assert!(response.starts_with("HTTP/1.1 201 Created\r\n"));
        assert!(response.contains("Content-Length: 7\r\n"));
    }

    #[test]
    fn malformed_request_returns_bounded_error() {
        let composition = composition("onoal.package.test.http.malformed");
        let instance = started_instance(&composition, "onoal.package.test.http.malformed.instance");
        let probe = activate::<TcpTransportProbe>(&instance);
        let server = activate::<HttpServer>(&instance);
        let address = actual_address(&probe);
        let client = thread::spawn(move || client_exchange(address, b"not http\r\n\r\n".to_vec()));

        let result = block_on(server.serve_once(HttpResponse::new(200, b"unused".to_vec())))
            .expect("serve result");
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
    fn request_size_bound_is_enforced() {
        let composition = composition("onoal.package.test.http.bounds");
        let instance = started_instance(&composition, "onoal.package.test.http.bounds.instance");
        let probe = activate::<TcpTransportProbe>(&instance);
        let server = activate::<HttpServer>(&instance);
        let address = actual_address(&probe);
        let oversized = format!(
            "POST /too-large HTTP/1.1\r\nHost: example.test\r\nContent-Length: {}\r\n\r\n",
            DEFAULT_MAX_BODY_BYTES + 1
        );
        let client = thread::spawn(move || client_exchange(address, oversized.into_bytes()));

        let result = block_on(server.serve_once(HttpResponse::new(200, b"unused".to_vec())))
            .expect("serve result");
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
    fn composition_binds_http_server_to_named_transport() {
        let composition = Fabric::new("onoal.package.test.http.binding")
            .expect("fabric")
            .with(loopback_tcp_transport("api"))
            .with(loopback_tcp_transport("admin"))
            .with(http_server("admin"))
            .build()
            .expect("composition");

        assert_eq!(composition.resources().count(), 2);
        assert!(composition
            .relations()
            .iter()
            .any(|relation| relation.role().as_str() == "transport"
                && matches!(
                    relation.resolved_target(),
                    SemanticRelationTargetOccurrence::Resource { resource_name, .. }
                        if resource_name.as_str() == "admin"
                )));
    }

    #[test]
    fn stopped_instance_rejects_stale_http_invocation() {
        let composition = composition("onoal.package.test.http.lifecycle");
        let mut instance =
            started_instance(&composition, "onoal.package.test.http.lifecycle.instance");
        activate::<HttpServer>(&instance);
        instance.stop().expect("stop");
        let server = instance.component::<HttpServer>().expect("server");
        assert!(block_on(server.serve_once(HttpResponse::new(200, b"stopped".to_vec()))).is_err());
    }
}
