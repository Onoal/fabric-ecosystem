//! HTTP/1 server behavior package for Fabric.
//!
//! HTTP owns request/response message semantics over TCP byte streams. TCP
//! remains the transport capability; applications decide responses after they
//! inspect requests through `HttpExchange`.
//!
//! This package does not define routing, middleware, TLS, HTTP/2, logging,
//! database access, callbacks, or a web framework.

mod authoring;
mod codec;
mod error;
mod exchange;
mod model;
mod server;

pub use authoring::http_server;
pub use codec::{DEFAULT_MAX_BODY_BYTES, DEFAULT_MAX_HEAD_BYTES};
pub use error::{HttpError, HttpErrorKind};
pub use exchange::HttpExchange;
pub use model::{HttpHeader, HttpRequest, HttpResponse, HttpVersion};
pub use server::{HttpServer, HttpServerInstanceApi};
