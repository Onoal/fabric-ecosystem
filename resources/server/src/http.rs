use crate::ServerError;
use crate::model::ServerInstanceId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatchServerHttpRequest {
    pub server_instance_id: ServerInstanceId,
    pub request: ServerHttpRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerHttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<ServerHttpHeader>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerHttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerHttpResponse {
    pub status: u16,
    pub headers: Vec<ServerHttpHeader>,
    pub body: Vec<u8>,
}

impl ServerHttpRequest {
    pub fn validate(&self) -> Result<(), ServerError> {
        validate_http_method(&self.method)?;
        if self.url.is_empty() {
            return Err(ServerError::invalid_input(
                "server http request url must not be empty",
            ));
        }
        for header in &self.headers {
            header.validate()?;
        }
        Ok(())
    }
}

impl ServerHttpHeader {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Result<Self, ServerError> {
        let header = Self {
            name: name.into(),
            value: value.into(),
        };
        header.validate()?;
        Ok(header)
    }

    pub fn validate(&self) -> Result<(), ServerError> {
        let valid_name = !self.name.is_empty()
            && self.name.len() <= 256
            && self
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if !valid_name {
            return Err(ServerError::invalid_input(
                "invalid server http header name",
            ));
        }
        if self
            .value
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            return Err(ServerError::invalid_input(
                "invalid server http header value",
            ));
        }
        Ok(())
    }
}

fn validate_http_method(method: &str) -> Result<(), ServerError> {
    let valid = !method.is_empty()
        && method.len() <= 32
        && method
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit());
    if valid {
        Ok(())
    } else {
        Err(ServerError::invalid_input("invalid http method"))
    }
}
