use anyhow::Result;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::types::{RealmObject, HASH};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LogVersionIndex {
    version_to_hash: HashMap<u64, HASH>,
    max_version: u64,
}

impl LogVersionIndex {
    pub fn new() -> Self {
        Self {
            version_to_hash: HashMap::new(),
            max_version: 0,
        }
    }

    pub fn index_log(&mut self, version: u64, hash: HASH) {
        self.version_to_hash.insert(version, hash);
        if version > self.max_version {
            self.max_version = version;
        }
    }

    pub fn get(&self, version: u64) -> Option<HASH> {
        self.version_to_hash.get(&version).copied()
    }

    pub fn get_range(&self, start: u64, end: u64) -> Vec<HASH> {
        (start..=end)
            .filter_map(|v| self.version_to_hash.get(&v).copied())
            .collect()
    }

    pub fn max_version(&self) -> u64 {
        self.max_version
    }

    pub fn has_version(&self, version: u64) -> bool {
        self.version_to_hash.contains_key(&version)
    }

    pub fn len(&self) -> usize {
        self.version_to_hash.len()
    }

    pub fn is_empty(&self) -> bool {
        self.version_to_hash.is_empty()
    }

    pub fn from_objects(objects: &[RealmObject]) -> Self {
        let mut index = Self::new();

        for obj in objects {
            if let RealmObject::GovernmentRuleLog(log) = obj {
                index.index_log(log.version_number, log.log_hash);
            }
        }

        index
    }

    pub fn all_versions(&self) -> Vec<u64> {
        let mut versions: Vec<u64> = self.version_to_hash.keys().copied().collect();
        versions.sort_unstable();
        versions
    }
}

pub fn build_index_from_store<T>(object_store: &T) -> Result<LogVersionIndex>
where
    T: crate::storage::ObjectStoreInterface,
{
    let mut index = LogVersionIndex::new();

    let all_hashes = object_store.list_all()?;

    for hash in all_hashes {
        if let Some(bytes) = object_store.get_object(&hash)? {
            if let Ok(obj) = serde_json::from_slice::<RealmObject>(&bytes) {
                if let RealmObject::GovernmentRuleLog(log) = obj {
                    index.index_log(log.version_number, hash);
                }
            }
        }
    }

    Ok(index)
}

