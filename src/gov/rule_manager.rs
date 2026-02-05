use anyhow::Result;
use ed25519_dalek::SigningKey;
use crate::types::{GovernmentRuleLog, ActionRule, Resources};
use crate::storage::OrganizedObjectStore;

/// Get the latest government log from storage
pub fn get_latest_log(object_store: &OrganizedObjectStore) -> Result<GovernmentRuleLog> {
    use crate::types::RealmObject;
    
    let all_hashes = object_store.list_all()?;
    
    let mut logs = Vec::new();
    for hash in all_hashes {
        if let Some(bytes) = object_store.get_object(&hash)? {
            if let Ok(RealmObject::GovernmentRuleLog(log)) = serde_json::from_slice(&bytes) {
                logs.push(log);
            }
        }
    }
    
    logs.into_iter()
        .max_by_key(|log| log.version_number)
        .ok_or_else(|| anyhow::anyhow!("No government logs found"))
}

/// Create new log with updated tax amount
pub fn create_tax_update(
    current_log: &GovernmentRuleLog,
    new_tax_amount: u64,
    signing_key: &SigningKey,
) -> Result<GovernmentRuleLog> {
    // Copy current rules, update tax only
    super::log_manager::GovernmentNode::create_successor_log(
        current_log,
        current_log.state_rules.clone(),
        new_tax_amount,
        signing_key,
    )
}

/// Create new log with added task
pub fn create_add_task(
    current_log: &GovernmentRuleLog,
    task_name: String,
    multiplier: u64,
    resources: Resources,
    signing_key: &SigningKey,
) -> Result<GovernmentRuleLog> {
    let mut new_rules = current_log.state_rules.clone();
    
    // Check if task already exists
    if new_rules.iter().any(|r| r.task_name == task_name) {
        return Err(anyhow::anyhow!("Task '{}' already exists", task_name));
    }
    
    new_rules.push(ActionRule {
        task_name,
        coin_multiplier: multiplier,
        required_resources: resources,
    });
    
    super::log_manager::GovernmentNode::create_successor_log(
        current_log,
        new_rules,
        current_log.tax_amount,
        signing_key,
    )
}

/// Create new log with modified task
pub fn create_modify_task(
    current_log: &GovernmentRuleLog,
    task_name: &str,
    multiplier: Option<u64>,
    land: Option<u64>,
    labor: Option<u64>,
    capital: Option<u64>,
    signing_key: &SigningKey,
) -> Result<GovernmentRuleLog> {
    let mut new_rules = current_log.state_rules.clone();
    
    // Find and modify task
    let task = new_rules.iter_mut()
        .find(|r| r.task_name == task_name)
        .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", task_name))?;
    
    if let Some(m) = multiplier {
        task.coin_multiplier = m;
    }
    if let Some(l) = land {
        task.required_resources.land = l;
    }
    if let Some(l) = labor {
        task.required_resources.labor = l;
    }
    if let Some(c) = capital {
        task.required_resources.capital = c;
    }
    
    super::log_manager::GovernmentNode::create_successor_log(
        current_log,
        new_rules,
        current_log.tax_amount,
        signing_key,
    )
}

/// Create new log with removed task
pub fn create_remove_task(
    current_log: &GovernmentRuleLog,
    task_name: &str,
    signing_key: &SigningKey,
) -> Result<GovernmentRuleLog> {
    let mut new_rules = current_log.state_rules.clone();
    
    let initial_len = new_rules.len();
    new_rules.retain(|r| r.task_name != task_name);
    
    if new_rules.len() == initial_len {
        return Err(anyhow::anyhow!("Task '{}' not found", task_name));
    }
    
    super::log_manager::GovernmentNode::create_successor_log(
        current_log,
        new_rules,
        current_log.tax_amount,
        signing_key,
    )
}
