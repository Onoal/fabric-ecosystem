use axum::Json;
use axum::extract::State;
use axum::routing::post;
use axum::{Router, response::IntoResponse};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};
use fabric_resource_kv::KvAccess;

#[derive(Clone)]
pub(crate) struct KvBridgeConfig {
    pub(crate) access: KvAccess,
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

pub(crate) fn kv_router(config: KvBridgeConfig) -> Router {
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
