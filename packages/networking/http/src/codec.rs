use fabric_package_networking_tcp::TcpConnection;

use crate::{HttpError, HttpErrorKind, HttpHeader, HttpRequest, HttpResponse, HttpVersion};

pub const DEFAULT_MAX_HEAD_BYTES: usize = 8 * 1024;
pub const DEFAULT_MAX_BODY_BYTES: usize = 1024 * 1024;

pub(crate) fn read_http_request(
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
                ));
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
        let mut content_length = None;
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
                let parsed = value.trim().parse::<usize>().map_err(|_| {
                    HttpError::new(
                        HttpErrorKind::MalformedRequest,
                        "Content-Length is not a valid non-negative length",
                    )
                })?;
                if let Some(existing) = content_length {
                    if existing != parsed {
                        return Err(HttpError::new(
                            HttpErrorKind::MalformedRequest,
                            "conflicting Content-Length headers are not allowed",
                        ));
                    }
                }
                content_length = Some(parsed);
            }
            if header.name.eq_ignore_ascii_case("transfer-encoding")
                && value.split(',').any(|part| {
                    let part = part.trim();
                    !part.is_empty() && !part.eq_ignore_ascii_case("identity")
                })
            {
                return Err(HttpError::new(
                    HttpErrorKind::UnsupportedTransferEncoding,
                    "transfer encodings other than identity are not supported",
                ));
            }
            headers.push(HttpHeader::new(header.name, value));
        }
        (
            method,
            target,
            version,
            headers,
            content_length.unwrap_or(0),
        )
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

pub(crate) fn write_http_response(
    connection: &TcpConnection,
    response: &HttpResponse,
) -> Result<(), HttpError> {
    let bytes = encode_http_response(response)?;
    connection.write_all(&bytes).map_err(|error| {
        HttpError::new(
            HttpErrorKind::WriteResponseFailed,
            format!("failed to write HTTP response: {}", error.detail),
        )
    })
}

fn encode_http_response(response: &HttpResponse) -> Result<Vec<u8>, HttpError> {
    validate_status_code(response.status_code)?;
    let reason = response
        .reason
        .as_deref()
        .unwrap_or_else(|| default_reason_phrase(response.status_code));
    validate_header_value("reason phrase", reason)?;
    let mut bytes = format!(
        "{} {} {}\r\n",
        response.version.status_line_prefix(),
        response.status_code,
        reason
    )
    .into_bytes();
    let mut has_connection = false;
    for header in &response.headers {
        validate_header_name(&header.name)?;
        validate_header_value(&header.name, &header.value)?;
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
    Ok(bytes)
}

fn validate_status_code(status_code: u16) -> Result<(), HttpError> {
    if (100..=999).contains(&status_code) {
        Ok(())
    } else {
        Err(HttpError::new(
            HttpErrorKind::InvalidResponse,
            format!("HTTP status code {status_code} is outside the valid three-digit range"),
        ))
    }
}

fn validate_header_name(name: &str) -> Result<(), HttpError> {
    if name.is_empty()
        || name
            .bytes()
            .any(|byte| byte <= 0x20 || byte == b':' || byte >= 0x7f)
    {
        return Err(HttpError::new(
            HttpErrorKind::InvalidResponse,
            format!("invalid HTTP response header name: {name:?}"),
        ));
    }
    Ok(())
}

fn validate_header_value(label: &str, value: &str) -> Result<(), HttpError> {
    if value.bytes().any(|byte| byte == b'\r' || byte == b'\n') {
        return Err(HttpError::new(
            HttpErrorKind::InvalidResponse,
            format!("HTTP response {label} contains CR/LF injection"),
        ));
    }
    Ok(())
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

    #[test]
    fn response_headers_reject_crlf_injection() {
        let response =
            HttpResponse::new(200, b"ok".to_vec()).with_header("X-Test", "safe\r\nInjected: yes");
        assert!(matches!(
            encode_http_response(&response),
            Err(HttpError {
                kind: HttpErrorKind::InvalidResponse,
                ..
            })
        ));
    }

    #[test]
    fn response_content_length_is_owned_by_encoder() {
        let response = HttpResponse::new(200, b"abc".to_vec()).with_header("Content-Length", "999");
        let encoded =
            String::from_utf8(encode_http_response(&response).expect("encoded")).expect("utf8");
        assert!(encoded.contains("Content-Length: 3\r\n"));
        assert!(!encoded.contains("Content-Length: 999\r\n"));
    }
}
