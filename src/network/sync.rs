use anyhow::Result;

use crate::types::{HASH, RealmObject};
use crate::network::protocol::NodeHead;
use crate::storage::OrganizedObjectStore;

/// Reconciliation result - objects that need to be requested
#[derive(Debug, Clone)]
pub struct ReconciliationPlan {
    pub missing_gov_logs: Vec<HASH>,
    pub missing_receipts: Vec<HASH>,
}

impl ReconciliationPlan {
    pub fn new() -> Self {
        Self {
            missing_gov_logs: Vec::new(),
            missing_receipts: Vec::new(),
        }
    }
    
    pub fn is_empty(&self) -> bool {
        self.missing_gov_logs.is_empty() && self.missing_receipts.is_empty()
    }
    
    pub fn all_hashes(&self) -> Vec<HASH> {
        let mut result = Vec::new();
        result.extend_from_slice(&self.missing_gov_logs);
        result.extend_from_slice(&self.missing_receipts);
        result
    }
}

/// Compare local and peer heads to determine what objects to fetch
/// This is used for backwards-compatible object-based sync
pub fn reconcile_heads(
    local: &NodeHead,
    peer: &NodeHead,
    object_store: &OrganizedObjectStore,
) -> ReconciliationPlan {
    let mut plan = ReconciliationPlan::new();
    
    // Check if peer has newer government log OR if we don't have any log yet
   let local_has_no_logs = object_store.list_all().map(|v| v.is_empty()).unwrap_or(true);
if (peer.gov_log_version == 0 && local_has_no_logs)
    || peer.gov_log_version > local.gov_log_version
{
    if peer.gov_log_hash != [0u8; 32]
        && !object_store.has_object(&peer.gov_log_hash)
    {
        plan.missing_gov_logs.push(peer.gov_log_hash);
    }
}

    
    // Check receipts head
    if let Some(peer_receipts_head) = peer.receipts_head {
        if !object_store.has_object(&peer_receipts_head) {
            plan.missing_receipts.push(peer_receipts_head);
        }
    }
    
    plan
}

/// Ingest and validate a received object
/// Returns Ok(hash) if object is valid and stored, Err if invalid
pub fn ingest_object(
    object_bytes: &[u8],
    gov_key: &ed25519_dalek::VerifyingKey,
    object_store: &OrganizedObjectStore,
) -> Result<HASH> {
    // Deserialize object
    let object: RealmObject = serde_json::from_slice(object_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to deserialize object: {}", e))?;
    
    // Verify hash
    let computed_hash = object.compute_hash();
    let actual_hash: HASH = *blake3::hash(object_bytes).as_bytes();
    
    if computed_hash != actual_hash {
        anyhow::bail!("Hash mismatch");
    }
    
    // Verify signature based on object type
    if !object.verify_signature(gov_key) {
        anyhow::bail!("Signature verification failed");
    }
    
    // Check if we already have this object
    if object_store.has_object(&computed_hash) {
        return Ok(computed_hash); // Already have it, skip
    }
    
    // Store object
    object_store.store_object(&computed_hash, object_bytes)?;
    
    Ok(computed_hash)
}

/// Build a logs chunk response for a log request
/// Used by government nodes to serve delta sync requests
pub fn build_logs_chunk(
    start_version: u64,
    max_entries: usize,
    object_store: &OrganizedObjectStore,
    log_index: &crate::storage::LogVersionIndex,
) -> Result<Vec<RealmObject>> {
    let end_version = start_version.saturating_add(max_entries as u64).saturating_sub(1);
    
    log::debug!(
        "[SYNC] Building logs chunk from version {} to {} (max {} entries)",
        start_version,
        end_version,
        max_entries
    );
    
    // Get hashes for the requested range
    let hashes = log_index.get_range(start_version, end_version);
    
    if hashes.is_empty() {
        log::debug!("[SYNC] No logs found in range {}-{}", start_version, end_version);
        return Ok(Vec::new());
    }
    
    let mut logs = Vec::new();
    
    // Retrieve each log from storage
    for hash in hashes {
        match object_store.get_object(&hash)? {
            Some(bytes) => {
                match serde_json::from_slice::<RealmObject>(&bytes) {
                    Ok(obj) => {
                        // Only include government logs in the chunk
                        if matches!(obj, RealmObject::GovernmentRuleLog(_)) {
                            logs.push(obj);
                        }
                    }
                    Err(e) => {
                        log::warn!("[SYNC] Failed to deserialize object {}: {}", hex::encode(hash), e);
                        continue;
                    }
                }
            }
            None => {
                log::warn!("[SYNC] Object {} not found in store", hex::encode(hash));
                continue;
            }
        }
    }
    
    log::debug!("[SYNC] Built logs chunk with {} entries", logs.len());
    
    Ok(logs)
}

/// Validate that a new log correctly chains to the previous log
pub fn validate_log_chain(
    new_log: &crate::types::GovernmentRuleLog,
    previous_log: &crate::types::GovernmentRuleLog,
) -> Result<()> {
    // Check version is sequential
    if new_log.version_number != previous_log.version_number + 1 {
        anyhow::bail!(
            "Non-sequential version: expected {}, got {}",
            previous_log.version_number + 1,
            new_log.version_number
        );
    }
    
    // Check previous_log_hash matches
    if new_log.previous_log_hash != previous_log.log_hash {
        anyhow::bail!(
            "Invalid previous_log_hash: expected {}, got {}",
            hex::encode(previous_log.log_hash),
            hex::encode(new_log.previous_log_hash)
        );
    }
    
    // Check realm_id matches
    if new_log.realm_id != previous_log.realm_id {
        anyhow::bail!("Realm ID mismatch in log chain");
    }
    
    Ok(())
}