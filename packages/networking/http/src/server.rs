use fabric_package_networking_tcp::{TcpAcceptResult, TcpByteStreamTransport};

use crate::codec::{read_http_request, DEFAULT_MAX_BODY_BYTES, DEFAULT_MAX_HEAD_BYTES};
use crate::{HttpError, HttpErrorKind, HttpExchange};

fabric::component! {
    pub HttpServer {
        id: "onoal.package.networking.http.server";

        relations {
            requires {
                transport: TcpByteStreamTransport;
            }
        }

        api {
            fn accept_exchange(&self) -> Result<HttpExchange, HttpError>;
        }

        runtime {
            fn accept_exchange(&self) -> Result<HttpExchange, HttpError> {
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
                Ok(HttpExchange::new(request, connection))
            }
        }
    }
}
