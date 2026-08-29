use axum::Json;
use axum::extract::State;
use axum::routing::post;
use axum::{Router, response::IntoResponse};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use fabric_resource_kv::{KvAccess, KvContract, KvRef};
use serde::{Deserialize, Serialize};

use crate::{BindingProjection, WorkerError, WorkloadBinding, WorkloadBindingProjection};
use fabric_projection::{ProjectionError, ProjectionLeases};

use super::bridge::{bind_loopback_listener, spawn_router};
use crate::projection::PreparedWorkloadProjections;

#[derive(Clone)]
struct KvBridgeConfig {
    access: KvAccess,
}

#[derive(Deserialize)]
struct KeyRequest {
    key_b64: String,
}

#[derive(Deserialize)]
struct PutRequest {
    key_b64: String,
    value_b64: String,
}

#[derive(Serialize)]
struct GetResponse {
    found: bool,
    value_b64: Option<String>,
}

#[derive(Serialize)]
struct SuccessResponse {
    success: bool,
}

pub(crate) fn project_kv_binding(
    binding: &WorkloadBinding,
    reference: &KvRef,
    projections: &[WorkloadBindingProjection],
    kv: &KvContract,
    prepared: &mut PreparedWorkloadProjections,
) -> Result<(), ProjectionError> {
    let access = kv.access(reference).map_err(|error| {
        ProjectionError::materialization_failed(format!(
            "resolve kv binding {}: {error}",
            binding.name().as_str()
        ))
    })?;
    let (listener, address) = bind_loopback_listener().map_err(map_projection_error)?;
    let lease = spawn_router(listener, kv_router(KvBridgeConfig { access }))
        .map_err(map_projection_error)?;
    let base_url = format!("http://127.0.0.1:{}/kv", address.port());
    prepared.allow_network_authority(format!("127.0.0.1:{}", address.port()));
    for projection in projections {
        match projection.projection() {
            BindingProjection::Structured => {
                prepared.insert_structured(
                    binding.name().as_str(),
                    serde_json::json!({
                        "kind": "kv",
                        "baseUrl": base_url,
                    }),
                );
            }
            BindingProjection::Environment(variable) => {
                prepared.insert_public_environment(variable.as_str(), base_url.clone());
            }
        }
    }
    let mut leases = ProjectionLeases::default();
    leases.push(lease);
    prepared.append_leases(leases);
    Ok(())
}

fn map_projection_error(error: WorkerError) -> ProjectionError {
    ProjectionError::materialization_failed(error.to_string())
}

fn kv_router(config: KvBridgeConfig) -> Router {
    Router::new()
        .route("/kv/get", post(get))
        .route("/kv/put", post(put))
        .route("/kv/delete", post(delete_key))
        .with_state(config)
}

async fn get(
    State(config): State<KvBridgeConfig>,
    Json(request): Json<KeyRequest>,
) -> impl IntoResponse {
    let key = B64.decode(request.key_b64).unwrap_or_default();
    let value = config.access.get(&key).ok().flatten();
    Json(GetResponse {
        found: value.is_some(),
        value_b64: value.map(|value| B64.encode(value)),
    })
}

async fn put(
    State(config): State<KvBridgeConfig>,
    Json(request): Json<PutRequest>,
) -> impl IntoResponse {
    let key = B64.decode(request.key_b64).unwrap_or_default();
    let value = B64.decode(request.value_b64).unwrap_or_default();
    let _ = config.access.put(&key, &value);
    Json(SuccessResponse { success: true })
}

async fn delete_key(
    State(config): State<KvBridgeConfig>,
    Json(request): Json<KeyRequest>,
) -> impl IntoResponse {
    let key = B64.decode(request.key_b64).unwrap_or_default();
    let _ = config.access.delete(&key);
    Json(SuccessResponse { success: true })
}
