use crate::{types::{ActionRule, GovernmentRuleLog, HASH, RealmId, Task}, utils::serialize};
use ed25519_dalek::{SigningKey, Verifier, VerifyingKey, Signature, Signer};
use std::collections::BTreeMap;
use anyhow::{Result, anyhow};

#[derive(Debug)]
pub struct GovernmentNode {
    pub realm_id: RealmId,
    pub signing_key: SigningKey,
    pub latest_log: Option<GovernmentRuleLog>,
    pub previous_log: Option<GovernmentRuleLog>
}

impl GovernmentNode {
    pub fn new(realm_id: RealmId, signing_key: SigningKey) -> Self {
        Self {
            realm_id,
            signing_key,
            latest_log: None,
            previous_log: None,
        }
    }

pub fn generate_genesis_government_log(
    realm_id: RealmId,
    signing_key: &SigningKey,
) -> Result<GovernmentRuleLog> {
    let public_key = signing_key.verifying_key();

    let mut log = GovernmentRuleLog {
        realm_id,
        version_number: 0,
        effective_slot: 0,
        rules: BTreeMap::new(),
        tasks: Vec::new(),
        tax_rate_percent: 0,
        tax_deadline: 0,
        previous_log_hash: None,
        log_hash: [0; 32],
        government_signature: [0; 64],
        government_public_key: public_key,
    };

    let serialized = serialize(&log)?;
    let hash = blake3::hash(&serialized);
    log.log_hash = *hash.as_bytes();
    log.government_signature = signing_key.sign(hash.as_bytes()).to_bytes();

    Ok(log)
}


pub fn append_new_rule_log(
    &mut self,
    rules: BTreeMap<String, ActionRule>,
    tasks: Vec<Task>,
    tax_rate_percent: u8,
    tax_deadline: u64,
    effective_slot: u64,
) -> Result<GovernmentRuleLog> {
    let version_number = self.latest_log
        .as_ref()
        .map(|log| log.version_number + 1)
        .unwrap_or(1);

    let previous_log_hash = self.latest_log.as_ref().map(|log| log.log_hash);

    let government_public_key = self.signing_key.verifying_key();

    let mut new_log = GovernmentRuleLog {
        version_number,
        effective_slot,
        rules,
        tasks,
        tax_rate_percent,
        tax_deadline,
        previous_log_hash,
        log_hash: [0; 32],
        government_signature: [0; 64],
        government_public_key,
        realm_id: self.realm_id,
    };

    let serialized_log = serialize(&new_log)?;
    let hash_output = blake3::hash(&serialized_log);
    let log_hash: HASH = *hash_output.as_bytes();

    let signature: Signature = self.signing_key.sign(&log_hash);

    new_log.log_hash = log_hash;
    new_log.government_signature = signature.to_bytes();

    self.previous_log = self.latest_log.clone();
    self.latest_log = Some(new_log.clone());

    Ok(new_log)
}
    
    pub fn verify_rule_log(&self, log: &GovernmentRuleLog, pubkey_bytes: &[u8]) -> Result<bool> {
        let mut log_for_hash = log.clone();
        log_for_hash.log_hash = [0; 32];
        log_for_hash.government_signature = [0; 64];

        let serialized_log = serialize(&log_for_hash)?;
        let expected_hash = blake3::hash(&serialized_log);

        if expected_hash.as_bytes() != &log.log_hash {
            println!("Verification Failed: Data hash mismatch.");
            return Ok(false);
        }
        
        let public_key = VerifyingKey::from_bytes(pubkey_bytes.try_into()?)
            .map_err(|_| anyhow!("Invalid Public Key format"))?;
        let sig = Signature::from_bytes(&log.government_signature);
        
        match public_key.verify(&log.log_hash, &sig) {
            Ok(_) => Ok(true),
            Err(_) => {
                println!("Verification Failed: Invalid Government Signature.");
                Ok(false)
            }
        }
    }
    
    pub fn verify_log_chain(&self, log: &GovernmentRuleLog) -> Result<bool> {
        if let Some(prev_hash) = &log.previous_log_hash {
            if let Some(prev_log) = &self.previous_log {
                if &prev_log.log_hash != prev_hash {
                    println!("Verification Failed: Log chain broken - previous hash mismatch.");
                    return Ok(false);
                }
            } else {
                println!("Verification Failed: Log references previous hash but no previous log exists.");
                return Ok(false);
            }
        }
        
        if let Some(prev_log) = &self.previous_log {
            if log.version_number != prev_log.version_number + 1 {
                println!("Verification Failed: Version number not sequential.");
                return Ok(false);
            }
        } else if log.version_number != 1 {
            println!("Verification Failed: Genesis log must have version 1.");
            return Ok(false);
        }
        
        Ok(true)
    }

    pub fn latest(&self) -> Option<GovernmentRuleLog> {
        self.latest_log.clone()
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    fn setup_gov_node() -> (GovernmentNode, SigningKey) {
        let signing_key = SigningKey::generate(&mut OsRng);
        (GovernmentNode::new([0;32],signing_key.clone()), signing_key)
    }

    #[test]
    fn test_latest_log_retrieval() {
        let (mut gov_node, _) = setup_gov_node();

        assert!(gov_node.latest().is_none());

        let rules = BTreeMap::new();
        gov_node.append_new_rule_log(rules, vec![], 10, 1000, 1)
            .expect("Failed to append log");

        let latest = gov_node.latest().expect("Log should exist");
        assert_eq!(latest.version_number, 1);
    }

    #[test]
    fn test_verify_valid_log() {
        let (mut gov_node, signing_key) = setup_gov_node();
        let pubkey_bytes = signing_key.verifying_key().to_bytes();

        let log = gov_node
            .append_new_rule_log(BTreeMap::new(), vec![], 5, 500, 1)
            .unwrap();

        let is_valid = gov_node.verify_rule_log(&log, &pubkey_bytes).unwrap();
        assert!(is_valid, "Valid log failed verification");
    }

    #[test]
    fn test_verify_fails_with_wrong_key() {
        let (mut gov_node, _) = setup_gov_node();
        
        let impostor_key = SigningKey::generate(&mut OsRng);
        let impostor_pubkey = impostor_key.verifying_key().to_bytes();

        let log = gov_node
            .append_new_rule_log(BTreeMap::new(), vec![], 5, 500, 1)
            .unwrap();

        let is_valid = gov_node.verify_rule_log(&log, &impostor_pubkey).unwrap();
        assert!(!is_valid, "Log should not verify with an incorrect public key");
    }

    #[test]
    fn test_verify_fails_with_tampered_data() {
        let (mut gov_node, signing_key) = setup_gov_node();
        let pubkey_bytes = signing_key.verifying_key().to_bytes();

        let mut log = gov_node
            .append_new_rule_log(BTreeMap::new(), vec![], 10, 1000, 1)
            .unwrap();

        log.tax_rate_percent = 99; 

        let is_valid = gov_node.verify_rule_log(&log, &pubkey_bytes).unwrap();
        assert!(!is_valid, "Tampered data should have failed hash verification");
    }

    #[test]
fn test_logs_are_overwritten_past_logs_discarded() {
    let (mut gov_node, signing_key) = setup_gov_node();
    let pubkey_bytes = signing_key.verifying_key().to_bytes();

    let log_v1 = gov_node
        .append_new_rule_log(BTreeMap::new(), vec![], 5, 500, 1)
        .unwrap();
    
    assert_eq!(log_v1.version_number, 1);
    assert_eq!(gov_node.latest().unwrap().version_number, 1);
    assert!(gov_node.verify_rule_log(&log_v1, &pubkey_bytes).unwrap());

    let log_v2 = gov_node
        .append_new_rule_log(BTreeMap::new(), vec![], 10, 1000, 2)
        .unwrap();
    
    assert_eq!(log_v2.version_number, 2);
    assert_eq!(gov_node.latest().unwrap().version_number, 2);
    
    let log_v3 = gov_node
        .append_new_rule_log(BTreeMap::new(), vec![], 15, 1500, 3)
        .unwrap();
    
    assert_eq!(log_v3.version_number, 3);
    
    let latest = gov_node.latest().unwrap();
    assert_eq!(latest.version_number, 3, "Only version 3 should be accessible");
    assert_eq!(latest.tax_rate_percent, 15, "Latest log should have tax rate of 15");
    assert_eq!(latest.effective_slot, 3, "Latest log should have effective slot of 3");
    
    assert_ne!(latest.version_number, log_v1.version_number, "Version 1 should be discarded");
    assert_ne!(latest.version_number, log_v2.version_number, "Version 2 should be discarded");
    
    assert!(gov_node.verify_rule_log(&log_v1, &pubkey_bytes).unwrap(), 
            "Old log v1 is cryptographically valid but not stored");
    assert!(gov_node.verify_rule_log(&log_v2, &pubkey_bytes).unwrap(),
            "Old log v2 is cryptographically valid but not stored");
    
    assert_eq!(gov_node.latest_log.as_ref().unwrap().version_number, 3,
               "Internal storage only keeps version 3");
}
}