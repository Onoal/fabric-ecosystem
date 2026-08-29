use std::path::Path;
use std::sync::{Arc, RwLock, Weak};

use fabric_resource::{ResourceError, ResourceInstanceId};
use fabric_resource_kv::{
    KvAccess, KvEntry, KvListPage, KvListQuery, KvMutation, KvRuntimeApi, KvRuntimeHandle,
    PreparedKv, PreparedKvState,
};
use fjall::{Database, Keyspace, KeyspaceCreateOptions, PersistMode};

use crate::config::FjallKvConfig;

pub(crate) struct FjallKvRuntimeCore {
    config: FjallKvConfig,
    state: RwLock<FjallRuntimeState>,
}

struct FjallRuntimeState {
    generation: u64,
    database: Option<Database>,
}

struct FjallKvRuntime {
    resource_id: ResourceInstanceId,
    generation: u64,
    core: Weak<FjallKvRuntimeCore>,
}

impl FjallKvRuntimeCore {
    pub(crate) fn new(config: FjallKvConfig) -> Self {
        Self {
            config,
            state: RwLock::new(FjallRuntimeState {
                generation: 0,
                database: None,
            }),
        }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.config.root
    }

    pub(crate) fn start(&self) -> Result<(), ResourceError> {
        let mut state = self.state.write().map_err(lock_poisoned)?;
        if state.database.is_some() {
            return Ok(());
        }

        let database = Database::builder(&self.config.root)
            .cache_size(self.config.cache_size_bytes)
            .worker_threads(self.config.worker_threads)
            .max_journaling_size(self.config.max_journaling_size_bytes)
            .manual_journal_persist(true)
            .journal_compression(self.config.journal_compression)
            .open()
            .map_err(map_fjall_prepare)?;
        state.database = Some(database);
        Ok(())
    }

    pub(crate) fn stop(&self) {
        if let Ok(mut state) = self.state.write() {
            state.generation = state.generation.saturating_add(1);
            state.database = None;
        }
    }

    pub(crate) fn testing_keyspace_count(&self) -> usize {
        self.state
            .read()
            .ok()
            .and_then(|state| {
                state
                    .database
                    .as_ref()
                    .map(|db| db.list_keyspace_names().len())
            })
            .unwrap_or(0)
    }

    fn keyspace_create_options(&self) -> KeyspaceCreateOptions {
        KeyspaceCreateOptions::default()
            .manual_journal_persist(true)
            .max_memtable_size(self.config.max_memtable_size_bytes)
    }

    pub(crate) fn access_handle(
        self: &Arc<Self>,
        resource_id: &ResourceInstanceId,
    ) -> Result<KvAccess, ResourceError> {
        {
            let state = self.state.read().map_err(lock_poisoned)?;
            let database = state
                .database
                .as_ref()
                .ok_or_else(|| provider_stopped(resource_id))?;
            let keyspace_name = keyspace_name(resource_id);
            if !database.keyspace_exists(&keyspace_name) {
                return Err(missing_keyspace(resource_id));
            }
        }
        let generation = self.state.read().map_err(lock_poisoned)?.generation;
        let runtime = FjallKvRuntime::shared(resource_id.clone(), generation, Arc::downgrade(self));
        Ok(KvAccess::ProviderOwned {
            runtime: KvRuntimeHandle::new(resource_id.clone(), runtime),
        })
    }

    pub(crate) fn testing_access_handle_with_pause(
        self: &Arc<Self>,
        resource_id: &ResourceInstanceId,
        entered: &std::sync::Barrier,
        release: &std::sync::Barrier,
    ) -> Result<KvAccess, ResourceError> {
        let state = self.state.read().map_err(lock_poisoned)?;
        let database = state
            .database
            .as_ref()
            .ok_or_else(|| provider_stopped(resource_id))?;
        let keyspace_name = keyspace_name(resource_id);
        if !database.keyspace_exists(&keyspace_name) {
            return Err(missing_keyspace(resource_id));
        }
        let generation = state.generation;
        entered.wait();
        release.wait();
        let runtime = FjallKvRuntime::shared(resource_id.clone(), generation, Arc::downgrade(self));
        Ok(KvAccess::ProviderOwned {
            runtime: KvRuntimeHandle::new(resource_id.clone(), runtime),
        })
    }

    fn with_existing_keyspace<T>(
        &self,
        generation: u64,
        resource_id: &ResourceInstanceId,
        f: impl FnOnce(&Database, &Keyspace) -> Result<T, ResourceError>,
    ) -> Result<T, ResourceError> {
        let state = self.state.read().map_err(lock_poisoned)?;
        if state.generation != generation {
            return Err(provider_stopped(resource_id));
        }
        let database = state
            .database
            .as_ref()
            .ok_or_else(|| provider_stopped(resource_id))?;
        let keyspace_name = keyspace_name(resource_id);
        if !database.keyspace_exists(&keyspace_name) {
            return Err(missing_keyspace(resource_id));
        }
        let keyspace = database
            .keyspace(&keyspace_name, || self.keyspace_create_options())
            .map_err(map_fjall_integrity)?;
        f(database, &keyspace)
    }
}

impl FjallKvRuntimeCore {
    pub(crate) fn prepare(
        &self,
        resource_id: &ResourceInstanceId,
    ) -> Result<PreparedKvState, ResourceError> {
        let state = self.state.write().map_err(lock_poisoned)?;
        let database = state
            .database
            .as_ref()
            .ok_or_else(|| provider_stopped(resource_id))?;
        let keyspace_name = keyspace_name(resource_id);
        let created = !database.keyspace_exists(&keyspace_name);
        let _ = database
            .keyspace(&keyspace_name, || self.keyspace_create_options())
            .map_err(map_fjall_prepare)?;
        if created {
            database
                .persist(PersistMode::SyncAll)
                .map_err(map_fjall_prepare)?;
        }
        Ok(PreparedKvState { created })
    }

    pub(crate) fn cleanup(&self, prepared: &PreparedKv) {
        if !prepared.created {
            return;
        }
        let Ok(state) = self.state.write() else {
            return;
        };
        let Some(database) = state.database.as_ref() else {
            return;
        };
        let keyspace_name = keyspace_name(&prepared.resource_id);
        if !database.keyspace_exists(&keyspace_name) {
            return;
        }
        if let Ok(handle) = database.keyspace(&keyspace_name, || self.keyspace_create_options()) {
            let _ = database.delete_keyspace(handle);
            let _ = database.persist(PersistMode::SyncAll);
        }
    }
}

impl FjallKvRuntime {
    fn shared(
        resource_id: ResourceInstanceId,
        generation: u64,
        core: Weak<FjallKvRuntimeCore>,
    ) -> Arc<dyn KvRuntimeApi> {
        Arc::new(Self {
            resource_id,
            generation,
            core,
        }) as Arc<dyn KvRuntimeApi>
    }

    fn with_existing_keyspace<T>(
        &self,
        f: impl FnOnce(&Database, &Keyspace) -> Result<T, ResourceError>,
    ) -> Result<T, ResourceError> {
        let core = self
            .core
            .upgrade()
            .ok_or_else(|| provider_stopped(&self.resource_id))?;
        core.with_existing_keyspace(self.generation, &self.resource_id, f)
    }

    fn durable_put(&self, key: &[u8], value: &[u8]) -> Result<(), ResourceError> {
        self.with_existing_keyspace(|database, keyspace| {
            let mut batch = database.batch().durability(Some(PersistMode::SyncData));
            batch.insert(keyspace, key, value);
            batch.commit().map_err(map_fjall_failed)
        })
    }

    fn durable_delete(&self, key: &[u8]) -> Result<(), ResourceError> {
        self.with_existing_keyspace(|database, keyspace| {
            let mut batch = database.batch().durability(Some(PersistMode::SyncData));
            batch.remove(keyspace, key);
            batch.commit().map_err(map_fjall_failed)
        })
    }
}

impl KvRuntimeApi for FjallKvRuntime {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ResourceError> {
        self.with_existing_keyspace(|_database, keyspace| {
            keyspace
                .get(key)
                .map(|value| value.map(|bytes| bytes.as_ref().to_vec()))
                .map_err(map_fjall_failed)
        })
    }

    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), ResourceError> {
        self.durable_put(key, value)
    }

    fn delete(&self, key: &[u8]) -> Result<(), ResourceError> {
        self.durable_delete(key)
    }

    fn contains(&self, key: &[u8]) -> Result<bool, ResourceError> {
        self.with_existing_keyspace(|_database, keyspace| {
            keyspace.contains_key(key).map_err(map_fjall_failed)
        })
    }

    fn list(&self, query: &KvListQuery) -> Result<KvListPage, ResourceError> {
        self.with_existing_keyspace(|_database, keyspace| {
            let mut entries = Vec::new();
            let mut next_after = None;

            for item in keyspace.prefix(query.prefix()) {
                let key = item.key().map_err(map_fjall_failed)?.as_ref().to_vec();
                if let Some(after) = query.after()
                    && key.as_slice() <= after
                {
                    continue;
                }

                let value = keyspace
                    .get(&key)
                    .map_err(map_fjall_failed)?
                    .ok_or_else(|| ResourceError::Integrity {
                        message: "fjall prefix iteration returned a key without a readable value"
                            .to_owned(),
                    })?
                    .as_ref()
                    .to_vec();

                if entries.len() == query.limit() {
                    next_after = entries.last().map(|entry: &KvEntry| entry.key().to_vec());
                    break;
                }

                entries.push(KvEntry::new(key, value));
            }

            Ok(KvListPage::new(entries, next_after))
        })
    }

    fn write_batch(&self, mutations: &[KvMutation]) -> Result<(), ResourceError> {
        self.with_existing_keyspace(|database, keyspace| {
            let mut batch = database.batch().durability(Some(PersistMode::SyncData));
            for mutation in mutations {
                match mutation {
                    KvMutation::Put { key, value } => batch.insert(keyspace, key, value),
                    KvMutation::Delete { key } => batch.remove(keyspace, key),
                }
            }
            batch.commit().map_err(map_fjall_failed)
        })
    }
}

pub(crate) fn keyspace_name(resource_id: &ResourceInstanceId) -> String {
    resource_id.as_str().to_owned()
}

fn map_fjall_failed(error: impl std::fmt::Display) -> ResourceError {
    ResourceError::Integrity {
        message: error.to_string(),
    }
}

fn map_fjall_prepare(error: impl std::fmt::Display) -> ResourceError {
    ResourceError::PrepareFailed {
        message: error.to_string(),
    }
}

fn map_fjall_integrity(error: impl std::fmt::Display) -> ResourceError {
    ResourceError::Integrity {
        message: error.to_string(),
    }
}

fn lock_poisoned(error: impl std::fmt::Display) -> ResourceError {
    ResourceError::Integrity {
        message: format!("fjall runtime lock poisoned: {error}"),
    }
}

fn provider_stopped(resource_id: &ResourceInstanceId) -> ResourceError {
    ResourceError::Integrity {
        message: format!(
            "kv provider is not running for resource {}",
            resource_id.as_str()
        ),
    }
}

fn missing_keyspace(resource_id: &ResourceInstanceId) -> ResourceError {
    ResourceError::Integrity {
        message: format!(
            "kv resource {} has not been prepared or no longer exists",
            resource_id.as_str()
        ),
    }
}
