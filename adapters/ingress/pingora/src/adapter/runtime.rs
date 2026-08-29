use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use pingora::apps::http_app::HttpServer;
use pingora::server::configuration::ServerConf;
use pingora::server::{RunArgs, Server};
use pingora::services::listening::Service as PingoraListeningService;

use crate::adapter::app::PingoraHttpApp;
use crate::adapter::shared::{PingoraRuntime, SharedPingoraState};
use crate::adapter::shutdown::ShutdownController;
use crate::config::PingoraIngressConfig;

pub(crate) fn spawn_runtime(
    shared: std::sync::Arc<SharedPingoraState>,
    config: &PingoraIngressConfig,
    previous_port: Option<u16>,
) -> Result<PingoraRuntime, String> {
    let listen_address = choose_listen_address(config.bind_address, previous_port)?;
    let (shutdown, watcher) = ShutdownController::new();
    let upgrade_sock = format!(
        "/tmp/fabric-adapter-ingress-pingora-{}-{}.sock",
        std::process::id(),
        listen_address.port()
    );

    let thread = thread::spawn({
        let shared = std::sync::Arc::clone(&shared);
        move || {
            let mut pingora_conf = ServerConf {
                daemon: false,
                threads: 1,
                listener_tasks_per_fd: 1,
                work_stealing: true,
                upgrade_sock,
                graceful_shutdown_timeout_seconds: Some(0),
                grace_period_seconds: Some(0),
                ..ServerConf::default()
            };
            pingora_conf.pid_file = "/tmp/fabric-adapter-ingress-pingora.pid".to_owned();

            let mut server = Server::new_with_opt_and_conf(None, pingora_conf);
            server.bootstrap();

            let app = PingoraHttpApp::new(shared);
            let mut http_service = PingoraListeningService::new(
                "stel pingora ingress".to_owned(),
                HttpServer::new_app(app),
            );
            http_service.add_tcp(&listen_address.to_string());
            server.add_service(http_service);
            server.run(RunArgs {
                #[cfg(unix)]
                shutdown_signal: Box::new(watcher),
            });
        }
    });

    Ok(PingoraRuntime {
        listen_address,
        shutdown,
        thread,
    })
}

pub(crate) fn wait_for_readiness(
    runtime: &PingoraRuntime,
    config: &PingoraIngressConfig,
) -> Result<(), String> {
    let deadline = Instant::now() + config.startup_timeout;
    let ready_request =
        "GET /__fabric_pingora_ready HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    while Instant::now() < deadline {
        if runtime.thread.is_finished() {
            return Err("pingora listener thread exited before readiness".to_owned());
        }
        match TcpStream::connect_timeout(&runtime.listen_address, config.probe_interval) {
            Ok(mut stream) => {
                stream
                    .set_read_timeout(Some(config.probe_interval))
                    .map_err(|error| format!("configure readiness read timeout: {error}"))?;
                stream
                    .set_write_timeout(Some(config.probe_interval))
                    .map_err(|error| format!("configure readiness write timeout: {error}"))?;
                stream
                    .write_all(ready_request.as_bytes())
                    .map_err(|error| format!("write readiness probe: {error}"))?;
                let mut response = [0_u8; 256];
                let size = stream
                    .read(&mut response)
                    .map_err(|error| format!("read readiness probe: {error}"))?;
                if std::str::from_utf8(&response[..size])
                    .ok()
                    .is_some_and(|body| {
                        body.starts_with("HTTP/1.1 204") || body.starts_with("HTTP/1.0 204")
                    })
                {
                    return Ok(());
                }
            }
            Err(_) => thread::sleep(config.probe_interval),
        }
        thread::sleep(config.probe_interval);
    }
    Err("pingora listener readiness probe timed out".to_owned())
}

pub(crate) fn shutdown_runtime(runtime: PingoraRuntime, timeout: Duration) -> Result<(), String> {
    runtime.shutdown.shutdown_fast();
    let start = Instant::now();
    while !runtime.thread.is_finished() && start.elapsed() < timeout {
        thread::sleep(Duration::from_millis(10));
    }
    runtime
        .thread
        .join()
        .map_err(|_| "pingora listener thread panicked".to_owned())
}

fn choose_listen_address(
    configured: SocketAddr,
    previous_port: Option<u16>,
) -> Result<SocketAddr, String> {
    if configured.port() != 0 {
        return Ok(configured);
    }
    for _ in 0..32 {
        let listener = TcpListener::bind(SocketAddr::new(configured.ip(), 0))
            .map_err(|error| format!("reserve pingora listener port: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("read reserved pingora port: {error}"))?;
        if previous_port != Some(address.port()) {
            return Ok(address);
        }
    }
    Err("failed to reserve a fresh pingora listener port".to_owned())
}
