
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};
use std::fs;
use crate::types::HASH;

/// Content-addressed object store using filesystem with two-byte fanout
pub struct ObjectStore {
    base_path: PathBuf,
}

impl ObjectStore {
    /// Create a new object store at the specified base path
    pub fn new<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path)?;
        
        Ok(Self { base_path })
    }
    
    /// Store an object with the given hash
    /// Verifies that the blake3 hash of the bytes matches the provided hash
    pub fn put(&self, hash: HASH, bytes: Vec<u8>) -> Result<()> {
        // Verify hash matches content
        let computed_hash = blake3::hash(&bytes);
        if computed_hash.as_bytes() != &hash {
            bail!("Hash mismatch: provided hash does not match content");
        }
        
        let path = self.object_path(&hash);
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Write atomically using a temporary file
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, &bytes)?;
        fs::rename(&temp_path, &path)?;
        
        Ok(())
    }
    
    /// Retrieve an object by its hash
    pub fn get(&self, hash: &HASH) -> Result<Option<Vec<u8>>> {
        let path = self.object_path(hash);
        
        if !path.exists() {
            return Ok(None);
        }
        
        let bytes = fs::read(&path)?;
        
        // Verify integrity on read
        let computed_hash = blake3::hash(&bytes);
        if computed_hash.as_bytes() != hash {
            bail!("Integrity check failed: stored object hash mismatch");
        }
        
        Ok(Some(bytes))
    }
    
    /// Check if an object exists
    pub fn has(&self, hash: &HASH) -> bool {
        self.object_path(hash).exists()
    }
    
    /// Delete an object by its hash
    pub fn delete(&self, hash: &HASH) -> Result<bool> {
        let path = self.object_path(hash);
        
        if !path.exists() {
            return Ok(false);
        }
        
        fs::remove_file(&path)?;
        Ok(true)
    }
    
    /// List all object hashes in the store
    pub fn list_all(&self) -> Result<Vec<HASH>> {
        let mut hashes = Vec::new();
        
        // Iterate through fanout directories
        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let fanout_path = entry.path();
            
            if !fanout_path.is_dir() {
                continue;
            }
            
            // Iterate through objects in fanout directory
            for obj_entry in fs::read_dir(&fanout_path)? {
                let obj_entry = obj_entry?;
                let file_name = obj_entry.file_name();
                let file_str = file_name.to_string_lossy();
                
                // Parse hash from filename
                if let Some(fanout_prefix) = fanout_path.file_name() {
                    let full_hash_str = format!("{}{}", fanout_prefix.to_string_lossy(), file_str);
                    if let Ok(hash_bytes) = hex::decode(&full_hash_str) {
                        if hash_bytes.len() == 32 {
                            let mut hash = [0u8; 32];
                            hash.copy_from_slice(&hash_bytes);
                            hashes.push(hash);
                        }
                    }
                }
            }
        }
        
        Ok(hashes)
    }
    
    /// Alias for has() - check if object exists
    pub fn has_object(&self, hash: &HASH) -> bool {
        self.has(hash)
    }
    
    /// Alias for put() - store object
    pub fn store_object(&self, hash: &HASH, bytes: &[u8]) -> Result<()> {
        self.put(*hash, bytes.to_vec())
    }
    
    /// Alias for get() - retrieve object
    pub fn get_object(&self, hash: &HASH) -> Result<Option<Vec<u8>>> {
        self.get(hash)
    }
    
    /// Get the filesystem path for an object with the given hash
    /// Uses two-byte fanout: objects/ab/cd1234...
    fn object_path(&self, hash: &HASH) -> PathBuf {
        let hash_hex = hex::encode(hash);
        let fanout = &hash_hex[..2];
        let remainder = &hash_hex[2..];
        
        self.base_path.join(fanout).join(remainder)
    }
}

// Implement the common interface trait
impl crate::storage::ObjectStoreInterface for ObjectStore {
    fn list_all(&self) -> Result<Vec<crate::types::HASH>> {
        self.list_all()
    }
    
    fn get_object(&self, hash: &crate::types::HASH) -> Result<Option<Vec<u8>>> {
        self.get(hash)
    }
    
    fn store_object(&self, hash: &crate::types::HASH, bytes: &[u8]) -> Result<()> {
        self.put(*hash, bytes.to_vec())
    }
    
    fn has_object(&self, hash: &crate::types::HASH) -> bool {
        self.has(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_put_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let store = ObjectStore::new(temp_dir.path()).unwrap();
        
        let data = b"test data";
        let hash = *blake3::hash(data).as_bytes();
        
        store.put(hash, data.to_vec()).unwrap();
        let retrieved = store.get(&hash).unwrap();
        
        assert_eq!(retrieved, Some(data.to_vec()));
    }
    
    #[test]
    fn test_hash_verification() {
        let temp_dir = TempDir::new().unwrap();
        let store = ObjectStore::new(temp_dir.path()).unwrap();
        
        let data = b"test data";
        let wrong_hash = [0u8; 32];
        
        let result = store.put(wrong_hash, data.to_vec());
        assert!(result.is_err());
    }
    
    #[test]
    fn test_has() {
        let temp_dir = TempDir::new().unwrap();
        let store = ObjectStore::new(temp_dir.path()).unwrap();
        
        let data = b"test data";
        let hash = *blake3::hash(data).as_bytes();
        
        assert!(!store.has(&hash));
        store.put(hash, data.to_vec()).unwrap();
        assert!(store.has(&hash));
    }
}


