#![forbid(unsafe_code)]

mod contract;
mod model;
mod native;
#[cfg(test)]
mod tests;

pub use contract::{KvContract, KvService, kv_contract_id, kv_contract_key, kv_resource_id};
pub use model::{
    KvAccess, KvBinding, KvEntry, KvListPage, KvListQuery, KvMutation, KvRef, KvRuntimeApi,
    KvRuntimeHandle, PreparedKv, PreparedKvState,
};
pub use native::{KvAdapter, NativeKv};
