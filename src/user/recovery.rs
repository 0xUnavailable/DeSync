
use anyhow::Result;
use std::collections::{HashSet, HashMap};

use crate::network::libp2p::NodeState;
use crate::utils::persistence::{PersistentRecord, load_events, load_snapshot};

/// Recover NodeState from persistent storage
pub fn recover_state() -> Result<NodeState> {
    // Try to load snapshot first
    if let Some(snapshot) = load_snapshot()? {
        return Ok(snapshot);
    }

    // Otherwise, rebuild from events
    let events = load_events()?;
    let mut state = NodeState {
        realm_id: [0u8; 32],
        latest_government_log: None,
        latest_gov_log_version: 0,
        previous_government_log: None,
        tax_obligations: Vec::new(),
        tax_receipts: Vec::new(),
        p2p_receipts: Vec::new(),
        pending_obligations: HashMap::new(),
        seen_receipts: HashSet::new(),
        coins: 0,
    };

    for record in events {
        match record {
            PersistentRecord::GovernmentLog(log) => {
                // Update realm_id from first government log
                if state.realm_id == [0u8; 32] {
                    state.realm_id = log.realm_id;
                }
                
                // Store as previous if we have a current one
                if let Some(current) = state.latest_government_log.take() {
                    state.previous_government_log = Some(current);
                }
                
                state.latest_government_log = Some(log.clone());
                state.latest_gov_log_version = log.version_number;
            }

            PersistentRecord::TaxObligation(env) => {
                // Add to pending obligations
                let hash = env.obligation.obligation_hash;
                if !state.pending_obligations.contains_key(&hash) {
                    state.pending_obligations.insert(hash, env.clone());
                }
                state.tax_obligations.push(env);
            }

            PersistentRecord::TaxReceipt(env) => {
                let receipt_hash = *blake3::hash(&crate::utils::serialize(&env.receipt).unwrap()).as_bytes();
                
                // Remove from pending if this receipt matches an obligation
                state.pending_obligations.remove(&env.receipt.obligation_hash);
                
                // Add to receipts if not seen
                if state.seen_receipts.insert(receipt_hash) {
                    state.tax_receipts.push(env);
                }
            }

            PersistentRecord::P2PReceipt(env) => {
                state.p2p_receipts.push(env);
            }

            PersistentRecord::ChatMessage(_) => {
                // Chat messages are no longer part of core state
                // Ignore for now - can be handled separately if needed
            }

            PersistentRecord::TaskStarted(_) => {
                // Task started events are logged for audit purposes
                // They don't affect the network state reconstruction
                // User state is managed separately in user_state.json
            }

            PersistentRecord::TaskHarvested(_) => {
                // Task harvested events are logged for audit purposes
                // They don't affect the network state reconstruction
                // User state is managed separately in user_state.json
            }
        }
    }

    Ok(state)
}

