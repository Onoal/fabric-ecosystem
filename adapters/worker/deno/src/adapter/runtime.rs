use std::collections::BTreeMap;
use std::fs;
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use fabric_projection::ProjectionLeases;
use fabric_resource_worker::{
    DispatchHttpRequest, HttpHeader, HttpResponse, WorkerApiVersion, WorkerCapabilities,
    WorkerError, WorkerExecution, WorkerExecutionRequest, WorkerFeature, WorkerFeatureSupport,
    WorkerSpec, WorkloadArtifact,
};
use fabric_resource_worker::{PreparedWorkloadEnvironmentValue, PreparedWorkloadProjections};
#[cfg(unix)]
use nix::sys::signal::{self, Signal};
#[cfg(unix)]
use nix::unistd::Pid;
use sha2::{Digest, Sha256};
use ureq::RequestExt;

use crate::artifact::{DenoArtifactResolver, ResolvedDenoArtifact};
use crate::config::DenoWorkerConfig;

const EXPECTED_DENO_VERSION: &str = "2.9.5";

pub(crate) struct DenoWorkerRuntime {
    config: DenoWorkerConfig,
    resolver: Arc<dyn DenoArtifactResolver>,
    prepared: BTreeMap<fabric_resource_worker::WorkloadId, PreparedArtifact>,
    binary_verified: bool,
}

#[derive(Clone)]
struct PreparedArtifact {
    resolved: ResolvedDenoArtifact,
}

pub(crate) struct DenoWorkerExecution {
    endpoint: SocketAddr,
    runtime_dir: PathBuf,
    child: Child,
    projection_leases: ProjectionLeases,
}

impl DenoWorkerRuntime {
    pub(crate) fn new(
        config: DenoWorkerConfig,
        resolver: Arc<dyn DenoArtifactResolver>,
    ) -> Result<Self, WorkerError> {
        Ok(Self {
            config,
            resolver,
            prepared: BTreeMap::new(),
            binary_verified: false,
        })
    }

    pub(crate) fn capabilities(&self) -> WorkerCapabilities {
        WorkerCapabilities {
            api_versions: vec![WorkerApiVersion::parse("1.0.0").expect("static version")],
            features: vec![
                WorkerFeatureSupport {
                    feature: WorkerFeature::new("js.module").expect("static feature"),
                    supported: true,
                },
                WorkerFeatureSupport {
                    feature: WorkerFeature::new("http.fetch").expect("static feature"),
                    supported: true,
                },
            ],
        }
    }

    pub(crate) fn prepare_worker(&mut self, worker: &WorkerSpec) -> Result<(), WorkerError> {
        let resolved = self.resolve_and_verify(&worker.artifact, &worker.entrypoint)?;
        self.prepared
            .insert(worker.workload_id.clone(), PreparedArtifact { resolved });
        Ok(())
    }

    pub(crate) fn start_execution(
        &mut self,
        request: WorkerExecutionRequest,
    ) -> Result<Box<dyn WorkerExecution>, WorkerError> {
        self.verify_deno_binary()?;
        let prepared = self
            .prepared
            .get(&request.start.worker.workload_id)
            .ok_or_else(|| WorkerError::ProtocolViolation {
                message: "worker was not prepared by this deno runtime".to_owned(),
            })?;

        let runtime_dir = self.config.runtime_root.join(request.instance_id.as_str());
        reset_runtime_dir(&runtime_dir)?;
        let staged_entry = stage_artifact(&prepared.resolved, &runtime_dir)?;
        let ResolvedBindings {
            bindings_json,
            environment,
            network_authorities,
            mut projection_leases,
        } = serialize_projections(request.projections)?;
        let endpoint = bind_loopback_socket()?;
        let bootstrap_path = runtime_dir.join("bootstrap.ts");
        fs::write(
            &bootstrap_path,
            bootstrap_source(endpoint.port(), &staged_entry, &bindings_json),
        )
        .map_err(|error| WorkerError::StartFailed {
            message: format!("write deno bootstrap: {error}"),
        })?;
        let stdout_log = fs::File::create(runtime_dir.join("stdout.log")).map_err(|error| {
            WorkerError::StartFailed {
                message: format!("create deno stdout log: {error}"),
            }
        })?;
        let stderr_log = fs::File::create(runtime_dir.join("stderr.log")).map_err(|error| {
            WorkerError::StartFailed {
                message: format!("create deno stderr log: {error}"),
            }
        })?;
        let mut command = Command::new(&self.config.deno_bin);
        command
            .arg("run")
            .arg("--no-prompt")
            .arg("--quiet")
            .arg(format!("--allow-read={}", runtime_dir.display()))
            .arg(format!(
                "--allow-net={}",
                allowed_net_argument(endpoint, &network_authorities,)
            ));
        if let Some(allowed_env) = allowed_env_argument(&environment) {
            command.arg(format!("--allow-env={allowed_env}"));
            command.envs(exposed_environment(&environment));
        }
        command
            .arg(&bootstrap_path)
            .stdout(Stdio::from(stdout_log))
            .stderr(Stdio::from(stderr_log));
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn().map_err(|error| WorkerError::StartFailed {
            message: format!("spawn deno: {error}"),
        })?;
        if let Err(error) = wait_for_readiness(&mut child, endpoint, self.config.startup_timeout) {
            let _ = terminate_child_process_tree(&mut child);
            let _ = child.wait();
            projection_leases.release_all();
            let _ = fs::remove_dir_all(&runtime_dir);
            return Err(error);
        }
        Ok(Box::new(DenoWorkerExecution {
            endpoint,
            runtime_dir,
            child,
            projection_leases,
        }))
    }

    pub(crate) fn clear(&mut self) {
        self.prepared.clear();
    }

    fn verify_deno_binary(&mut self) -> Result<(), WorkerError> {
        if self.binary_verified {
            return Ok(());
        }
        let output = Command::new(&self.config.deno_bin)
            .arg("--version")
            .output()
            .map_err(|error| WorkerError::StartFailed {
                message: format!("run deno --version: {error}"),
            })?;
        if !output.status.success() {
            return Err(WorkerError::StartFailed {
                message: "deno --version failed".to_owned(),
            });
        }
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if !combined.contains(EXPECTED_DENO_VERSION) {
            return Err(WorkerError::StartFailed {
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
        artifact: &WorkloadArtifact,
        entrypoint: &fabric_resource_worker::WorkloadEntrypoint,
    ) -> Result<ResolvedDenoArtifact, WorkerError> {
        let resolved = self.resolver.resolve(artifact)?;
        let artifact_file = canonical_file(&resolved.artifact_file, "artifact")?;
        let module_root = canonical_directory(&resolved.module_root, "module root")?;
        let entry_module = canonical_file(&module_root.join(entrypoint.as_str()), "entry module")?;
        if !entry_module.starts_with(&module_root) {
            return Err(WorkerError::PrepareFailed {
                message: "entry module is outside the trusted module root".to_owned(),
            });
        }
        let actual = Sha256::digest(fs::read(&artifact_file).map_err(|error| {
            WorkerError::PrepareFailed {
                message: format!("read materialized artifact: {error}"),
            }
        })?);
        if actual.as_slice() != artifact.sha256 {
            return Err(WorkerError::PrepareFailed {
                message: "materialized artifact digest does not match workload artifact".to_owned(),
            });
        }
        Ok(ResolvedDenoArtifact {
            artifact_file,
            module_root,
        })
    }
}

impl WorkerExecution for DenoWorkerExecution {
    fn dispatch_http(&mut self, request: DispatchHttpRequest) -> Result<HttpResponse, WorkerError> {
        request.request.validate()?;
        if let Some(status) =
            self.child
                .try_wait()
                .map_err(|error| WorkerError::DispatchFailed {
                    message: format!("inspect deno process: {error}"),
                })?
        {
            return Err(WorkerError::DispatchFailed {
                message: format!("deno exited before dispatch with status {status}"),
            });
        }
        dispatch_to_deno(self.endpoint, &request.request)
    }

    fn stop(&mut self) -> Result<(), WorkerError> {
        terminate_child_process_tree(&mut self.child)?;
        self.child.wait().map_err(|error| WorkerError::StopFailed {
            message: format!("wait for deno shutdown: {error}"),
        })?;
        Ok(())
    }

    fn cleanup(&mut self) {
        let _ = terminate_child_process_tree(&mut self.child);
        let _ = self.child.wait();
        self.projection_leases.release_all();
        let _ = fs::remove_dir_all(&self.runtime_dir);
    }
}

struct ResolvedBindings {
    bindings_json: String,
    environment: BTreeMap<String, PreparedWorkloadEnvironmentValue>,
    network_authorities: std::collections::BTreeSet<String>,
    projection_leases: ProjectionLeases,
}

fn serialize_projections(
    projections: PreparedWorkloadProjections,
) -> Result<ResolvedBindings, WorkerError> {
    let (structured, environment, network_authorities, projection_leases) =
        projections.into_parts();
    let bindings_json =
        serde_json::to_string(&structured).map_err(|error| WorkerError::StartFailed {
            message: format!("serialize binding projection: {error}"),
        })?;
    Ok(ResolvedBindings {
        bindings_json,
        environment,
        network_authorities,
        projection_leases,
    })
}

fn reset_runtime_dir(runtime_dir: &Path) -> Result<(), WorkerError> {
    if runtime_dir.exists() {
        fs::remove_dir_all(runtime_dir).map_err(|error| WorkerError::StartFailed {
            message: format!("reset deno runtime dir: {error}"),
        })?;
    }
    fs::create_dir_all(runtime_dir).map_err(|error| WorkerError::StartFailed {
        message: format!("create deno runtime dir: {error}"),
    })?;
    Ok(())
}

fn stage_artifact(
    resolved: &ResolvedDenoArtifact,
    runtime_dir: &Path,
) -> Result<PathBuf, WorkerError> {
    let staged_root = runtime_dir.join("artifact");
    copy_dir_recursive(&resolved.module_root, &staged_root)?;
    Ok(staged_root.join("main.ts"))
}

fn bootstrap_source(port: u16, staged_entry: &Path, bindings_json: &str) -> String {
    format!(
        r#"
const workload = await import("file://{entry}");
const bindings = Object.freeze({bindings});
const handler = typeof workload.fetch === "function"
  ? workload.fetch
  : typeof workload.default === "function"
    ? workload.default
    : typeof workload.default?.fetch === "function"
      ? workload.default.fetch.bind(workload.default)
      : null;

if (!handler) {{
  throw new Error("workload must export fetch or default handler");
}}

Deno.serve({{ hostname: "127.0.0.1", port: {port} }}, async (request) => {{
  const url = new URL(request.url);
  if (url.pathname === "/__fabric_ready") {{
    return new Response(null, {{ status: 204 }});
  }}
  return await handler(request, {{ bindings }});
}});
"#,
        entry = staged_entry.display(),
        bindings = bindings_json,
        port = port
    )
}

fn allowed_net_argument(
    endpoint: SocketAddr,
    network_authorities: &std::collections::BTreeSet<String>,
) -> String {
    let mut allowed = vec![format!("127.0.0.1:{}", endpoint.port())];
    allowed.extend(network_authorities.iter().cloned());
    allowed.join(",")
}

fn allowed_env_argument(
    environment: &BTreeMap<String, PreparedWorkloadEnvironmentValue>,
) -> Option<String> {
    if environment.is_empty() {
        None
    } else {
        Some(environment.keys().cloned().collect::<Vec<_>>().join(","))
    }
}

fn exposed_environment(
    environment: &BTreeMap<String, PreparedWorkloadEnvironmentValue>,
) -> BTreeMap<String, String> {
    environment
        .iter()
        .map(|(name, value)| (name.clone(), value.expose().to_owned()))
        .collect()
}

fn bind_loopback_socket() -> Result<SocketAddr, WorkerError> {
    let listener =
        TcpListener::bind(("127.0.0.1", 0)).map_err(|error| WorkerError::StartFailed {
            message: format!("bind deno loopback socket: {error}"),
        })?;
    listener
        .local_addr()
        .map_err(|error| WorkerError::StartFailed {
            message: format!("resolve deno loopback socket: {error}"),
        })
}

fn wait_for_readiness(
    child: &mut Child,
    endpoint: SocketAddr,
    timeout: Duration,
) -> Result<(), WorkerError> {
    let deadline = Instant::now() + timeout;
    let ready_url = format!("http://127.0.0.1:{}/__fabric_ready", endpoint.port());
    loop {
        if let Some(status) = child.try_wait().map_err(|error| WorkerError::StartFailed {
            message: format!("inspect deno process: {error}"),
        })? {
            return Err(WorkerError::StartFailed {
                message: format!("deno exited before readiness with status {status}"),
            });
        }
        match ureq::get(&ready_url).call() {
            Ok(response) if response.status() == 204 => return Ok(()),
            _ if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                return Err(WorkerError::StartFailed {
                    message: "deno readiness probe timed out".to_owned(),
                });
            }
        }
    }
}

fn dispatch_to_deno(
    endpoint: SocketAddr,
    request: &fabric_resource_worker::HttpRequest,
) -> Result<HttpResponse, WorkerError> {
    let target = dispatch_url(endpoint, &request.url)?;
    let mut builder = ureq::http::Request::builder()
        .method(request.method.as_str())
        .uri(&target);
    for header in &request.headers {
        builder = builder.header(&header.name, &header.value);
    }
    let request =
        builder
            .body(request.body.clone())
            .map_err(|error| WorkerError::DispatchFailed {
                message: format!("build worker request: {error}"),
            })?;
    let response = request
        .with_default_agent()
        .configure()
        .http_status_as_error(false)
        .run()
        .map_err(|error| WorkerError::DispatchFailed {
            message: format!("dispatch worker request: {error}"),
        })?;
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            HttpHeader::new(
                name.to_string(),
                value.to_str().unwrap_or_default().to_owned(),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| WorkerError::DispatchFailed {
            message: error.to_string(),
        })?;
    let body = response
        .into_body()
        .read_to_vec()
        .map_err(|error| WorkerError::DispatchFailed {
            message: format!("read worker response body: {error}"),
        })?;
    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

fn dispatch_url(endpoint: SocketAddr, request_url: &str) -> Result<String, WorkerError> {
    if let Ok(url) = url::Url::parse(request_url) {
        let path = url.path();
        let query = url
            .query()
            .map(|query| format!("?{query}"))
            .unwrap_or_default();
        return Ok(format!(
            "http://127.0.0.1:{}{}{}",
            endpoint.port(),
            path,
            query
        ));
    }
    if request_url.starts_with('/') {
        return Ok(format!(
            "http://127.0.0.1:{}{}",
            endpoint.port(),
            request_url
        ));
    }
    Err(WorkerError::DispatchFailed {
        message: "request url must be absolute or start with '/'".to_owned(),
    })
}

fn canonical_file(path: &Path, label: &str) -> Result<PathBuf, WorkerError> {
    let canonical = path
        .canonicalize()
        .map_err(|error| WorkerError::PrepareFailed {
            message: format!("canonicalize {label}: {error}"),
        })?;
    if canonical.is_file() {
        Ok(canonical)
    } else {
        Err(WorkerError::PrepareFailed {
            message: format!("{label} is not a file"),
        })
    }
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, WorkerError> {
    let canonical = path
        .canonicalize()
        .map_err(|error| WorkerError::PrepareFailed {
            message: format!("canonicalize {label}: {error}"),
        })?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(WorkerError::PrepareFailed {
            message: format!("{label} is not a directory"),
        })
    }
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), WorkerError> {
    fs::create_dir_all(target).map_err(|error| WorkerError::PrepareFailed {
        message: format!("create staged module root: {error}"),
    })?;
    for entry in fs::read_dir(source).map_err(|error| WorkerError::PrepareFailed {
        message: format!("read module root: {error}"),
    })? {
        let entry = entry.map_err(|error| WorkerError::PrepareFailed {
            message: format!("read module entry: {error}"),
        })?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path).map_err(|error| WorkerError::PrepareFailed {
                message: format!("stage module file {}: {error}", source_path.display()),
            })?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn terminate_child_process_tree(child: &mut Child) -> Result<(), WorkerError> {
    let pid = Pid::from_raw(child.id() as i32);
    signal::killpg(pid, Signal::SIGTERM).map_err(|error| WorkerError::StopFailed {
        message: format!("terminate deno process group: {error}"),
    })?;
    Ok(())
}

#[cfg(not(unix))]
fn terminate_child_process_tree(child: &mut Child) -> Result<(), WorkerError> {
    child.kill().map_err(|error| WorkerError::StopFailed {
        message: format!("terminate deno process: {error}"),
    })
}
