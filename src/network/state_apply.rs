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
    config: &crate::network::libp2p::NetworkConfig,
) -> Result<()> {
    match object {
        RealmObject::GovernmentRuleLog(log) => {
            apply_government_log(log, state)?;
        }
        RealmObject::TaxObligationEnvelope(env) => {
            apply_tax_obligation(env, state)?;
        }
        RealmObject::TaxReceiptEnvelope(env) => {
            apply_tax_receipt(env, state, config)?;
        }
        RealmObject::P2PReceiptEnvelope(env) => {
            apply_p2p_receipt(env, state, config)?;
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
    config: &crate::network::libp2p::NetworkConfig,
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
    
    // FIX: Update UserStateManager with the receipt to mark payment as CONFIRMED
    // This synchronizes the persistent user state with the network state
    update_user_state_with_receipt(&env, config)?;
    
    // Append to event log
    append_event(&PersistentRecord::TaxReceipt(env))?;
    
    Ok(())
}

/// Update UserStateManager when a tax receipt is received
fn update_user_state_with_receipt(
    env: &crate::types::TaxReceiptEnvelope,
    config: &crate::network::libp2p::NetworkConfig,
) -> Result<()> {
    use crate::utils::file_protection::ProtectedFile;
    use crate::user::state_manager::UserStateManager;
    use std::path::Path;
    
    println!("[DEBUG] update_user_state_with_receipt called");
    println!("  Obligation hash: {}", hex::encode(env.receipt.obligation_hash));
    
    // Only update if we have a data_dir (user node)
    let data_dir = match &config.data_dir {
        Some(dir) => {
            println!("  Data dir: {}", dir);
            dir
        }
        None => {
            println!("  No data_dir in config (government node?)");
            return Ok(());
        }
    };
    
    let state_file = ProtectedFile::new(Path::new(data_dir).join("user_state.json"));
    
    if !state_file.exists() {
        println!("  State file doesn't exist");
        return Ok(());
    }
    
    println!("  Reading UserStateManager...");
    match state_file.read::<UserStateManager>() {
        Ok(mut user_state) => {
            println!("  UserStateManager loaded, processing receipt...");
            // Process the receipt to update the payment record
            user_state.process_tax_receipt(
                env.receipt.obligation_hash,
                *blake3::hash(&serde_json::to_vec(&env.receipt)?).as_bytes(),
                env.receipt.coins_transferred,
            );
            
            println!("  Saving UserStateManager...");
            // Save updated state
            state_file.write(&user_state)?;
            
            println!("[STATE] UserStateManager saved successfully!");
            log::info!(
                "[STATE] Updated UserStateManager with tax receipt for obligation {}",
                hex::encode(&env.receipt.obligation_hash[..8])
            );
        }
        Err(e) => {
            println!("  ERROR reading UserStateManager: {}", e);
            log::debug!("Could not update UserStateManager: {}", e);
        }
    }
    
    Ok(())
}

fn apply_p2p_receipt(
    env: crate::types::ReceiptEnvelope,
    state: &mut NodeState,
    config: &crate::network::libp2p::NetworkConfig,
) -> Result<()> {
    use ed25519_dalek::VerifyingKey;
    
    // Get our public key from config
   let our_key = VerifyingKey::from_bytes(
    &config.keypair.public().try_into_ed25519()
        .map_err(|_| anyhow::anyhow!("Not an Ed25519 key"))? // Changed ok_or_else to map_err
        .to_bytes()
)?;
    
    // Determine if we are sender or receiver
    let is_sender = env.receipt.from == our_key;
    let is_receiver = env.receipt.to == our_key;
    
    println!("[P2P] Processing transfer receipt:");
    println!("  From: {}", hex::encode(env.receipt.from.to_bytes()));
    println!("  To: {}", hex::encode(env.receipt.to.to_bytes()));
    println!("  Amount: {}", env.receipt.amount);
    println!("  We are: {}", if is_sender { "SENDER (ignore)" } else if is_receiver { "RECEIVER (credit)" } else { "NEITHER" });
    
    // Only apply if we are the RECEIVER
    // Sender already deducted coins when creating the transfer
    if is_receiver {
        let credit_amount = -env.receipt.amount; // Negate to make positive
        
        println!("  Crediting {} coins to receiver", credit_amount);
        
        // Apply balance change to NodeState
        state.coins += credit_amount;
        
        // Update UserStateManager
        update_user_state_with_p2p(&env, config, credit_amount)?;
    } else if is_sender {
        println!("  Sender sees own receipt - no balance change (already deducted)");
    } else {
        println!("[WARN] Received P2P receipt where we are neither sender nor receiver");
    }
    
    // Add to receipts for record keeping
    state.p2p_receipts.push(env.clone());
    
    // Append to event log
    append_event(&PersistentRecord::P2PReceipt(env))?;
    
    Ok(())
}

/// Update UserStateManager when a P2P receipt is received
fn update_user_state_with_p2p(
    _: &crate::types::ReceiptEnvelope,
    config: &crate::network::libp2p::NetworkConfig,
    balance_change: i64,
) -> Result<()> {
    use crate::utils::file_protection::ProtectedFile;
    use crate::user::state_manager::UserStateManager;
    use std::path::Path;
    
    // Only update if we have a data_dir (user node)
    let data_dir = match &config.data_dir {
        Some(dir) => dir,
        None => return Ok(()), // Not a user node
    };
    
    let state_file = ProtectedFile::new(Path::new(data_dir).join("user_state.json"));
    
    if !state_file.exists() {
        return Ok(());
    }
    
    match state_file.read::<UserStateManager>() {
        Ok(mut user_state) => {
            // Update coin balance in UserStateManager with the calculated change
            user_state.coins += balance_change;
            
            // Save updated state
            state_file.write(&user_state)?;
            
            log::info!(
                "[STATE] Updated UserStateManager with P2P receipt: {} coins (new balance: {})",
                balance_change,
                user_state.coins
            );
        }
        Err(e) => {
            log::debug!("Could not update UserStateManager: {}", e);
        }
    }
    
    Ok(())
}

