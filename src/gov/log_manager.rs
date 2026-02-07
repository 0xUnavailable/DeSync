use anyhow::{anyhow, Result};
use ed25519_dalek::{Signer, SigningKey};
use crate::utils::serialization::serialize;
use crate::types::{ActionRule, GovernmentRuleLog, Resources, HASH};


#[derive(serde::Serialize, serde::Deserialize)]
pub struct GovernmentNode {
    pub signing_key: SigningKey,
    pub latest_log: Option<GovernmentRuleLog>,
    pub previous_log: Option<GovernmentRuleLog>,
}

impl GovernmentNode {
    pub fn new(signing_key: SigningKey) -> Self {
        Self {
            signing_key,
            latest_log: None,
            previous_log: None,
        }
    }

    pub fn generate_genesis_government_log(
        realm_id: HASH,
        signing_key: &SigningKey,
    ) -> Result<GovernmentRuleLog> {
        let default_rules = vec![
            ActionRule {
                task_name: "farming".to_string(),
                coin_multiplier: 10,
                required_resources: Resources {
                    land: 1,
                    labor: 1,
                    capital: 0,
                },
            },
            ActionRule {
                task_name: "mining".to_string(),
                coin_multiplier: 15,
                required_resources: Resources {
                    land: 1,
                    labor: 1,
                    capital: 1,
                },
            },
        ];

        let mut log = GovernmentRuleLog {
            version_number: 0,
            realm_id,
            previous_log_hash: [0; 32],
            state_rules: default_rules,
            tax_amount: 100, // Default tax amount
            log_hash: [0; 32],
            government_signature: [0; 64],
        };

        let hash_input = serialize(&log)?;
        log.log_hash = *blake3::hash(&hash_input).as_bytes();
        log.government_signature = signing_key.sign(&log.log_hash).to_bytes();

        Ok(log)
    }

    pub fn append_log(
        &mut self,
        new_rules: Vec<ActionRule>,
        tax_amount: u64,
    ) -> Result<GovernmentRuleLog> {
        let current_log = self
            .latest_log
            .as_ref()
            .ok_or_else(|| anyhow!("No current log to append to"))?;

        let new_version = current_log.version_number + 1;
        let previous_hash = current_log.log_hash;

        let mut new_log = GovernmentRuleLog {
            version_number: new_version,
            realm_id: current_log.realm_id,
            previous_log_hash: previous_hash,
            state_rules: new_rules,
            tax_amount,
            log_hash: [0; 32],
            government_signature: [0; 64],
        };

        let hash_input = serialize(&new_log)?;
        new_log.log_hash = *blake3::hash(&hash_input).as_bytes();
        new_log.government_signature = self.signing_key.sign(&new_log.log_hash).to_bytes();

        self.previous_log = self.latest_log.clone();
        self.latest_log = Some(new_log.clone());

        Ok(new_log)
    }

    pub fn create_successor_log(
        previous_log: &GovernmentRuleLog,
        new_rules: Vec<ActionRule>,
        new_tax_amount: u64,
        signing_key: &SigningKey,
    ) -> Result<GovernmentRuleLog> {
        let new_version = previous_log.version_number + 1;

        let mut log = GovernmentRuleLog {
            version_number: new_version,
            realm_id: previous_log.realm_id,
            previous_log_hash: previous_log.log_hash,
            state_rules: new_rules,
            tax_amount: new_tax_amount,
            log_hash: [0u8; 32],
            government_signature: [0u8; 64],
        };

        let log_bytes = serialize(&log)?;
        log.log_hash = *blake3::hash(&log_bytes).as_bytes();
        let signature = signing_key.sign(&log.log_hash);
        log.government_signature = signature.to_bytes();

        Ok(log)
    }

    pub fn verify_log_chain(&self, log: &GovernmentRuleLog) -> Result<()> {
        if let Some(current) = &self.latest_log {
            if log.version_number != current.version_number + 1 {
                return Err(anyhow!("Invalid version number"));
            }

            if log.previous_log_hash != current.log_hash {
                return Err(anyhow!("Invalid previous log hash"));
            }
        }

        let mut log_copy = log.clone();
        log_copy.government_signature = [0; 64];

        let computed_hash = *blake3::hash(&serialize(&log_copy)?).as_bytes();

        if computed_hash != log.log_hash {
            return Err(anyhow!("Invalid log hash"));
        }

        Ok(())
    }
}
