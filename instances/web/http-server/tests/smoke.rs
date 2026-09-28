use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

struct ChildGuard {
    child: Child,
}

impl ChildGuard {
    fn spawn() -> std::io::Result<Self> {
        let child = Command::new(env!("CARGO_BIN_EXE_onoal-fabric-instance-http-server"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        Ok(Self { child })
    }

    fn terminate(mut self) -> std::io::Result<()> {
        self.kill_and_wait()
    }

    fn kill_and_wait(&mut self) -> std::io::Result<()> {
        if self.child.try_wait()?.is_none() {
            self.child.kill()?;
        }
        let _ = self.child.wait()?;
        Ok(())
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.kill_and_wait();
    }
}

#[test]
fn binary_serves_multiple_external_requests_until_terminated() {
    let mut server = ChildGuard::spawn().expect("spawn server");
    let stdout = server.child.stdout.take().expect("stdout");
    let (lines, reader) = mpsc::channel();
    let _reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if lines.send(line).is_err() {
                break;
            }
        }
    });

    let listening = reader
        .recv_timeout(Duration::from_secs(10))
        .expect("server listening output")
        .expect("read listening line");
    assert_eq!(listening, "HTTP server listening on http://127.0.0.1:8080");

    let first = get("/smoke").expect("first request");
    assert!(first.starts_with("HTTP/1.1 200"));
    assert!(first.contains("hello from /smoke"));

    let second = get("/second").expect("second request");
    assert!(second.starts_with("HTTP/1.1 200"));
    assert!(second.contains("hello from /second"));

    server.terminate().expect("terminate server");
}

fn get(target: &str) -> std::io::Result<String> {
    let mut stream = TcpStream::connect("127.0.0.1:8080")?;
    let request = format!("GET {target} HTTP/1.1\r\nHost: instance.local\r\n\r\n");
    stream.write_all(request.as_bytes())?;
    stream.shutdown(Shutdown::Write)?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}
