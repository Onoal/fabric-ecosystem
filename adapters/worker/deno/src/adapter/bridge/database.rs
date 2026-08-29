use std::path::PathBuf;

use axum::Json;
use axum::extract::State;
use axum::routing::post;
use axum::{Router, response::IntoResponse};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use rusqlite::{Connection, types::ValueRef};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Clone)]
pub(crate) struct DatabaseBridgeConfig {
    pub(crate) path: PathBuf,
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

pub(crate) fn database_router(config: DatabaseBridgeConfig) -> Router {
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
                let value = row.get_ref(index)?;
                values.push(sqlite_value_to_json(value));
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
