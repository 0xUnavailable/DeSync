/// File protection module to ensure state files can only be modified through the protocol
/// This prevents users from manually editing critical state files

use anyhow::{Result, bail};
use std::fs::{self, File, OpenOptions};
use std::io::{Write, Read};
use std::path::Path;
use serde::{Serialize, Deserialize};
use blake3;

const CHECKSUM_SUFFIX: &str = ".checksum";

/// Wrapper for protected file operations
pub struct ProtectedFile {
    path: String,
}

impl ProtectedFile {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_string_lossy().to_string(),
        }
    }
    
    /// Write data to a protected file with checksum
    pub fn write<T: Serialize>(&self, data: &T) -> Result<()> {
        // Serialize the data
        let json_bytes = serde_json::to_vec_pretty(data)?;
        
        // Compute checksum
        let checksum = blake3::hash(&json_bytes);
        let checksum_hex = hex::encode(checksum.as_bytes());
        
        // Write data file
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)?;
        file.write_all(&json_bytes)?;
        file.sync_all()?;
        
        // Write checksum file
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
    
    /// Read and verify a protected file
    pub fn read<T: for<'de> Deserialize<'de>>(&self) -> Result<T> {
        // Read data file
        let mut file = File::open(&self.path)?;
        let mut json_bytes = Vec::new();
        file.read_to_end(&mut json_bytes)?;
        
        // Read checksum file
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
        
        // Verify checksum
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
        
        // Deserialize if checksum is valid
        let data: T = serde_json::from_slice(&json_bytes)?;
        
        println!("[PROTECTION] Verified protected file: {}", self.path);
        
        Ok(data)
    }
    
    /// Check if a protected file exists
    pub fn exists(&self) -> bool {
        Path::new(&self.path).exists()
    }
    
    /// Append to a protected append-only log (like node-events.jsonl)
    pub fn append_line(&self, line: &str) -> Result<()> {
        // Read current content
        let current_content = if Path::new(&self.path).exists() {
            fs::read(&self.path)?
        } else {
            Vec::new()
        };
        
        // Append new line
        let mut new_content = current_content.clone();
        new_content.extend_from_slice(line.as_bytes());
        new_content.push(b'\n');
        
        // Compute new checksum
        let checksum = blake3::hash(&new_content);
        let checksum_hex = hex::encode(checksum.as_bytes());
        
        // Write updated file
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)?;
        file.write_all(&new_content)?;
        file.sync_all()?;
        
        // Write checksum file
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
    
    /// Read and verify an append-only log file
    pub fn read_lines(&self) -> Result<Vec<String>> {
        // Read the file
        let mut file = File::open(&self.path)?;
        let mut content = Vec::new();
        file.read_to_end(&mut content)?;
        
        // Verify checksum
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
        
        // Parse lines
        let content_str = String::from_utf8(content)?;
        let lines: Vec<String> = content_str
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();
        
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::fs;
    
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestData {
        value: i32,
    }
    
    #[test]
    fn test_protected_write_and_read() {
        let test_path = "/tmp/test_protected.json";
        let protected = ProtectedFile::new(test_path);
        
        let data = TestData { value: 42 };
        protected.write(&data).unwrap();
        
        let read_data: TestData = protected.read().unwrap();
        assert_eq!(data, read_data);
        
        // Cleanup
        let _ = fs::remove_file(test_path);
        let _ = fs::remove_file(format!("{}{}", test_path, CHECKSUM_SUFFIX));
    }
    
    #[test]
    fn test_tampering_detection() {
        let test_path = "/tmp/test_tampered.json";
        let protected = ProtectedFile::new(test_path);
        
        let data = TestData { value: 42 };
        protected.write(&data).unwrap();
        
        // Tamper with the file
        let tampered = TestData { value: 999 };
        let tampered_json = serde_json::to_vec_pretty(&tampered).unwrap();
        fs::write(test_path, tampered_json).unwrap();
        
        // This should fail
        let result: Result<TestData> = protected.read();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("INTEGRITY ERROR"));
        
        // Cleanup
        let _ = fs::remove_file(test_path);
        let _ = fs::remove_file(format!("{}{}", test_path, CHECKSUM_SUFFIX));
    }
}
