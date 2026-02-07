use std::fs;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use crate::types::{RealmObject, HASH};

#[derive(Debug, Clone, Copy)]
pub enum ObjectType {
    GovernmentLog,
    TaxObligation,
    TaxReceipt,
    P2PReceipt,
}

impl ObjectType {
    pub fn folder_name(&self) -> &'static str {
        match self {
            ObjectType::GovernmentLog => "00_government_logs",
            ObjectType::TaxObligation => "01_tax_obligations",
            ObjectType::TaxReceipt => "02_tax_receipts",
            ObjectType::P2PReceipt => "03_p2p_receipts",
        }
    }

    pub fn from_object(obj: &RealmObject) -> Self {
        match obj {
            RealmObject::GovernmentRuleLog(_) => ObjectType::GovernmentLog,
            RealmObject::TaxObligationEnvelope(_) => ObjectType::TaxObligation,
            RealmObject::TaxReceiptEnvelope(_) => ObjectType::TaxReceipt,
            RealmObject::P2PReceiptEnvelope(_) => ObjectType::P2PReceipt,
        }
    }
}

pub struct OrganizedObjectStore {
    base_path: PathBuf,
    is_government: bool,
}

impl OrganizedObjectStore {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        Self::with_config(base_path, false)
    }

    pub fn new_government<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        Self::with_config(base_path, true)
    }

    fn with_config<P: AsRef<Path>>(base_path: P, is_government: bool) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path)?;

        let mut folders = vec![
            ObjectType::GovernmentLog,
            ObjectType::TaxObligation,
            ObjectType::TaxReceipt,
        ];

        if !is_government {
            folders.push(ObjectType::P2PReceipt);
        }

        for obj_type in folders {
            fs::create_dir_all(base_path.join(obj_type.folder_name()))?;
        }

        Ok(Self {
            base_path,
            is_government,
        })
    }

    pub fn put(&self, hash: HASH, bytes: Vec<u8>) -> Result<()> {
        let computed_hash = blake3::hash(&bytes);
        if computed_hash.as_bytes() != &hash {
            bail!("Hash mismatch: provided hash does not match content");
        }

        let obj_type = match serde_json::from_slice::<RealmObject>(&bytes) {
            Ok(obj) => ObjectType::from_object(&obj),
            Err(_) => bail!("Cannot store unknown object type - must be a valid RealmObject"),
        };

        let path = self.object_path(&hash, obj_type);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, &bytes)?;
        fs::rename(&temp_path, &path)?;

        Ok(())
    }

    pub fn get(&self, hash: &HASH) -> Result<Option<Vec<u8>>> {
        let folders = if self.is_government {
            vec![
                ObjectType::GovernmentLog,
                ObjectType::TaxObligation,
                ObjectType::TaxReceipt,
            ]
        } else {
            vec![
                ObjectType::GovernmentLog,
                ObjectType::TaxObligation,
                ObjectType::TaxReceipt,
                ObjectType::P2PReceipt,
            ]
        };

        for obj_type in folders {
            let path = self.object_path(hash, obj_type);

            if path.exists() {
                let bytes = fs::read(&path)?;

                let computed_hash = blake3::hash(&bytes);
                if computed_hash.as_bytes() != hash {
                    bail!("Integrity check failed: stored object hash mismatch");
                }

                return Ok(Some(bytes));
            }
        }

        Ok(None)
    }

    pub fn has(&self, hash: &HASH) -> bool {
        for obj_type in [
            ObjectType::GovernmentLog,
            ObjectType::TaxObligation,
            ObjectType::TaxReceipt,
            ObjectType::P2PReceipt,
        ] {
            if self.object_path(hash, obj_type).exists() {
                return true;
            }
        }
        false
    }

    pub fn delete(&self, hash: &HASH) -> Result<bool> {
        for obj_type in [
            ObjectType::GovernmentLog,
            ObjectType::TaxObligation,
            ObjectType::TaxReceipt,
            ObjectType::P2PReceipt,
        ] {
            let path = self.object_path(hash, obj_type);

            if path.exists() {
                fs::remove_file(&path)?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn list_by_type(&self, obj_type: ObjectType) -> Result<Vec<HASH>> {
        let mut hashes = Vec::new();
        let folder = self.base_path.join(obj_type.folder_name());

        if !folder.exists() {
            return Ok(hashes);
        }

        for entry in fs::read_dir(&folder)? {
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

    pub fn list_all(&self) -> Result<Vec<HASH>> {
        let mut all_hashes = Vec::new();

        for obj_type in [
            ObjectType::GovernmentLog,
            ObjectType::TaxObligation,
            ObjectType::TaxReceipt,
            ObjectType::P2PReceipt,
        ] {
            all_hashes.extend(self.list_by_type(obj_type)?);
        }

        Ok(all_hashes)
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

    pub fn get_stats(&self) -> Result<ObjectStoreStats> {
        Ok(ObjectStoreStats {
            government_logs: self.list_by_type(ObjectType::GovernmentLog)?.len(),
            tax_obligations: self.list_by_type(ObjectType::TaxObligation)?.len(),
            tax_receipts: self.list_by_type(ObjectType::TaxReceipt)?.len(),
            p2p_receipts: self.list_by_type(ObjectType::P2PReceipt)?.len(),
        })
    }

    fn object_path(&self, hash: &HASH, obj_type: ObjectType) -> PathBuf {
        let hash_hex = hex::encode(hash);
        let fanout = &hash_hex[..2];
        let remainder = &hash_hex[2..];

        self.base_path
            .join(obj_type.folder_name())
            .join(fanout)
            .join(remainder)
    }
}

pub struct ObjectStoreStats {
    pub government_logs: usize,
    pub tax_obligations: usize,
    pub tax_receipts: usize,
    pub p2p_receipts: usize,
}

impl ObjectStoreStats {
    pub fn total(&self) -> usize {
        self.government_logs + self.tax_obligations + self.tax_receipts + self.p2p_receipts
    }

    pub fn display(&self) -> String {
        format!(
            "Object Store Stats:\n\
             Government Logs: {}\n\
             Tax Obligations: {}\n\
             Tax Receipts: {}\n\
             P2P Receipts: {}\n\
             Total: {}",
            self.government_logs,
            self.tax_obligations,
            self.tax_receipts,
            self.p2p_receipts,
            self.total()
        )
    }
}

impl crate::storage::ObjectStoreInterface for OrganizedObjectStore {
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
