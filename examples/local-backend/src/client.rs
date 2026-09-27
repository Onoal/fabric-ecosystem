use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::thread;

use fabric_package_networking_tcp::TcpSocketAddress;

pub(crate) fn get(
    address: TcpSocketAddress,
    target: &'static str,
) -> thread::JoinHandle<std::io::Result<String>> {
    thread::spawn(move || {
        let socket: SocketAddr = format!("{}:{}", address.host, address.port)
            .parse()
            .map_err(|error| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid socket address: {error}"),
                )
            })?;
        let mut stream = TcpStream::connect(socket)?;
        let request = format!("GET {target} HTTP/1.1\r\nHost: example.local\r\n\r\n");
        stream.write_all(request.as_bytes())?;
        stream.shutdown(Shutdown::Write)?;

        let mut response = Vec::new();
        stream.read_to_end(&mut response)?;
        String::from_utf8(response).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("response was not UTF-8: {error}"),
            )
        })
    })
}
