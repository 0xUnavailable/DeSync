use anyhow::Result;
use std::collections::{HashMap, HashSet};

use crate::network::libp2p::NodeState;
use crate::utils::persistence::{load_events, load_snapshot, PersistentRecord};

pub fn recover_state() -> Result<NodeState> {
    if let Some(snapshot) = load_snapshot()? {
        return Ok(snapshot);
    }

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
                if state.realm_id == [0u8; 32] {
                    state.realm_id = log.realm_id;
                }

                if let Some(current) = state.latest_government_log.take() {
                    state.previous_government_log = Some(current);
                }

                state.latest_government_log = Some(log.clone());
                state.latest_gov_log_version = log.version_number;
            }

            PersistentRecord::TaxObligation(env) => {
                let hash = env.obligation.obligation_hash;
                if !state.pending_obligations.contains_key(&hash) {
                    state.pending_obligations.insert(hash, env.clone());
                }
                state.tax_obligations.push(env);
            }

            PersistentRecord::TaxReceipt(env) => {
                let receipt_hash =
                    *blake3::hash(&crate::utils::serialize(&env.receipt).unwrap()).as_bytes();

                state
                    .pending_obligations
                    .remove(&env.receipt.obligation_hash);

                if state.seen_receipts.insert(receipt_hash) {
                    state.tax_receipts.push(env);
                }
            }

            PersistentRecord::P2PReceipt(env) => {
                state.p2p_receipts.push(env);
            }

            PersistentRecord::ChatMessage(_) => {
                // SOON
            }

            PersistentRecord::TaskStarted(_) => {
              // SOON
            }

            PersistentRecord::TaskHarvested(_) => {
               // SOON
            }
        }
    }

    Ok(state)
}
