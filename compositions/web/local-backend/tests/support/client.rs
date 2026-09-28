use fabric_package_networking_tcp::TcpSocketAddress;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::thread;

pub fn http_client(address: TcpSocketAddress, target: &'static str) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let socket: SocketAddr = format!("{}:{}", address.host, address.port)
            .parse()
            .expect("socket addr");
        let mut stream = TcpStream::connect(socket).expect("client connect");
        stream
            .write_all(format!("GET {target} HTTP/1.1\r\nHost: backend.test\r\n\r\n").as_bytes())
            .expect("write request");
        stream.shutdown(Shutdown::Write).expect("shutdown write");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read response");
        String::from_utf8(response).expect("utf8 response")
    })
}
