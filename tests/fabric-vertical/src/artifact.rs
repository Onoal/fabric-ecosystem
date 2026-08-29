use std::path::PathBuf;
use std::sync::Arc;

use fabric_adapter_worker_deno::{DenoArtifactResolver, ResolvedDenoArtifact};
use fabric_resource_worker::{WorkerError, WorkloadArtifact};
use sha2::{Digest, Sha256};

use crate::layout::HarnessLayout;

pub(crate) struct ArtifactFixture {
    pub(crate) artifact: WorkloadArtifact,
    pub(crate) resolver: Arc<dyn DenoArtifactResolver>,
}

#[derive(Clone)]
struct FixtureResolver {
    artifact_file: PathBuf,
    module_root: PathBuf,
}

impl DenoArtifactResolver for FixtureResolver {
    fn resolve(&self, _artifact: &WorkloadArtifact) -> Result<ResolvedDenoArtifact, WorkerError> {
        Ok(ResolvedDenoArtifact {
            artifact_file: self.artifact_file.clone(),
            module_root: self.module_root.clone(),
        })
    }
}

pub(crate) fn create_artifact_fixture(layout: &HarnessLayout) -> ArtifactFixture {
    let artifacts_root = layout.artifacts_root();
    let module_root = artifacts_root.join("module");
    std::fs::create_dir_all(&module_root).expect("create module root");

    let entry = module_root.join("main.ts");
    std::fs::write(&entry, workload_source()).expect("write fv0 workload");

    let artifact_file = artifacts_root.join("artifact.bin");
    std::fs::write(&artifact_file, b"fv0-deno-artifact").expect("write artifact bytes");
    let sha256 = Sha256::digest(std::fs::read(&artifact_file).expect("read artifact bytes"));

    ArtifactFixture {
        artifact: WorkloadArtifact {
            reference: "stel://fv0/semantic-vertical".to_owned(),
            sha256: sha256.into(),
        },
        resolver: Arc::new(FixtureResolver {
            artifact_file,
            module_root,
        }),
    }
}

fn workload_source() -> &'static str {
    r#"
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

function toBase64(value) {
  let binary = "";
  for (const byte of value) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
}

function fromBase64(value) {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function readOptionalEnv(name) {
  try {
    return Deno.env.get(name);
  } catch (_error) {
    return null;
  }
}

async function dbExecute(binding, sql) {
  await globalThis.fetch(`${binding.baseUrl}/execute`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ sql }),
  });
}

async function dbQuery(binding, sql) {
  const response = await globalThis.fetch(`${binding.baseUrl}/query`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ sql }),
  });
  return await response.json();
}

async function kvPut(binding, key, value) {
  await globalThis.fetch(`${binding.baseUrl}/put`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      key_b64: toBase64(textEncoder.encode(key)),
      value_b64: toBase64(textEncoder.encode(value)),
    }),
  });
}

async function kvGet(binding, key) {
  const response = await globalThis.fetch(`${binding.baseUrl}/get`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      key_b64: toBase64(textEncoder.encode(key)),
    }),
  });
  const json = await response.json();
  if (!json.found || !json.value_b64) {
    return null;
  }
  return textDecoder.decode(fromBase64(json.value_b64));
}

async function readState(bindings) {
  const dbBaseUrl = readOptionalEnv("DB_BASE_URL");
  if (!dbBaseUrl) {
    throw new Error("missing DB_BASE_URL projection");
  }
  const secretValue = readOptionalEnv("FABRIC_TEST_SECRET");
  if (!secretValue) {
    throw new Error("missing FABRIC_TEST_SECRET projection");
  }
  const rows = await dbQuery(
    { baseUrl: dbBaseUrl },
    "SELECT value FROM fv0_state WHERE key = 'db' LIMIT 1;",
  );
  const dbValue = rows.rows?.[0]?.[0] ?? null;
  const kvValue = await kvGet(bindings.CACHE, "fv0-state");
  return { dbValue, dbBaseUrl, kvValue, secretValue };
}

export async function fetch(request, env) {
  const bindings = env.bindings;
  if (request.method === "GET" && new URL(request.url).pathname === "/secret") {
    const secretValue = readOptionalEnv("FABRIC_TEST_SECRET");
    if (!secretValue) {
      return Response.json(
        { secretValue: null, error: "missing FABRIC_TEST_SECRET projection" },
        { status: 503 },
      );
    }
    return Response.json({ secretValue });
  }

  if (!bindings?.DB || !bindings?.CACHE) {
    return new Response("missing bindings", { status: 500 });
  }

  const url = new URL(request.url);

  if (request.method === "POST" && url.pathname === "/state") {
    const payload = await request.json();
    await dbExecute(
      bindings.DB,
      "CREATE TABLE IF NOT EXISTS fv0_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    );
    await dbExecute(
      bindings.DB,
      `INSERT INTO fv0_state(key, value) VALUES ('db', '${payload.dbValue}')
       ON CONFLICT(key) DO UPDATE SET value = excluded.value;`,
    );
    await kvPut(bindings.CACHE, "fv0-state", payload.kvValue);
    return Response.json(await readState(bindings));
  }

  if (request.method === "GET" && url.pathname === "/state") {
    return Response.json(await readState(bindings));
  }

  return new Response("not found", { status: 404 });
}
"#
}
