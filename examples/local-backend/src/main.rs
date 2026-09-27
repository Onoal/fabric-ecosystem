mod app;
mod client;

use std::io::Error;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use app::{ExampleBackendApp, ExampleBackendAppInstanceApi};
use fabric::prelude::*;
use fabric_composition_local_backend::{local_backend_stack, LocalBackendCompositionConfig};
use fabric_package_networking_http::{HttpResponse, HttpServer, HttpServerInstanceApi};
use fabric_package_networking_tcp::{TcpTransportInspector, TcpTransportInspectorInstanceApi};
use futures::executor::block_on;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_file = TempDatabaseFile::new()?;
    let composition = Fabric::new("fabric.ecosystem.example.local-backend")?
        .with(local_backend_stack(LocalBackendCompositionConfig::local(
            "api",
            "primary",
            database_file.path().to_path_buf(),
            "application",
        )))
        .with(app::backend_app("primary", "application"))
        .build()?;
    let mut instance = composition.materialize_on(
        "fabric.ecosystem.example.local-backend.local",
        &HostDescriptor::native(),
    )?;
    instance.start()?;

    let app = instance.component::<ExampleBackendApp>()?;
    app.reconcile()?;
    let stored = block_on(app.write_read_log("local-data".to_owned()))??;

    let inspector = instance.component::<TcpTransportInspector>()?;
    inspector.reconcile()?;
    let address = block_on(inspector.inspect_transport())?
        .actual
        .ok_or_else(|| Error::other("local backend did not bind a TCP address"))?;
    let server = instance.component::<HttpServer>()?;
    server.reconcile()?;

    let target = "/local-backend";
    let client = client::get(address.clone(), target);
    let exchange = block_on(server.accept_exchange())??;
    let request_target = exchange.request().target.clone();
    let response_body = format!("{} via {}", stored.value, request_target);
    exchange.respond(HttpResponse::new(200, response_body.clone().into_bytes()))?;
    let response = join_client(client)?;

    instance.stop()?;

    println!("database value: {}", stored.value);
    println!(
        "log target: {}",
        stored.logged.target.as_deref().unwrap_or("missing")
    );
    println!("HTTP server bound to {}:{}", address.host, address.port);
    println!("request: GET {request_target}");
    println!("response: 200 {response_body}");
    println!("raw response bytes: {}", response.len());

    Ok(())
}

struct TempDatabaseFile {
    path: PathBuf,
}

impl TempDatabaseFile {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        Ok(Self {
            path: std::env::temp_dir().join(format!(
                "onoal-fabric-local-backend-example-{}-{nanos}.db",
                std::process::id()
            )),
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDatabaseFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn join_client(
    client: std::thread::JoinHandle<std::io::Result<String>>,
) -> Result<String, Box<dyn std::error::Error>> {
    client
        .join()
        .map_err(|_| Error::other("local backend client thread panicked"))?
        .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
}
