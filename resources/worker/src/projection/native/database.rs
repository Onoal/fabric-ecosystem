use std::path::PathBuf;

use axum::Json;
use axum::extract::State;
use axum::routing::post;
use axum::{Router, response::IntoResponse};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use fabric_resource_database::{DatabaseRef, SqliteDatabaseCompatibilityContract};
use rusqlite::{Connection, types::ValueRef};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::{BindingProjection, WorkerError, WorkloadBinding, WorkloadBindingProjection};
use fabric_projection::{ProjectionError, ProjectionLeases};

use super::bridge::{bind_loopback_listener, spawn_router};
use crate::projection::PreparedWorkloadProjections;

#[derive(Clone)]
struct DatabaseBridgeConfig {
    path: PathBuf,
}

#[derive(Deserialize)]
struct QueryRequest {
    sql: String,
}

#[derive(Serialize)]
struct QueryResponse {
    rows: Vec<Vec<JsonValue>>,
}

#[derive(Serialize)]
struct ExecuteResponse {
    success: bool,
}

pub(crate) fn project_database_binding(
    binding: &WorkloadBinding,
    reference: &DatabaseRef,
    projections: &[WorkloadBindingProjection],
    database: &SqliteDatabaseCompatibilityContract,
    prepared: &mut PreparedWorkloadProjections,
) -> Result<(), ProjectionError> {
    let materialization = database.materialize_sqlite(reference).map_err(|error| {
        ProjectionError::materialization_failed(format!(
            "materialize database binding {}: {error}",
            binding.name().as_str()
        ))
    })?;
    let (listener, address) = bind_loopback_listener().map_err(map_projection_error)?;
    let lease = spawn_router(
        listener,
        database_router(DatabaseBridgeConfig {
            path: materialization.sqlite_file_path().to_path_buf(),
        }),
    )
    .map_err(map_projection_error)?;
    let base_url = format!("http://127.0.0.1:{}/db", address.port());
    prepared.allow_network_authority(format!("127.0.0.1:{}", address.port()));
    for projection in projections {
        match projection.projection() {
            BindingProjection::Structured => {
                prepared.insert_structured(
                    binding.name().as_str(),
                    serde_json::json!({
                        "kind": "database",
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

fn database_router(config: DatabaseBridgeConfig) -> Router {
    Router::new()
        .route("/db/query", post(query))
        .route("/db/execute", post(execute))
        .with_state(config)
}

async fn query(
    State(config): State<DatabaseBridgeConfig>,
    Json(request): Json<QueryRequest>,
) -> impl IntoResponse {
    let connection = match Connection::open(&config.path) {
        Ok(connection) => connection,
        Err(error) => {
            return Json(QueryResponse {
                rows: vec![vec![JsonValue::String(error.to_string())]],
            });
        }
    };
    let mut statement = match connection.prepare(&request.sql) {
        Ok(statement) => statement,
        Err(error) => {
            return Json(QueryResponse {
                rows: vec![vec![JsonValue::String(error.to_string())]],
            });
        }
    };
    let column_count = statement.column_count();
    let rows = statement
        .query_map([], |row| {
            let mut values = Vec::new();
            for index in 0..column_count {
                values.push(sqlite_value_to_json(row.get_ref(index)?));
            }
            Ok(values)
        })
        .and_then(|mapped| mapped.collect::<Result<Vec<_>, _>>());
    match rows {
        Ok(rows) => Json(QueryResponse { rows }),
        Err(error) => Json(QueryResponse {
            rows: vec![vec![JsonValue::String(error.to_string())]],
        }),
    }
}

async fn execute(
    State(config): State<DatabaseBridgeConfig>,
    Json(request): Json<QueryRequest>,
) -> impl IntoResponse {
    let connection = Connection::open(&config.path);
    if let Ok(connection) = connection {
        let _ = connection.execute_batch(&request.sql);
    }
    Json(ExecuteResponse { success: true })
}

fn sqlite_value_to_json(value: ValueRef<'_>) -> JsonValue {
    match value {
        ValueRef::Null => JsonValue::Null,
        ValueRef::Integer(value) => JsonValue::from(value),
        ValueRef::Real(value) => JsonValue::from(value),
        ValueRef::Text(value) => JsonValue::String(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => JsonValue::String(B64.encode(value)),
    }
}
