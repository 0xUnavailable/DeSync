use blake3;
use std::path::Path;
use std::io::{Read, Write};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};


const CHECKSUM_SUFFIX: &str = ".checksum";

pub struct ProtectedFile {
    path: String,
}

impl ProtectedFile {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_string_lossy().to_string(),
        }
    }

    pub fn write<T: Serialize>(&self, data: &T) -> Result<()> {
        let json_bytes = serde_json::to_vec_pretty(data)?;

        let checksum = blake3::hash(&json_bytes);
        let checksum_hex = hex::encode(checksum.as_bytes());

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)?;
        file.write_all(&json_bytes)?;
        file.sync_all()?;

        let checksum_path = format!("{}{}", self.path, CHECKSUM_SUFFIX);
        let mut checksum_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&checksum_path)?;
        checksum_file.write_all(checksum_hex.as_bytes())?;
        checksum_file.sync_all()?;

        println!("[PROTECTION] Saved protected file: {}", self.path);
        println!("  Checksum: {}", checksum_hex);

        Ok(())
    }

    pub fn read<T: for<'de> Deserialize<'de>>(&self) -> Result<T> {
        let mut file = File::open(&self.path)?;
        let mut json_bytes = Vec::new();
        file.read_to_end(&mut json_bytes)?;

        let checksum_path = format!("{}{}", self.path, CHECKSUM_SUFFIX);
        if !Path::new(&checksum_path).exists() {
            bail!(
                "INTEGRITY ERROR: Checksum file missing for {}. \
                This file may have been tampered with. \
                Please restore from backup or reinitialize.",
                self.path
            );
        }

        let mut checksum_file = File::open(&checksum_path)?;
        let mut stored_checksum = String::new();
        checksum_file.read_to_string(&mut stored_checksum)?;

        let computed_checksum = blake3::hash(&json_bytes);
        let computed_checksum_hex = hex::encode(computed_checksum.as_bytes());

        if stored_checksum.trim() != computed_checksum_hex {
            bail!(
                "INTEGRITY ERROR: File {} has been modified outside the protocol!\n\
                Expected checksum: {}\n\
                Actual checksum:   {}\n\
                \n\
                This is a critical security violation. The file may have been tampered with.\n\
                Please restore from backup or reinitialize your node.",
                self.path,
                stored_checksum.trim(),
                computed_checksum_hex
            );
        }

        let data: T = serde_json::from_slice(&json_bytes)?;

        println!("[PROTECTION] Verified protected file: {}", self.path);

        Ok(data)
    }

    pub fn exists(&self) -> bool {
        Path::new(&self.path).exists()
    }

    pub fn append_line(&self, line: &str) -> Result<()> {
        let current_content = if Path::new(&self.path).exists() {
            fs::read(&self.path)?
        } else {
            Vec::new()
        };

        let mut new_content = current_content.clone();
        new_content.extend_from_slice(line.as_bytes());
        new_content.push(b'\n');

        let checksum = blake3::hash(&new_content);
        let checksum_hex = hex::encode(checksum.as_bytes());

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)?;
        file.write_all(&new_content)?;
        file.sync_all()?;

        let checksum_path = format!("{}{}", self.path, CHECKSUM_SUFFIX);
        let mut checksum_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&checksum_path)?;
        checksum_file.write_all(checksum_hex.as_bytes())?;
        checksum_file.sync_all()?;

        Ok(())
    }

    pub fn read_lines(&self) -> Result<Vec<String>> {
        let mut file = File::open(&self.path)?;
        let mut content = Vec::new();
        file.read_to_end(&mut content)?;

        let checksum_path = format!("{}{}", self.path, CHECKSUM_SUFFIX);
        if !Path::new(&checksum_path).exists() {
            bail!(
                "INTEGRITY ERROR: Checksum file missing for {}. \
                This file may have been tampered with.",
                self.path
            );
        }

        let mut checksum_file = File::open(&checksum_path)?;
        let mut stored_checksum = String::new();
        checksum_file.read_to_string(&mut stored_checksum)?;

        let computed_checksum = blake3::hash(&content);
        let computed_checksum_hex = hex::encode(computed_checksum.as_bytes());

        if stored_checksum.trim() != computed_checksum_hex {
            bail!(
                "INTEGRITY ERROR: Log file {} has been modified outside the protocol!\n\
                Expected checksum: {}\n\
                Actual checksum:   {}",
                self.path,
                stored_checksum.trim(),
                computed_checksum_hex
            );
        }

        let content_str = String::from_utf8(content)?;
        let lines: Vec<String> = content_str
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();

        Ok(lines)
    }
}
