use serde::{Deserialize, Serialize};
use ed25519_dalek::VerifyingKey;
pub type HASH = [u8; 32];
use serde_big_array::BigArray;

// Serde helpers for VerifyingKey
mod verifying_key_serde {
    use super::*;
    
    pub fn serialize<S>(key: &ed25519_dalek::VerifyingKey, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(key.as_bytes())
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<ed25519_dalek::VerifyingKey, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
        ed25519_dalek::VerifyingKey::from_bytes(bytes.as_slice().try_into().map_err(serde::de::Error::custom)?)
            .map_err(serde::de::Error::custom)
    }
}

/// Node role in the Realm network
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum NodeRole {
    Government,
    User,
}

impl NodeRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeRole::Government => "Government",
            NodeRole::User => "User",
        }
    }
}

/// Hello message exchanged on connection to identify peer role
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HelloMessage {
    pub role: NodeRole,
    pub realm_id: HASH,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GovernmentRuleLog {
    pub version_number: u64,
    pub realm_id: HASH,
    pub previous_log_hash: HASH,
    pub state_rules: Vec<ActionRule>,
    pub tax_amount: u64,  // NEW: Tax amount for this version
    pub log_hash: HASH,
    #[serde(with = "BigArray")]
    pub government_signature: [u8; 64],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ActionRule {
    pub task_name: String,
    pub coin_multiplier: u64,
    pub required_resources: Resources,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Resources {
    pub land: u64,
    pub labor: u64,
    pub capital: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ResourceDelta {
    pub land: i64,
    pub labor: i64,
    pub capital: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub task_name: String,
    pub start_time: u64,
    pub duration: u64,
    pub resources_locked: Resources,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GovernmentLogRef {
    pub realm_id: HASH,
    pub log_hash: HASH,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaxObligation {
    #[serde(with = "verifying_key_serde")]
    pub user_id: VerifyingKey,
    pub government_log: GovernmentLogRef,
    pub coin_amount: u64,
    pub obligation_hash: HASH,
    #[serde(with = "BigArray")]
    pub user_signature: [u8; 64],
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaxObligationEnvelope {
    pub obligation: TaxObligation,
    #[serde(with = "BigArray")]
    pub signature: [u8; 64],
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaxReceipt {
    pub obligation_hash: HASH,
    pub previous_receipt_hash: Option<HASH>,
    pub payment_slot: u64,
    pub coins_transferred: u64,
    #[serde(with = "BigArray")]
    pub government_signature: [u8; 64],
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaxReceiptEnvelope {
    pub receipt: TaxReceipt,
    #[serde(with = "BigArray")]
    pub signature: [u8; 64],
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct P2PTransferReceipt {
    #[serde(with = "verifying_key_serde")]
    pub from: VerifyingKey,
    #[serde(with = "verifying_key_serde")]
    pub to: VerifyingKey,
    pub amount: i64,
    pub timestamp: u64,
    #[serde(with = "BigArray")]
    pub signature: [u8; 64],
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReceiptEnvelope {
    pub receipt: P2PTransferReceipt,
    #[serde(with = "BigArray")]
    pub signature: [u8; 64],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ChatMessage {
    pub sender: String,
    pub content: String,
    pub timestamp: u64,
    pub message_hash: HASH,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessageEnvelope {
    pub message: ChatMessage,
    #[serde(with = "BigArray")]
    pub signature: [u8; 64],
}

/// Canonical enum for all synced objects in Realm
/// This is the single sync unit - all network-exchanged data must be wrapped in this enum
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RealmObject {
    GovernmentRuleLog(GovernmentRuleLog),
    TaxObligationEnvelope(TaxObligationEnvelope),
    TaxReceiptEnvelope(TaxReceiptEnvelope),
    P2PReceiptEnvelope(ReceiptEnvelope),
}

impl RealmObject {
    /// Compute hash of this object
    pub fn compute_hash(&self) -> HASH {
        let bytes = serde_json::to_vec(self).expect("serialization should not fail");
        blake3::hash(&bytes).into()
    }
    
    /// Verify signature based on object type
    pub fn verify_signature(&self, gov_key: &VerifyingKey) -> bool {
        use ed25519_dalek::{Signature, Verifier};
        
        match self {
            RealmObject::GovernmentRuleLog(log) => {
                // Verify government signature on log_hash
                // The signature is created by signing the log_hash directly in generate_genesis_government_log
                let sig = match Signature::try_from(&log.government_signature[..]) {
                    Ok(s) => s,
                    Err(_) => return false,
                };
                gov_key.verify(&log.log_hash, &sig).is_ok()
            }
            RealmObject::TaxReceiptEnvelope(env) => {
                // Verify government signature on receipt
                let msg = serde_json::to_vec(&env.receipt).expect("serialization");
                
                let sig = match Signature::try_from(&env.signature[..]) {
                    Ok(s) => s,
                    Err(_) => return false,
                };
                gov_key.verify(&msg, &sig).is_ok()
            }
            RealmObject::TaxObligationEnvelope(env) => {
                // Verify user signature on obligation
                let msg = serde_json::to_vec(&env.obligation).expect("serialization");
                
                let sig = match Signature::try_from(&env.signature[..]) {
                    Ok(s) => s,
                    Err(_) => return false,
                };
                env.obligation.user_id.verify(&msg, &sig).is_ok()
            }
            RealmObject::P2PReceiptEnvelope(env) => {
                // Verify sender signature on P2P receipt
                let msg = serde_json::to_vec(&env.receipt).expect("serialization");
                
                let sig = match Signature::try_from(&env.signature[..]) {
                    Ok(s) => s,
                    Err(_) => return false,
                };
                env.receipt.from.verify(&msg, &sig).is_ok()
            }
        }
    }
}