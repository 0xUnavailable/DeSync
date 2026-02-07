use anyhow::Result;
use crate::types::RealmObject;
use crate::network::libp2p::NodeState;
use crate::utils::persistence::{PersistentRecord, append_event};


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
    if state.latest_government_log.is_some() && log.version_number <= state.latest_gov_log_version {
        return Ok(());
    }
    
    if state.realm_id == [0u8; 32] {
        state.realm_id = log.realm_id;
    }
    
    if log.realm_id != state.realm_id {
        anyhow::bail!("Realm ID mismatch");
    }
    
    if let Some(ref current_log) = state.latest_government_log {
        if log.previous_log_hash != current_log.log_hash {
            anyhow::bail!("Government log chain broken");
        }
    }
    
    if let Some(current) = state.latest_government_log.take() {
        state.previous_government_log = Some(current);
    }
    
    state.latest_gov_log_version = log.version_number;
    state.latest_government_log = Some(log.clone());
    
    append_event(&PersistentRecord::GovernmentLog(log))?;
    
    Ok(())
}

fn apply_tax_obligation(
    env: crate::types::TaxObligationEnvelope,
    state: &mut NodeState,
) -> Result<()> {
    let hash = env.obligation.obligation_hash;
    
    if state.pending_obligations.contains_key(&hash) {
        return Ok(());
    }
    
    state.pending_obligations.insert(hash, env.clone());
    state.tax_obligations.push(env.clone());
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
    
    if state.seen_receipts.contains(&receipt_hash) {
        return Ok(());
    }
    
    state.pending_obligations.remove(&env.receipt.obligation_hash);
    
    state.seen_receipts.insert(receipt_hash);
    
    state.coins += env.receipt.coins_transferred as i64;
    
    state.tax_receipts.push(env.clone());

    update_user_state_with_receipt(&env, config)?;
    
    append_event(&PersistentRecord::TaxReceipt(env))?;
    
    Ok(())
}

fn update_user_state_with_receipt(
    env: &crate::types::TaxReceiptEnvelope,
    config: &crate::network::libp2p::NetworkConfig,
) -> Result<()> {
    use crate::utils::file_protection::ProtectedFile;
    use crate::user::state_manager::UserStateManager;
    use std::path::Path;
    
    println!("[DEBUG] update_user_state_with_receipt called");
    println!("  Obligation hash: {}", hex::encode(env.receipt.obligation_hash));
    
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
            user_state.process_tax_receipt(
                env.receipt.obligation_hash,
                *blake3::hash(&serde_json::to_vec(&env.receipt)?).as_bytes(),
                env.receipt.coins_transferred,
            );
            
            println!("  Saving UserStateManager...");
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
    
   let our_key = VerifyingKey::from_bytes(
    &config.keypair.public().try_into_ed25519()
        .map_err(|_| anyhow::anyhow!("Not an Ed25519 key"))? 
        .to_bytes()
)?;
    
    let is_sender = env.receipt.from == our_key;
    let is_receiver = env.receipt.to == our_key;
    
    println!("[P2P] Processing transfer receipt:");
    println!("  From: {}", hex::encode(env.receipt.from.to_bytes()));
    println!("  To: {}", hex::encode(env.receipt.to.to_bytes()));
    println!("  Amount: {}", env.receipt.amount);
    println!("  We are: {}", if is_sender { "SENDER (ignore)" } else if is_receiver { "RECEIVER (credit)" } else { "NEITHER" });

    if is_receiver {
        let credit_amount = -env.receipt.amount;
        
        println!("  Crediting {} coins to receiver", credit_amount);
        
        state.coins += credit_amount;
        
        update_user_state_with_p2p(&env, config, credit_amount)?;
    } else if is_sender {
        println!("  Sender sees own receipt - no balance change (already deducted)");
    } else {
        println!("[WARN] Received P2P receipt where we are neither sender nor receiver");
    }
    
    state.p2p_receipts.push(env.clone());
    
    append_event(&PersistentRecord::P2PReceipt(env))?;
    
    Ok(())
}

fn update_user_state_with_p2p(
    _: &crate::types::ReceiptEnvelope,
    config: &crate::network::libp2p::NetworkConfig,
    balance_change: i64,
) -> Result<()> {
    use crate::utils::file_protection::ProtectedFile;
    use crate::user::state_manager::UserStateManager;
    use std::path::Path;
    
    let data_dir = match &config.data_dir {
        Some(dir) => dir,
        None => return Ok(()), 
    };
    
    let state_file = ProtectedFile::new(Path::new(data_dir).join("user_state.json"));
    
    if !state_file.exists() {
        return Ok(());
    }
    
    match state_file.read::<UserStateManager>() {
        Ok(mut user_state) => {
            user_state.coins += balance_change;
            
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

