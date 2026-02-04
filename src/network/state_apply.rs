use anyhow::Result;
use crate::types::RealmObject;
use crate::network::libp2p::NodeState;
use crate::utils::persistence::{PersistentRecord, append_event};

/// Apply a verified RealmObject to NodeState
/// Objects MUST be verified before calling this function
/// This function mutates state deterministically
pub fn apply_object_to_state(
    object: RealmObject,
    state: &mut NodeState,
) -> Result<()> {
    match object {
        RealmObject::GovernmentRuleLog(log) => {
            apply_government_log(log, state)?;
        }
        RealmObject::TaxObligationEnvelope(env) => {
            apply_tax_obligation(env, state)?;
        }
        RealmObject::TaxReceiptEnvelope(env) => {
            apply_tax_receipt(env, state)?;
        }
        RealmObject::P2PReceiptEnvelope(env) => {
            apply_p2p_receipt(env, state)?;
        }
    }
    
    Ok(())
}

fn apply_government_log(
    log: crate::types::GovernmentRuleLog,
    state: &mut NodeState,
) -> Result<()> {
    // Validate ordering - new log must be newer than current IF we have a current log
    // This allows genesis (version 0) to be applied when state is empty
    if state.latest_government_log.is_some() && log.version_number <= state.latest_gov_log_version {
        // Ignore old or duplicate logs
        return Ok(());
    }
    
    // Update realm_id if this is the first log
    if state.realm_id == [0u8; 32] {
        state.realm_id = log.realm_id;
    }
    
    // Verify realm_id matches
    if log.realm_id != state.realm_id {
        anyhow::bail!("Realm ID mismatch");
    }
    
    // Validate chain
    if let Some(ref current_log) = state.latest_government_log {
        if log.previous_log_hash != current_log.log_hash {
            anyhow::bail!("Government log chain broken");
        }
    }
    
    // Move current to previous
    if let Some(current) = state.latest_government_log.take() {
        state.previous_government_log = Some(current);
    }
    
    // Update state
    state.latest_gov_log_version = log.version_number;
    state.latest_government_log = Some(log.clone());
    
    // Append to event log
    append_event(&PersistentRecord::GovernmentLog(log))?;
    
    Ok(())
}

fn apply_tax_obligation(
    env: crate::types::TaxObligationEnvelope,
    state: &mut NodeState,
) -> Result<()> {
    let hash = env.obligation.obligation_hash;
    
    // Check if already pending
    if state.pending_obligations.contains_key(&hash) {
        return Ok(()); // Duplicate, ignore
    }
    
    // Add to pending obligations
    state.pending_obligations.insert(hash, env.clone());
    state.tax_obligations.push(env.clone());
    
    // Append to event log
    append_event(&PersistentRecord::TaxObligation(env))?;
    
    Ok(())
}

fn apply_tax_receipt(
    env: crate::types::TaxReceiptEnvelope,
    state: &mut NodeState,
) -> Result<()> {
    let receipt_hash = blake3::hash(
        &serde_json::to_vec(&env.receipt).expect("serialization")
    ).into();
    
    // Check if already seen
    if state.seen_receipts.contains(&receipt_hash) {
        return Ok(()); // Duplicate, ignore
    }
    
    // Clear pending obligation if this receipt matches
    state.pending_obligations.remove(&env.receipt.obligation_hash);
    
    // Add to seen receipts
    state.seen_receipts.insert(receipt_hash);
    
    // Increase balance
    state.coins += env.receipt.coins_transferred as i64;
    
    // Add to receipts
    state.tax_receipts.push(env.clone());
    
    // Append to event log
    append_event(&PersistentRecord::TaxReceipt(env))?;
    
    Ok(())
}

fn apply_p2p_receipt(
    env: crate::types::ReceiptEnvelope,
    state: &mut NodeState,
) -> Result<()> {
    // Apply balance change
    state.coins += env.receipt.amount;
    
    // Add to receipts
    state.p2p_receipts.push(env.clone());
    
    // Append to event log
    append_event(&PersistentRecord::P2PReceipt(env))?;
    
    Ok(())
}

