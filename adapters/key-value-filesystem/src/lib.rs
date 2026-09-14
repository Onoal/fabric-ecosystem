use std::{
    fs,
    path::{Path, PathBuf},
};

use fabric_resource_key_value::{KeyValueError, KeyValueStore, KeyValueStoreRealization};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileLayout {
    Flat,
    Sharded { depth: u8 },
}

fn encoded_key(key: &[u8]) -> Result<String, KeyValueError> {
    if key.is_empty() {
        return Err(KeyValueError::InvalidKey);
    }
    Ok(key.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn key_path(root: &Path, layout: &FileLayout, key: &[u8]) -> Result<PathBuf, KeyValueError> {
    let encoded = encoded_key(key)?;
    let mut path = root.to_path_buf();
    if let FileLayout::Sharded { depth } = layout {
        for index in 0..usize::from(*depth) {
            let start = index * 2;
            if start + 2 <= encoded.len() {
                path.push(&encoded[start..start + 2]);
            }
        }
    }
    Ok(path.join(encoded))
}

fabric_sdk::adapter! {
    pub FilesystemKeyValueAdapter
        for resource KeyValueStore
        implements KeyValueStoreRealization
    {
        schema: "^1";
        realization: "1.0.0";

        config { root: PathBuf; layout: FileLayout; fsync: bool; }

        runtime {
            fn get(&self, key: Vec<u8>) -> Result<Option<Vec<u8>>, KeyValueError> {
                let path = key_path(&self.config.root, &self.config.layout, &key)?;
                match fs::read(path) {
                    Ok(value) => Ok(Some(value)),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    Err(_) => Err(KeyValueError::Unavailable),
                }
            }

            fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), KeyValueError> {
                let path = key_path(&self.config.root, &self.config.layout, &key)?;
                let parent = path.parent().ok_or(KeyValueError::Unavailable)?;
                fs::create_dir_all(parent).map_err(|_| KeyValueError::Unavailable)?;
                fs::write(&path, value).map_err(|_| KeyValueError::Unavailable)?;
                if self.config.fsync {
                    fs::OpenOptions::new().read(true).open(path).map_err(|_| KeyValueError::Unavailable)?.sync_all().map_err(|_| KeyValueError::Unavailable)?;
                }
                Ok(())
            }

            fn delete(&self, key: Vec<u8>) -> Result<(), KeyValueError> {
                let path = key_path(&self.config.root, &self.config.layout, &key)?;
                match fs::remove_file(path) {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(_) => Err(KeyValueError::Unavailable),
                }
            }
        }
    }
}

pub fn encoded_path_for_test(
    root: &Path,
    layout: &FileLayout,
    key: &[u8],
) -> Result<PathBuf, KeyValueError> {
    key_path(root, layout, key)
}
