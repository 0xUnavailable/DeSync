use serde::{Serialize, Deserialize};
use crate::crypto::ed25519_utils::Signature;
use crate::types::user_state::ResourceDelta;

#[derive(Serialize, Deserialize, Clone)]
pub struct Rule {
    pub energy_cost: u64,
    pub reward: ResourceDelta,
    pub grow_duration: u64, // for crops
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GovernmentRuleLog {
    pub version: u64,
    pub rules: std::collections::HashMap<String, Rule>,
    pub log_hash: [u8;32],
    pub gov_signature: Signature,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TaxSubmission {
    pub user_state_hash: [u8;32],
    pub taxable_totals: ResourceDelta,
    pub last_tax_receipt_hash: [u8;32],
    pub user_signature: Signature,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TaxReceipt {
    pub user_state_hash: [u8;32],
    pub taxable_totals: ResourceDelta,
    pub tax_amount: u64,
    pub previous_receipt_hash: [u8;32],
    pub gov_signature: Signature,
}
