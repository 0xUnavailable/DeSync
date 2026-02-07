use std::fs;
use crate::types::HASH;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub struct ObjectStore {
    base_path: PathBuf,
}

impl ObjectStore {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path)?;

        Ok(Self { base_path })
    }

    pub fn put(&self, hash: HASH, bytes: Vec<u8>) -> Result<()> {
        let computed_hash = blake3::hash(&bytes);
        if computed_hash.as_bytes() != &hash {
            bail!("Hash mismatch: provided hash does not match content");
        }

        let path = self.object_path(&hash);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, &bytes)?;
        fs::rename(&temp_path, &path)?;

        Ok(())
    }

    pub fn get(&self, hash: &HASH) -> Result<Option<Vec<u8>>> {
        let path = self.object_path(hash);

        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(&path)?;

        let computed_hash = blake3::hash(&bytes);
        if computed_hash.as_bytes() != hash {
            bail!("Integrity check failed: stored object hash mismatch");
        }

        Ok(Some(bytes))
    }

    pub fn has(&self, hash: &HASH) -> bool {
        self.object_path(hash).exists()
    }

    pub fn delete(&self, hash: &HASH) -> Result<bool> {
        let path = self.object_path(hash);

        if !path.exists() {
            return Ok(false);
        }

        fs::remove_file(&path)?;
        Ok(true)
    }

    pub fn list_all(&self) -> Result<Vec<HASH>> {
        let mut hashes = Vec::new();

        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let fanout_path = entry.path();

            if !fanout_path.is_dir() {
                continue;
            }

            for obj_entry in fs::read_dir(&fanout_path)? {
                let obj_entry = obj_entry?;
                let file_name = obj_entry.file_name();
                let file_str = file_name.to_string_lossy();

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

    pub fn has_object(&self, hash: &HASH) -> bool {
        self.has(hash)
    }

    pub fn store_object(&self, hash: &HASH, bytes: &[u8]) -> Result<()> {
        self.put(*hash, bytes.to_vec())
    }

    pub fn get_object(&self, hash: &HASH) -> Result<Option<Vec<u8>>> {
        self.get(hash)
    }

    fn object_path(&self, hash: &HASH) -> PathBuf {
        let hash_hex = hex::encode(hash);
        let fanout = &hash_hex[..2];
        let remainder = &hash_hex[2..];

        self.base_path.join(fanout).join(remainder)
    }
}

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
