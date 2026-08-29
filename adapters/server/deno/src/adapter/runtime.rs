use std::collections::BTreeMap;
use std::fs;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use fabric_resource_server::{
    DispatchServerHttpRequest, ServerError, ServerExecution, ServerExecutionRequest,
    ServerHttpRequest, ServerHttpResponse, ServerSpec,
};
#[cfg(unix)]
use nix::sys::signal::{self, Signal};
#[cfg(unix)]
use nix::unistd::Pid;
use sha2::{Digest, Sha256};
use ureq::RequestExt;

use crate::artifact::{DenoServerArtifactResolver, ResolvedDenoServerArtifact};
use crate::config::DenoServerConfig;

const EXPECTED_DENO_VERSION: &str = "2.9.5";
const DENO_SERVER_HOST_ENV: &str = "FABRIC_DENO_SERVER_HOST";
const DENO_SERVER_PORT_ENV: &str = "FABRIC_DENO_SERVER_PORT";

pub(crate) struct DenoServerRuntime {
    config: DenoServerConfig,
    resolver: Arc<dyn DenoServerArtifactResolver>,
    prepared: BTreeMap<fabric_resource_server::ServerId, PreparedArtifact>,
    binary_verified: bool,
}

#[derive(Clone)]
struct PreparedArtifact {
    resolved: ResolvedDenoServerArtifact,
    entrypoint: PathBuf,
}

pub(crate) struct DenoServerExecution {
    endpoint: SocketAddr,
    runtime_dir: PathBuf,
    child: Child,
}

impl DenoServerRuntime {
    pub(crate) fn new(
        config: DenoServerConfig,
        resolver: Arc<dyn DenoServerArtifactResolver>,
    ) -> Result<Self, ServerError> {
        Ok(Self {
            config,
            resolver,
            prepared: BTreeMap::new(),
            binary_verified: false,
        })
    }

    pub(crate) fn prepare_server(&mut self, server: &ServerSpec) -> Result<(), ServerError> {
        let (resolved, entrypoint) =
            self.resolve_and_verify(&server.artifact, &server.entrypoint)?;
        self.prepared.insert(
            server.server_id.clone(),
            PreparedArtifact {
                resolved,
                entrypoint,
            },
        );
        Ok(())
    }

    pub(crate) fn start_execution(
        &mut self,
        request: ServerExecutionRequest,
    ) -> Result<Box<dyn ServerExecution>, ServerError> {
        self.verify_deno_binary()?;
        let prepared = self
            .prepared
            .get(&request.start.server.server_id)
            .ok_or_else(|| ServerError::ProtocolViolation {
                message: "server was not prepared by this deno runtime".to_owned(),
            })?;

        let runtime_dir = self.config.runtime_root.join(request.instance_id.as_str());
        reset_runtime_dir(&runtime_dir)?;
        let staged_entry = stage_artifact(&prepared.resolved, &prepared.entrypoint, &runtime_dir)?;
        let endpoint = bind_loopback_socket()?;
        let stdout_log = fs::File::create(runtime_dir.join("stdout.log")).map_err(|error| {
            ServerError::StartFailed {
                message: format!("create deno server stdout log: {error}"),
            }
        })?;
        let stderr_log = fs::File::create(runtime_dir.join("stderr.log")).map_err(|error| {
            ServerError::StartFailed {
                message: format!("create deno server stderr log: {error}"),
            }
        })?;
        let mut command = Command::new(&self.config.deno_bin);
        command
            .arg("run")
            .arg("--no-prompt")
            .arg("--quiet")
            .arg(format!("--allow-read={}", runtime_dir.display()))
            .arg(format!("--allow-net=127.0.0.1:{}", endpoint.port()))
            .arg(format!(
                "--allow-env={},{}",
                DENO_SERVER_HOST_ENV, DENO_SERVER_PORT_ENV
            ))
            .env(DENO_SERVER_HOST_ENV, "127.0.0.1")
            .env(DENO_SERVER_PORT_ENV, endpoint.port().to_string())
            .arg(&staged_entry)
            .stdout(Stdio::from(stdout_log))
            .stderr(Stdio::from(stderr_log));
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn().map_err(|error| ServerError::StartFailed {
            message: format!("spawn deno server: {error}"),
        })?;
        if let Err(error) = wait_for_readiness(&mut child, endpoint, self.config.startup_timeout) {
            let _ = terminate_child_process_tree(&mut child);
            let _ = child.wait();
            let _ = fs::remove_dir_all(&runtime_dir);
            return Err(error);
        }
        Ok(Box::new(DenoServerExecution {
            endpoint,
            runtime_dir,
            child,
        }))
    }

    pub(crate) fn clear(&mut self) {
        self.prepared.clear();
    }

    fn verify_deno_binary(&mut self) -> Result<(), ServerError> {
        if self.binary_verified {
            return Ok(());
        }
        let output = Command::new(&self.config.deno_bin)
            .arg("--version")
            .output()
            .map_err(|error| ServerError::StartFailed {
                message: format!("run deno --version: {error}"),
            })?;
        if !output.status.success() {
            return Err(ServerError::StartFailed {
                message: "deno --version failed".to_owned(),
            });
        }
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if !combined.contains(EXPECTED_DENO_VERSION) {
            return Err(ServerError::StartFailed {
                message: format!(
                    "deno version mismatch: expected {EXPECTED_DENO_VERSION}, got {}",
                    combined.trim()
                ),
            });
        }
        self.binary_verified = true;
        Ok(())
    }

    fn resolve_and_verify(
        &self,
        artifact: &fabric_resource_server::ServerArtifact,
        entrypoint: &fabric_resource_server::ServerEntrypoint,
    ) -> Result<(ResolvedDenoServerArtifact, PathBuf), ServerError> {
        let resolved = self.resolver.resolve(artifact)?;
        let artifact_file = canonical_file(&resolved.artifact_file, "artifact")?;
        let module_root = canonical_directory(&resolved.module_root, "module root")?;
        let entry_module = canonical_file(&module_root.join(entrypoint.as_str()), "entry module")?;
        if !entry_module.starts_with(&module_root) {
            return Err(ServerError::PrepareFailed {
                message: "entry module is outside the trusted module root".to_owned(),
            });
        }
        let actual = Sha256::digest(fs::read(&artifact_file).map_err(|error| {
            ServerError::PrepareFailed {
                message: format!("read materialized artifact: {error}"),
            }
        })?);
        if actual.as_slice() != artifact.sha256 {
            return Err(ServerError::PrepareFailed {
                message: "materialized artifact digest does not match server artifact".to_owned(),
            });
        }
        let relative_entrypoint = entry_module
            .strip_prefix(&module_root)
            .expect("validated entry module should remain within module root")
            .to_path_buf();
        Ok((
            ResolvedDenoServerArtifact {
                artifact_file,
                module_root,
            },
            relative_entrypoint,
        ))
    }
}

impl ServerExecution for DenoServerExecution {
    fn dispatch_http(
        &mut self,
        request: DispatchServerHttpRequest,
    ) -> Result<ServerHttpResponse, ServerError> {
        request.request.validate()?;
        if let Some(status) =
            self.child
                .try_wait()
                .map_err(|error| ServerError::DispatchFailed {
                    message: format!("inspect deno server process: {error}"),
                })?
        {
            return Err(ServerError::DispatchFailed {
                message: format!("deno server exited before dispatch with status {status}"),
            });
        }
        dispatch_to_deno(self.endpoint, &request.request)
    }

    fn stop(&mut self) -> Result<(), ServerError> {
        terminate_child_process_tree(&mut self.child)?;
        self.child.wait().map_err(|error| ServerError::StopFailed {
            message: format!("wait for deno server shutdown: {error}"),
        })?;
        Ok(())
    }

    fn cleanup(&mut self) {
        let _ = terminate_child_process_tree(&mut self.child);
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.runtime_dir);
    }
}

fn reset_runtime_dir(runtime_dir: &Path) -> Result<(), ServerError> {
    if runtime_dir.exists() {
        fs::remove_dir_all(runtime_dir).map_err(|error| ServerError::StartFailed {
            message: format!("reset deno server runtime dir: {error}"),
        })?;
    }
    fs::create_dir_all(runtime_dir).map_err(|error| ServerError::StartFailed {
        message: format!("create deno server runtime dir: {error}"),
    })?;
    Ok(())
}

fn stage_artifact(
    resolved: &ResolvedDenoServerArtifact,
    entrypoint: &Path,
    runtime_dir: &Path,
) -> Result<PathBuf, ServerError> {
    let staged_root = runtime_dir.join("artifact");
    copy_dir_recursive(&resolved.module_root, &staged_root)?;
    Ok(staged_root.join(entrypoint))
}

fn bind_loopback_socket() -> Result<SocketAddr, ServerError> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| ServerError::StartFailed {
        message: format!("bind server loopback listener: {error}"),
    })?;
    let address = listener
        .local_addr()
        .map_err(|error| ServerError::StartFailed {
            message: format!("inspect server loopback listener: {error}"),
        })?;
    drop(listener);
    Ok(address)
}

fn wait_for_readiness(
    child: &mut Child,
    endpoint: SocketAddr,
    timeout: Duration,
) -> Result<(), ServerError> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|error| ServerError::StartFailed {
            message: format!("inspect deno server child: {error}"),
        })? {
            return Err(ServerError::StartFailed {
                message: format!("deno server exited before readiness with status {status}"),
            });
        }
        if TcpStream::connect_timeout(&endpoint, Duration::from_millis(100)).is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(ServerError::StartFailed {
                message: format!("timed out waiting for deno server readiness at {endpoint}"),
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn dispatch_to_deno(
    endpoint: SocketAddr,
    request: &ServerHttpRequest,
) -> Result<ServerHttpResponse, ServerError> {
    let url = rewrite_url_to_endpoint(&request.url, endpoint)?;
    let mut builder = ureq::http::Request::builder()
        .method(request.method.as_str())
        .uri(&url);
    for header in &request.headers {
        builder = builder.header(&header.name, &header.value);
    }
    let request =
        builder
            .body(request.body.clone())
            .map_err(|error| ServerError::DispatchFailed {
                message: format!("build server request: {error}"),
            })?;
    let response = request
        .with_default_agent()
        .configure()
        .http_status_as_error(false)
        .run()
        .map_err(|error| ServerError::DispatchFailed {
            message: format!("dispatch to deno server: {error}"),
        })?;
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            fabric_resource_server::ServerHttpHeader::new(
                name.to_string(),
                value.to_str().unwrap_or_default().to_owned(),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| ServerError::DispatchFailed {
            message: error.to_string(),
        })?;
    let body = response
        .into_body()
        .read_to_vec()
        .map_err(|error| ServerError::DispatchFailed {
            message: format!("read deno server response body: {error}"),
        })?;
    Ok(ServerHttpResponse {
        status,
        headers,
        body,
    })
}

fn rewrite_url_to_endpoint(url: &str, endpoint: SocketAddr) -> Result<String, ServerError> {
    let mut parsed = url::Url::parse(url).map_err(|error| ServerError::DispatchFailed {
        message: format!("parse request url: {error}"),
    })?;
    parsed
        .set_host(Some("127.0.0.1"))
        .map_err(|_| ServerError::DispatchFailed {
            message: "set deno server host".to_owned(),
        })?;
    parsed
        .set_port(Some(endpoint.port()))
        .map_err(|_| ServerError::DispatchFailed {
            message: "set deno server port".to_owned(),
        })?;
    Ok(parsed.into())
}

fn canonical_file(path: &Path, label: &str) -> Result<PathBuf, ServerError> {
    let canonical = fs::canonicalize(path).map_err(|error| ServerError::PrepareFailed {
        message: format!("canonicalize {label}: {error}"),
    })?;
    if canonical.is_file() {
        Ok(canonical)
    } else {
        Err(ServerError::PrepareFailed {
            message: format!("{label} is not a file"),
        })
    }
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, ServerError> {
    let canonical = fs::canonicalize(path).map_err(|error| ServerError::PrepareFailed {
        message: format!("canonicalize {label}: {error}"),
    })?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(ServerError::PrepareFailed {
            message: format!("{label} is not a directory"),
        })
    }
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), ServerError> {
    fs::create_dir_all(target).map_err(|error| ServerError::StartFailed {
        message: format!("create staged server directory: {error}"),
    })?;
    for entry in fs::read_dir(source).map_err(|error| ServerError::StartFailed {
        message: format!("read staged server directory: {error}"),
    })? {
        let entry = entry.map_err(|error| ServerError::StartFailed {
            message: format!("read staged server entry: {error}"),
        })?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| ServerError::StartFailed {
                message: format!("read staged server file type: {error}"),
            })?;
        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path).map_err(|error| ServerError::StartFailed {
                message: format!("stage server artifact file: {error}"),
            })?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn terminate_child_process_tree(child: &mut Child) -> Result<(), ServerError> {
    let pid = Pid::from_raw(child.id() as i32);
    signal::killpg(pid, Signal::SIGTERM).map_err(|error| ServerError::StopFailed {
        message: format!("signal deno server process group: {error}"),
    })?;
    Ok(())
}

#[cfg(not(unix))]
fn terminate_child_process_tree(child: &mut Child) -> Result<(), ServerError> {
    child.kill().map_err(|error| ServerError::StopFailed {
        message: format!("kill deno server process: {error}"),
    })?;
    Ok(())
}
