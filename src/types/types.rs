use serde::{Serialize, Deserialize};
use serde_big_array::BigArray;
use std::collections::BTreeMap;
use ed25519_dalek::VerifyingKey as PublicKey;

use crate::crypto::VDFProof;


pub type SIGNATURE = [u8; 64];
pub type HASH = [u8; 32];

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ResourceDelta {
    pub wheat: i64,
    pub wood: i64,
    pub coins: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Resources {
    pub wheat: u64,
    pub wood: u64,
    pub coins: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Plot {
    pub plot_id: String,
    pub crop: Option<String>,
    pub planted_at_event_hash: Option<HASH>,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Actions {
    Plant { crop: Crops},
    Harvest { crop: Crops},
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Crops {
    Wheat,
    Corn,
    Rice,
}

impl Crops {
    pub fn as_str(&self) -> &str {
        match self {
            Crops::Wheat => "wheat",
            Crops::Corn => "corn",
            Crops::Rice => "rice"
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub task_id: u64,
    pub action_required: Actions, //ITERATE
    pub start_slot: u64,
    pub duration_iterations: u64,
}



#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub event_id: String,
    pub user_id: PublicKey,
    pub plot_id: String,    
    pub action: Actions,
    pub task_id: Option<u64>, 
    pub deltas: ResourceDelta,     
    pub gov_log_version: u64,      
    pub gov_log_hash: HASH,     
    pub prev_event_hash: Option<HASH>,
    #[serde(with = "BigArray")]
    pub signature: SIGNATURE,      
    pub timestamp_slot: u64,
    pub proof: Option<VDFProof>,   
    pub sequence_number: u64,
}


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UserState {
    pub user_id: PublicKey,
    pub resources: Resources,
    pub energy: u32,
    pub plot: Plot, // Singular plot for now
    pub last_event_hashes: BTreeMap<String, HASH>,
    pub sequence_number: u64,
    pub government_log_version: u64,
    pub government_log_hash: HASH,
    pub prev_state_hash: Option<HASH>, 
    #[serde(with = "BigArray")]
    pub signature: SIGNATURE,
}


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ActionRule {
    pub energy_cost: u32,
    pub resource_reward: ResourceDelta,
    pub maturation_iterations: u64,
}


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GovernmentRuleLog {
    pub version_number: u64,
    pub effective_slot: u64,
    pub rules: BTreeMap<String, ActionRule>,
    pub tasks: Vec<Task>,
    pub tax_rate_percent: u8, // e.g. 10 = 10%
    pub tax_deadline: u64,
    pub log_hash: HASH, 
    #[serde(with = "BigArray")]         
    pub government_signature: SIGNATURE,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TaxObligation {
    pub event_hash: HASH,
    pub user_id: PublicKey,
    pub coin_amount: u64,
    pub government_log_hash: HASH,
    pub sequence_number: u64,
    #[serde(with = "BigArray")]
    pub user_signature:SIGNATURE,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TaxReceipt {
    pub obligation_hash: HASH,
    pub payment_slot: u64,      
    pub coins_transferred: u64,
    #[serde(with = "BigArray")]
    pub government_signature: SIGNATURE,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct P2PTransferReceipt {
    pub tx_id: String,
    pub sender: PublicKey,
    pub receiver: PublicKey,
    pub delta: ResourceDelta,      
    pub sender_balance_before: Resources,
    pub sender_balance_after: Resources,
    pub receiver_balance_before: Resources,
    pub receiver_balance_after: Resources,
    pub prev_tx_hash: Option<HASH>,
    #[serde(with = "BigArray")]
    pub sender_signature: SIGNATURE,
    #[serde(with = "BigArray")]
    pub receiver_signature: SIGNATURE,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Snapshot {
    pub resources: ResourceDelta,
    pub energy: u64,
    pub land: Vec<Plot>,
    pub tasks: Vec<Task>,
    pub timestamp: u64,
}

#[cfg(test)]
mod tests {
    use crate::utils::{deserialize, serialize};

    use super::*;

    #[test]
    fn test_serialization_determinism() {
        // 1. Create first instance
        let mut rules_a = BTreeMap::new();
        rules_a.insert("plant".to_string(), ActionRule { 
            energy_cost: 10, 
            resource_reward: ResourceDelta { wheat: 0, wood: 0, coins: 0 }, 
            maturation_iterations: 5 
        });
        rules_a.insert("harvest".to_string(), ActionRule { 
            energy_cost: 5, 
            resource_reward: ResourceDelta { wheat: 10, wood: 0, coins: 0 }, 
            maturation_iterations: 0 
        });

        let log_a = GovernmentRuleLog {
            version_number: 1,
            effective_slot: 100,
            rules: rules_a,
            tasks: vec![],
            tax_rate_percent: 10,
            tax_deadline: 1000,
            log_hash: [0;32],
            government_signature:[0;64],
        };

        // 2. Create second instance with IDENTICAL data but REVERSED insertion order
        let mut rules_b = BTreeMap::new();
        rules_b.insert("harvest".to_string(), ActionRule { 
            energy_cost: 5, 
            resource_reward: ResourceDelta { wheat: 10, wood: 0, coins: 0 }, 
            maturation_iterations: 0 
        });
        rules_b.insert("plant".to_string(), ActionRule { 
            energy_cost: 10, 
            resource_reward: ResourceDelta { wheat: 0, wood: 0, coins: 0 }, 
            maturation_iterations: 5 
        });

        let log_b = GovernmentRuleLog {
            version_number: 1,
            effective_slot: 100,
            rules: rules_b,
            tasks: vec![],
            tax_rate_percent: 10,
            tax_deadline: 1000,
            log_hash:[0;32],
            government_signature:[0; 64],
        };

        // 3. Serialize both
        let bytes_a = serialize(&log_a).expect("Serialization failed");
        let bytes_b = serialize(&log_b).expect("Serialization failed");

        // 4. ASSERT: The bytes must be identical for the hash to be consistent
        assert_eq!(bytes_a, bytes_b, "Determinism failed! Bytes are not identical.");
        
        // 5. ASSERT: Deserialization recovers the original object
        let recovered: GovernmentRuleLog = deserialize(&bytes_a).expect("Deserialization failed");
        assert_eq!(log_a, recovered);
        
        println!("Determinism check passed. Bytes length: {}", bytes_a.len());
    }
}