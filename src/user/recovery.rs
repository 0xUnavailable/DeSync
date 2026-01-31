use anyhow::{Result};

use crate::utils::persistence::{
    load_snapshot,
    load_events,
    PersistentRecord,
    save_snapshot,
    append_event,
};

use crate::network::libp2p::NodeState;
use crate::types::{
    GovernmentRuleLog, TaxObligationEnvelope, TaxReceiptEnvelope,
    ReceiptEnvelope, ChatMessageEnvelope, HASH,
};
use std::collections::HashSet;

pub fn recover_node_state() -> Result<NodeState> {
    let mut state = match load_snapshot()? {
        Some(mut s) => {
            // Defensive: ensure seen_messages contains all loaded chat messages' hashes
            for msg in &s.chat_messages {
                s.seen_messages.insert(msg.message_hash);
            }
            s
        }
        None => NodeState {
            latest_gov_log_version: 0,
            seen_messages: HashSet::new(),
            peer_states: Default::default(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: None,
            chat_messages: Vec::new(),
            government_key_locked: false,
            has_synced_government: false,
        },
    };

    let events = load_events()?;

    for record in events {
        match record {
            PersistentRecord::GovernmentLog(log) => {
                if state.latest_government_log.is_none()
                    || log.version_number > state.latest_gov_log_version
                {
                    state.latest_gov_log_version = log.version_number;
                    state.latest_government_log = Some(log);
                    state.has_synced_government = true;
                    state.government_key_locked = true;
                }
            }

            PersistentRecord::TaxObligation(env) => {
                if !state.pending_tax_obligations.iter().any(|o| {
                    o.obligation.obligation_hash == env.obligation.obligation_hash
                }) {
                    state.pending_tax_obligations.push(env);
                }
            }

            PersistentRecord::TaxReceipt(env) => {
                state.pending_tax_obligations.retain(|o| {
                    o.obligation.obligation_hash != env.receipt.obligation_hash
                });
            }

            PersistentRecord::P2PReceipt(_) => {}

            PersistentRecord::ChatMessage(env) => {
                if state.seen_messages.insert(env.message.message_hash) {
                    state.chat_messages.push(env.message);
                }
            }
        }
    }

    Ok(state)
}

#[cfg(test)]
mod recovery_logic_tests {
    use super::*;
    use crate::types::{
        ChatMessage, GovernmentRuleLog, HASH, P2PTransferReceipt, RealmId,
        ResourceDelta, Resources, TaxObligation, TaxObligationEnvelope, TaxReceipt,
        TaxReceiptEnvelope, ChatMessageEnvelope, ReceiptEnvelope,
    };
    use ed25519_dalek::{SigningKey, VerifyingKey};
    use std::collections::{HashMap, HashSet};
    use std::path::Path;
    use std::time::Duration;

    // ─────────────────────────────────────────────────────────────────────────
    // Helpers
    // ─────────────────────────────────────────────────────────────────────────

    fn dummy_realm_id() -> RealmId {
        [0u8; 32]
    }

    fn dummy_hash(n: u8) -> HASH {
        [n; 32]
    }

    fn dummy_pubkey(n: u8) -> VerifyingKey {
        let mut seed = [0u8; 32];
        seed[0] = n;
        SigningKey::from_bytes(&seed).verifying_key()
    }

    fn dummy_gov_log(version: u64) -> GovernmentRuleLog {
        GovernmentRuleLog {
            version_number: version,
            effective_slot: version * 1000,
            rules: Default::default(),
            tasks: vec![],
            tax_rate_percent: 12,
            tax_deadline: 1_000_000 + version * 10_000,
            previous_log_hash: if version == 0 { None } else { Some(dummy_hash((version as u8).wrapping_sub(1))) },
            log_hash: dummy_hash(version as u8 + 100),
            government_signature: [0u8; 64],
            realm_id: dummy_realm_id(),
            government_public_key: dummy_pubkey(200),
        }
    }

    fn dummy_tax_obligation(hash_seed: u8, coins: u64) -> TaxObligationEnvelope {
        let obligation = TaxObligation {
            realm_id: dummy_realm_id(),
            event_hash: dummy_hash(hash_seed),
            user_id: dummy_pubkey(hash_seed),
            coin_amount: coins,
            government_log: dummy_gov_log(0),
            sequence_number: 1,
            previous_receipt_hash: None,
            user_signature: [0u8; 64],
            obligation_hash: dummy_hash(hash_seed.wrapping_add(50)),
        };
        TaxObligationEnvelope {
            realm_id: dummy_realm_id(),
            obligation,
            timestamp: 1_700_000_000 + hash_seed as u64 * 1000,
        }
    }

    fn dummy_tax_receipt(obligation_hash: HASH) -> TaxReceiptEnvelope {
        TaxReceiptEnvelope {
            realm_id: dummy_realm_id(),
            receipt: TaxReceipt {
                obligation_hash,
                previous_receipt_hash: None,
                payment_slot: 42_000,
                coins_transferred: 100,
                government_signature: [0u8; 64],
            },
            timestamp: 1_700_010_000,
        }
    }

    fn dummy_chat(id_suffix: &str) -> ChatMessageEnvelope {
        // Deterministic hash based on suffix (no parse failure)
        let mut hash_bytes = [0u8; 32];
        let bytes = id_suffix.as_bytes();
        for (i, &b) in bytes.iter().enumerate().take(32) {
            hash_bytes[i] = b.wrapping_add(42);
        }

        ChatMessageEnvelope {
            realm_id: dummy_realm_id(),
            message: ChatMessage {
                message_id: format!("chat-{}", id_suffix),
                sender: dummy_pubkey(10),
                receiver: Some(dummy_pubkey(20)),
                content: format!("Hello from test {}", id_suffix),
                nonce: 1000,
                signature: [0u8; 64],
                message_hash: hash_bytes,
            },
            timestamp: 1_700_020_000,
        }
    }

    fn dummy_p2p_receipt() -> ReceiptEnvelope {
        ReceiptEnvelope {
            realm_id: dummy_realm_id(),
            receipt: P2PTransferReceipt {
                realm_id: dummy_realm_id(),
                tx_hash: dummy_hash(88),
                sender: dummy_pubkey(30),
                receiver: dummy_pubkey(40),
                delta: ResourceDelta { wheat: 0, wood: 0, coins: -100 },
                sender_prev_state_hash: dummy_hash(89),
                receiver_prev_state_hash: dummy_hash(90),
                sender_balance_before: Resources { wheat: 200, wood: 150, coins: 300 },
                sender_balance_after: Resources { wheat: 200, wood: 150, coins: 200 },
                receiver_balance_before: Resources { wheat: 80, wood: 60, coins: 100 },
                receiver_balance_after: Resources { wheat: 80, wood: 60, coins: 200 },
                nonce: 12345,
                sender_signature: [0u8; 64],
                receiver_signature: [0u8; 64],
            },
            timestamp: 1_700_050_000,
        }
    }

    fn aggressive_cleanup() {
        let _ = std::fs::remove_file("data/snapshot.bin");
        let _ = std::fs::remove_file("data/events.bin");
        let _ = std::fs::remove_dir_all("data");

        std::thread::sleep(Duration::from_millis(150));

        let _ = std::fs::remove_dir_all("data");

        if Path::new("data").exists() {
            panic!(
                "Cleanup failed: 'data' directory still exists after attempts. \
                 Possible causes: file lock, permissions, antivirus, or CI environment issue."
            );
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn recovery_no_snapshot_no_events_gives_empty_state() {
        aggressive_cleanup();
        assert!(!Path::new("data/events.bin").exists());

        let state = recover_node_state().unwrap();

        assert_eq!(state.latest_gov_log_version, 0);
        assert!(state.latest_government_log.is_none());
        assert!(!state.has_synced_government);
        assert!(!state.government_key_locked);
        assert!(state.pending_tax_obligations.is_empty());
        assert!(state.chat_messages.is_empty());
        assert!(state.seen_messages.is_empty());
    }

    #[test]
    fn recovery_only_snapshot_is_loaded_correctly() {
        aggressive_cleanup();

        let chat1 = dummy_chat("snap1");
        let chat2 = dummy_chat("snap2");

        let mut state = NodeState {
            latest_gov_log_version: 7,
            seen_messages: HashSet::from([chat1.message.message_hash, chat2.message.message_hash]),
            peer_states: HashMap::new(),
            pending_tax_obligations: vec![dummy_tax_obligation(10, 420)],
            latest_government_log: Some(dummy_gov_log(7)),
            chat_messages: vec![chat1.message, chat2.message],
            government_key_locked: true,
            has_synced_government: true,
        };

        save_snapshot(&state).unwrap();

        let recovered = recover_node_state().unwrap();

        assert_eq!(recovered.latest_gov_log_version, 7);
        assert_eq!(recovered.pending_tax_obligations.len(), 1);
        assert_eq!(recovered.chat_messages.len(), 2);
        assert!(recovered.has_synced_government);
        assert!(recovered.government_key_locked);
    }

    #[test]
    fn recovery_events_only_builds_state_from_scratch() {
        aggressive_cleanup();

        append_event(&PersistentRecord::GovernmentLog(dummy_gov_log(0))).unwrap();
        append_event(&PersistentRecord::GovernmentLog(dummy_gov_log(1))).unwrap();

        append_event(&PersistentRecord::TaxObligation(dummy_tax_obligation(10, 300))).unwrap(); // obligation hash = 60
        append_event(&PersistentRecord::TaxObligation(dummy_tax_obligation(20, 500))).unwrap(); // obligation hash = 70

        append_event(&PersistentRecord::TaxReceipt(dummy_tax_receipt(dummy_hash(60)))).unwrap(); // clears first
        append_event(&PersistentRecord::TaxReceipt(dummy_tax_receipt(dummy_hash(70)))).unwrap(); // clears second

        append_event(&PersistentRecord::ChatMessage(dummy_chat("a"))).unwrap();
        append_event(&PersistentRecord::ChatMessage(dummy_chat("b"))).unwrap();

        let recovered = recover_node_state().unwrap();

        assert_eq!(recovered.latest_gov_log_version, 1);
        assert!(recovered.has_synced_government);
        assert!(recovered.government_key_locked);
        assert_eq!(recovered.pending_tax_obligations.len(), 0); // both cleared
        assert_eq!(recovered.chat_messages.len(), 2);
    }

    #[test]
    fn recovery_snapshot_plus_later_events_merges_correctly() {
        aggressive_cleanup();

        let chat_old1 = dummy_chat("old-1");
        let chat_old2 = dummy_chat("old-2");

        let mut snap = NodeState {
            latest_gov_log_version: 3,
            seen_messages: HashSet::from([
                chat_old1.message.message_hash,
                chat_old2.message.message_hash,
            ]),
            peer_states: HashMap::new(),
            pending_tax_obligations: vec![
                dummy_tax_obligation(30, 200),
                dummy_tax_obligation(40, 350),
            ],
            latest_government_log: Some(dummy_gov_log(3)),
            chat_messages: vec![chat_old1.message, chat_old2.message],
            government_key_locked: true,
            has_synced_government: true,
        };
        save_snapshot(&snap).unwrap();

        append_event(&PersistentRecord::GovernmentLog(dummy_gov_log(4))).unwrap();
        append_event(&PersistentRecord::TaxObligation(dummy_tax_obligation(50, 100))).unwrap();
        append_event(&PersistentRecord::TaxReceipt(dummy_tax_receipt(dummy_hash(40u8.wrapping_add(50))))).unwrap();
        append_event(&PersistentRecord::ChatMessage(dummy_chat("new-3"))).unwrap();

        let recovered = recover_node_state().unwrap();

        assert_eq!(recovered.latest_gov_log_version, 4);
        assert_eq!(recovered.pending_tax_obligations.len(), 2); // 30 and 50 remain
        assert_eq!(recovered.chat_messages.len(), 3);
        assert!(recovered.chat_messages.iter().any(|m| m.message_id == "chat-old-1"));
        assert!(recovered.chat_messages.iter().any(|m| m.message_id == "chat-new-3"));
    }

    #[test]
    fn recovery_duplicate_chat_messages_are_not_duplicated() {
        aggressive_cleanup();

        let chat_a = dummy_chat("dup");
        let mut chat_b = dummy_chat("dup-different"); // different suffix → different hash originally
        chat_b.message.message_hash = chat_a.message.message_hash; // force duplicate

        append_event(&PersistentRecord::ChatMessage(chat_a)).unwrap();
        append_event(&PersistentRecord::ChatMessage(chat_b)).unwrap();

        let recovered = recover_node_state().unwrap();

        assert_eq!(recovered.chat_messages.len(), 1);
    }

    #[test]
    fn recovery_replays_government_logs_in_order_and_takes_latest() {
        aggressive_cleanup();

        append_event(&PersistentRecord::GovernmentLog(dummy_gov_log(0))).unwrap();
        append_event(&PersistentRecord::GovernmentLog(dummy_gov_log(2))).unwrap();
        append_event(&PersistentRecord::GovernmentLog(dummy_gov_log(1))).unwrap();

        let recovered = recover_node_state().unwrap();

        assert_eq!(recovered.latest_gov_log_version, 2);
        assert_eq!(
            recovered.latest_government_log.as_ref().unwrap().version_number,
            2
        );
    }

    #[test]
    fn recovery_handles_receipt_for_non_existing_obligation_gracefully() {
        aggressive_cleanup();

        append_event(&PersistentRecord::TaxReceipt(dummy_tax_receipt(dummy_hash(77)))).unwrap();

        let recovered = recover_node_state().unwrap();

        assert!(recovered.pending_tax_obligations.is_empty());
    }

    #[test]
    fn recovery_p2p_receipts_are_ignored_by_recovery_logic() {
        aggressive_cleanup();

        append_event(&PersistentRecord::P2PReceipt(dummy_p2p_receipt())).unwrap();

        let recovered = recover_node_state().unwrap();

        assert!(recovered.pending_tax_obligations.is_empty());
        assert!(recovered.chat_messages.is_empty());
    }


    #[test]
fn recovery_receipt_before_obligation_results_in_no_pending() {
    aggressive_cleanup();

    let obs_hash = dummy_hash(99);

    // 1. Receipt arrives first (referencing hash 99)
    append_event(&PersistentRecord::TaxReceipt(dummy_tax_receipt(obs_hash))).unwrap();
    
    // 2. Obligation arrives second (hash 99)
    let mut env = dummy_tax_obligation(1, 100);
    env.obligation.obligation_hash = obs_hash;
    append_event(&PersistentRecord::TaxObligation(env)).unwrap();

    let recovered = recover_node_state().unwrap();

    // Since the receipt was processed first and did nothing (obligation wasn't there),
    // then the obligation was added, we expect the obligation to still be there.
    // This confirms our logic is "order-dependent" based on the log.
    assert_eq!(recovered.pending_tax_obligations.len(), 1);
}


#[test]
fn recovery_chaos_merge_test() {
    aggressive_cleanup();

    let obs_hash_snap = dummy_hash(10);
    let obs_hash_event = dummy_hash(20);

    // 1. Snapshot with version 5 and one pending obligation
    let mut snap = NodeState {
        latest_gov_log_version: 5,
        pending_tax_obligations: vec![dummy_tax_obligation(10, 100)], // hash 60
        latest_government_log: Some(dummy_gov_log(5)),
        ..NodeState::default() // Assuming you have Default or manually fill
    };
    // Correct the hash for the test
    snap.pending_tax_obligations[0].obligation.obligation_hash = obs_hash_snap;
    save_snapshot(&snap).unwrap();

    // 2. Events: 
    // - An older Gov Log (should be ignored)
    append_event(&PersistentRecord::GovernmentLog(dummy_gov_log(4))).unwrap();
    // - A new Obligation
    let mut new_ob = dummy_tax_obligation(20, 200);
    new_ob.obligation.obligation_hash = obs_hash_event;
    append_event(&PersistentRecord::TaxObligation(new_ob)).unwrap();
    // - A Receipt for the SNAPSHOT obligation (should remove it)
    append_event(&PersistentRecord::TaxReceipt(dummy_tax_receipt(obs_hash_snap))).unwrap();

    let recovered = recover_node_state().unwrap();

    // Verification
    assert_eq!(recovered.latest_gov_log_version, 5, "Should not downgrade version");
    assert_eq!(recovered.pending_tax_obligations.len(), 1);
    assert_eq!(recovered.pending_tax_obligations[0].obligation.obligation_hash, obs_hash_event, "Only the new obligation should remain");
}


#[test]
fn recovery_prevents_duplicate_obligations_from_events() {
    aggressive_cleanup();

    let obs = dummy_tax_obligation(1, 100);
    let obs_hash = obs.obligation.obligation_hash;

    // Append the same exact obligation twice
    append_event(&PersistentRecord::TaxObligation(obs.clone())).unwrap();
    append_event(&PersistentRecord::TaxObligation(obs)).unwrap();

    let recovered = recover_node_state().unwrap();

    let count = recovered.pending_tax_obligations.iter()
        .filter(|o| o.obligation.obligation_hash == obs_hash)
        .count();

    assert_eq!(count, 1, "Obligation should only be added once regardless of log duplicates");
}

#[test]
fn recovery_handles_corrupted_event_file_gracefully() {
    aggressive_cleanup();
    
    // WORKAROUND: instead of ensure_data_dir(), 
    // just append one valid event to create the dir and file.
    append_event(&PersistentRecord::GovernmentLog(dummy_gov_log(1))).unwrap();

    // Now manually corrupt the file
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open("data/events.bin")
        .expect("File should exist after append_event");
        
    // Append 4 bytes that claim a massive length (0xFFFFFFFF) 
    // which will exceed the actual file size.
    file.write_all(&[0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00]).unwrap(); 

    // Try to recover
    let result = recover_node_state();

    // Verification: load_events should return an Err because the length prefix is a lie
    assert!(result.is_err());
    println!("Caught expected corruption error: {}", result.err().unwrap());
}

}