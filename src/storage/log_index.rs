use std::collections::HashMap;
use anyhow::Result;
use serde::{Serialize, Deserialize};
use crate::types::{HASH, RealmObject};

/// Index mapping version numbers to log hashes for efficient retrieval
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LogVersionIndex {
    /// Map: version_number -> log_hash
    version_to_hash: HashMap<u64, HASH>,
    /// Highest version in the index
    max_version: u64,
}

impl LogVersionIndex {
    /// Create a new empty index
    pub fn new() -> Self {
        Self {
            version_to_hash: HashMap::new(),
            max_version: 0,
        }
    }
    
    /// Add a log to the index
    pub fn index_log(&mut self, version: u64, hash: HASH) {
        self.version_to_hash.insert(version, hash);
        if version > self.max_version {
            self.max_version = version;
        }
    }
    
    /// Get a single log hash by version
    pub fn get(&self, version: u64) -> Option<HASH> {
        self.version_to_hash.get(&version).copied()
    }
    
    /// Get log hashes for a version range (inclusive)
    pub fn get_range(&self, start: u64, end: u64) -> Vec<HASH> {
        (start..=end)
            .filter_map(|v| self.version_to_hash.get(&v).copied())
            .collect()
    }
    
    /// Get the maximum version in the index
    pub fn max_version(&self) -> u64 {
        self.max_version
    }
    
    /// Check if a version exists in the index
    pub fn has_version(&self, version: u64) -> bool {
        self.version_to_hash.contains_key(&version)
    }
    
    /// Get the number of indexed logs
    pub fn len(&self) -> usize {
        self.version_to_hash.len()
    }
    
    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.version_to_hash.is_empty()
    }
    
    /// Build index from a collection of objects
    pub fn from_objects(objects: &[RealmObject]) -> Self {
        let mut index = Self::new();
        
        for obj in objects {
            if let RealmObject::GovernmentRuleLog(log) = obj {
                index.index_log(log.version_number, log.log_hash);
            }
        }
        
        index
    }
    
    /// Get all versions in sorted order
    pub fn all_versions(&self) -> Vec<u64> {
        let mut versions: Vec<u64> = self.version_to_hash.keys().copied().collect();
        versions.sort_unstable();
        versions
    }
}

/// Helper to build index from object store (works with both ObjectStore and OrganizedObjectStore)
pub fn build_index_from_store<T>(object_store: &T) -> Result<LogVersionIndex> 
where
    T: crate::storage::ObjectStoreInterface,
{
    let mut index = LogVersionIndex::new();
    
    // Get all object hashes
    let all_hashes = object_store.list_all()?;
    
    // Scan for government logs and index them
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_index_operations() {
        let mut index = LogVersionIndex::new();
        
        let hash1 = [1u8; 32];
        let hash2 = [2u8; 32];
        let hash3 = [3u8; 32];
        
        index.index_log(1, hash1);
        index.index_log(2, hash2);
        index.index_log(5, hash3);
        
        assert_eq!(index.get(1), Some(hash1));
        assert_eq!(index.get(2), Some(hash2));
        assert_eq!(index.get(3), None);
        assert_eq!(index.get(5), Some(hash3));
        
        assert_eq!(index.max_version(), 5);
        assert_eq!(index.len(), 3);
    }
    
    #[test]
    fn test_range_query() {
        let mut index = LogVersionIndex::new();
        
        let hash1 = [1u8; 32];
        let hash2 = [2u8; 32];
        let hash3 = [3u8; 32];
        
        index.index_log(1, hash1);
        index.index_log(2, hash2);
        index.index_log(3, hash3);
        
        let range = index.get_range(1, 3);
        assert_eq!(range.len(), 3);
        assert_eq!(range[0], hash1);
        assert_eq!(range[1], hash2);
        assert_eq!(range[2], hash3);
        
        // Range with gaps
        index.index_log(10, [10u8; 32]);
        let range = index.get_range(1, 10);
        assert_eq!(range.len(), 4); // Only returns existing versions
    }
}