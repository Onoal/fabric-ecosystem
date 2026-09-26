use fabric_package_networking_tcp::TcpConnection;

use crate::codec::write_http_response;
use crate::{HttpError, HttpRequest, HttpResponse};

#[derive(Debug)]
pub struct HttpExchange {
    request: HttpRequest,
    connection: TcpConnection,
}

impl HttpExchange {
    pub(crate) fn new(request: HttpRequest, connection: TcpConnection) -> Self {
        Self {
            request,
            connection,
        }
    }

    pub fn request(&self) -> &HttpRequest {
        &self.request
    }

    pub fn respond(self, response: HttpResponse) -> Result<(), HttpError> {
        write_http_response(&self.connection, &response)?;
        let _ = self.connection.shutdown_both();
        Ok(())
    }
}

impl Drop for HttpExchange {
    fn drop(&mut self) {
        let _ = self.connection.shutdown_both();
    }
}
