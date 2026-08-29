use std::path::PathBuf;

use fjall::CompressionType;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FjallKvConfig {
    pub root: PathBuf,
    pub worker_threads: usize,
    pub cache_size_bytes: u64,
    pub max_journaling_size_bytes: u64,
    pub max_memtable_size_bytes: u64,
    pub journal_compression: CompressionType,
}

impl FjallKvConfig {
    pub const DEFAULT_WORKER_THREADS: usize = 1;
    pub const DEFAULT_CACHE_SIZE_BYTES: u64 = 32 * 1024 * 1024;
    pub const DEFAULT_MAX_JOURNALING_SIZE_BYTES: u64 = 64 * 1024 * 1024;
    pub const DEFAULT_MAX_MEMTABLE_SIZE_BYTES: u64 = 8 * 1024 * 1024;
    pub const DEFAULT_JOURNAL_COMPRESSION: CompressionType = CompressionType::Lz4;

    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            worker_threads: Self::DEFAULT_WORKER_THREADS,
            cache_size_bytes: Self::DEFAULT_CACHE_SIZE_BYTES,
            max_journaling_size_bytes: Self::DEFAULT_MAX_JOURNALING_SIZE_BYTES,
            max_memtable_size_bytes: Self::DEFAULT_MAX_MEMTABLE_SIZE_BYTES,
            journal_compression: Self::DEFAULT_JOURNAL_COMPRESSION,
        }
    }
}
