use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};
use postcard::{to_allocvec, from_bytes};

use crate::network::libp2p::NodeState;
use crate::types::{
    GovernmentRuleLog,
    TaxObligationEnvelope,
    TaxReceiptEnvelope,
    ReceiptEnvelope,
    ChatMessageEnvelope,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PersistentRecord {
    GovernmentLog(GovernmentRuleLog),
    TaxObligation(TaxObligationEnvelope),
    TaxReceipt(TaxReceiptEnvelope),
    P2PReceipt(ReceiptEnvelope),
    ChatMessage(ChatMessageEnvelope),
}

const SNAPSHOT_FILE: &str = "data/snapshot.bin";
const EVENTS_FILE: &str = "data/events.bin";

fn ensure_data_dir() -> Result<()> {
    if !Path::new("data").exists() {
        fs::create_dir_all("data")?;
    }
    Ok(())
}

pub fn save_snapshot(state: &NodeState) -> Result<()> {
    ensure_data_dir()?;

    // Postcard handles the serialization using standard Serde traits
    let encoded = to_allocvec(state)
        .map_err(|e| anyhow!("Snapshot serialize failed: {}", e))?;

    let mut file = File::create(SNAPSHOT_FILE)?;
    file.write_all(&encoded)?;
    file.sync_all()?;

    Ok(())
}

pub fn load_snapshot() -> Result<Option<NodeState>> {
    if !Path::new(SNAPSHOT_FILE).exists() {
        return Ok(None);
    }

    let buf = fs::read(SNAPSHOT_FILE)?;
    let state = from_bytes(&buf)
        .map_err(|e| anyhow!("Snapshot deserialize failed: {}", e))?;

    Ok(Some(state))
}

pub fn append_event(event: &PersistentRecord) -> Result<()> {
    ensure_data_dir()?;

    let encoded = to_allocvec(event)
        .map_err(|e| anyhow!("Event serialize failed: {}", e))?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(EVENTS_FILE)?;

    // We still prefix with length so we can read the file as a stream of events
    let len = encoded.len() as u64;
    file.write_all(&len.to_le_bytes())?;
    file.write_all(&encoded)?;
    file.sync_all()?;

    Ok(())
}
pub fn load_events() -> Result<Vec<PersistentRecord>> {
    if !Path::new(EVENTS_FILE).exists() {
        return Ok(Vec::new());
    }

    let buf = fs::read(EVENTS_FILE)
        .map_err(|e| anyhow!("Cannot read events file: {}", e))?;

    if buf.is_empty() {
        return Ok(Vec::new());
    }

    let mut events = Vec::new();
    let mut cursor = 0usize;

    while cursor + 8 <= buf.len() {
        let len_bytes: [u8; 8] = buf[cursor..cursor + 8].try_into().unwrap();
        let len = u64::from_le_bytes(len_bytes) as usize;
        cursor += 8;

        if cursor + len > buf.len() {
            return Err(anyhow!(
                "Truncated event at offset {}: claimed len={}, remaining={}",
                cursor - 8,
                len,
                buf.len() - (cursor - 8)
            ));
        }

        match from_bytes::<PersistentRecord>(&buf[cursor..cursor + len]) {
            Ok(event) => {
                events.push(event);
                cursor += len;
            }
            Err(e) => {
                return Err(anyhow!(
                    "Deserialize failed at offset {} (len={}): {}",
                    cursor,
                    len,
                    e
                ));
            }
        }
    }

    if cursor != buf.len() {
        return Err(anyhow!("Extra {} bytes after last event", buf.len() - cursor));
    }

    Ok(events)
}
pub fn clear_events() -> Result<()> {
    if Path::new(EVENTS_FILE).exists() {
        fs::remove_file(EVENTS_FILE)?;
    }
    Ok(())
}


#[cfg(test)]
mod persistence_tests {
    use crate::types::{ChatMessage, HASH, P2PTransferReceipt, RealmId, ResourceDelta, Resources, SIGNATURE, TaxObligation, TaxReceipt};
    use ed25519_dalek::VerifyingKey as PublicKey;


    use super::*;
    use std::collections::{HashMap, HashSet, BTreeMap};
    use std::time::SystemTime;
    use ed25519_dalek::VerifyingKey;
    
    // Helper functions to create test data
    
    fn create_test_realm_id() -> RealmId {
        [1u8; 32]
    }
    
    fn create_test_hash(seed: u8) -> HASH {
        [seed; 32]
    }
    
    fn create_test_signature(seed: u8) -> SIGNATURE {
        [seed; 64]
    }
    
    fn create_test_public_key(seed: u8) -> PublicKey {
    use ed25519_dalek::SigningKey;
    
    let mut seed_bytes = [0u8; 32];
    seed_bytes[0] = seed;
    let signing_key = SigningKey::from_bytes(&seed_bytes);
    signing_key.verifying_key()
}
    
   fn create_test_government_log(version: u64) -> GovernmentRuleLog {
    GovernmentRuleLog {
        version_number: version,
        effective_slot: 100 + version,
        rules: BTreeMap::new(),
        tasks: Vec::new(),
        tax_rate_percent: 10,
        tax_deadline: 1000,
        previous_log_hash: if version > 0 {
            Some(create_test_hash(((version - 1) % 256) as u8))
        } else {
            None
        },
        log_hash: create_test_hash((version % 256) as u8),
        government_signature: create_test_signature((version % 256) as u8),
        realm_id: create_test_realm_id(),
        government_public_key: create_test_public_key(1),
    }
}#[test]
fn test_diagnostic_file_persistence() {
    cleanup_test_files();
    
    println!("=== Starting diagnostic test ===");
    
    // Create and save a simple state
    let state = NodeState {
        latest_gov_log_version: 42,
        seen_messages: HashSet::new(),
        peer_states: HashMap::new(),
        pending_tax_obligations: Vec::new(),
        latest_government_log: None,
        chat_messages: Vec::new(),
        government_key_locked: false,
        has_synced_government: false,
    };
    
    println!("Saving snapshot...");
    save_snapshot(&state).unwrap();
    
    // Check if file exists
    println!("Checking if snapshot file exists...");
    assert!(Path::new("data/snapshot.bin").exists(), "Snapshot file was not created!");
    
    let file_size = std::fs::metadata("data/snapshot.bin").unwrap().len();
    println!("Snapshot file size: {} bytes", file_size);
    assert!(file_size > 0, "Snapshot file is empty!");
    
    // Load and verify
    println!("Loading snapshot...");
    let loaded = load_snapshot().unwrap();
    assert!(loaded.is_some(), "Loaded snapshot is None!");
    
    let loaded_state = loaded.unwrap();
    println!("Loaded state version: {}", loaded_state.latest_gov_log_version);
    assert_eq!(loaded_state.latest_gov_log_version, 42, "Version mismatch!");
    
    // Test events
    println!("\n=== Testing events ===");
    let gov_log = create_test_government_log(1);
    let event = PersistentRecord::GovernmentLog(gov_log);
    
    println!("Appending event...");
    append_event(&event).unwrap();
    
    println!("Checking if events file exists...");
    assert!(Path::new("data/events.bin").exists(), "Events file was not created!");
    
    let events_size = std::fs::metadata("data/events.bin").unwrap().len();
    println!("Events file size: {} bytes", events_size);
    assert!(events_size > 0, "Events file is empty!");
    
    println!("Loading events...");
    let loaded_events = load_events().unwrap();
    println!("Loaded {} events", loaded_events.len());
    assert_eq!(loaded_events.len(), 1, "Expected 1 event!");
    
    println!("\n=== Test passed! Files are being created correctly ===");
    
    // Don't cleanup so we can inspect the files
    println!("\nFiles left for inspection:");
    println!("  - data/snapshot.bin ({} bytes)", file_size);
    println!("  - data/events.bin ({} bytes)", events_size);
}


    
    fn create_test_tax_obligation() -> TaxObligation {
        TaxObligation {
            realm_id: create_test_realm_id(),
            event_hash: create_test_hash(10),
            user_id: create_test_public_key(5),
            coin_amount: 100,
            government_log: create_test_government_log(1),
            sequence_number: 1,
            previous_receipt_hash: None,
            user_signature: create_test_signature(5),
            obligation_hash: create_test_hash(11),
        }
    }
    
    fn create_test_tax_obligation_envelope() -> TaxObligationEnvelope {
        TaxObligationEnvelope {
            realm_id: create_test_realm_id(),
            obligation: create_test_tax_obligation(),
            timestamp: 12345,
        }
    }
    
    fn create_test_tax_receipt() -> TaxReceipt {
        TaxReceipt {
            obligation_hash: create_test_hash(11),
            previous_receipt_hash: None,
            payment_slot: 200,
            coins_transferred: 100,
            government_signature: create_test_signature(1),
        }
    }
    
    fn create_test_tax_receipt_envelope() -> TaxReceiptEnvelope {
        TaxReceiptEnvelope {
            realm_id: create_test_realm_id(),
            receipt: create_test_tax_receipt(),
            timestamp: 12346,
        }
    }

    fn create_test_resources(wheat: u64, wood: u64, coins: u64) -> Resources {
    Resources { wheat, wood, coins }
}

fn create_test_resource_delta(wheat: i64, wood: i64, coins: i64) -> ResourceDelta {
    ResourceDelta { wheat, wood, coins }
}
    
fn create_test_p2p_receipt() -> P2PTransferReceipt {
    P2PTransferReceipt {
        realm_id: create_test_realm_id(),
        tx_hash: create_test_hash(20),
        sender: create_test_public_key(10),
        receiver: create_test_public_key(11),
        delta: create_test_resource_delta(0, 0, -50),  // Transfer 50 coins
        sender_prev_state_hash: create_test_hash(21),
        receiver_prev_state_hash: create_test_hash(22),
        sender_balance_before: create_test_resources(100, 100, 100),
        sender_balance_after: create_test_resources(100, 100, 50),   // Lost 50 coins
        receiver_balance_before: create_test_resources(50, 50, 50),
        receiver_balance_after: create_test_resources(50, 50, 100),  // Gained 50 coins
        nonce: 1,
        sender_signature: create_test_signature(10),
        receiver_signature: create_test_signature(11),
    }
}
    
    fn create_test_p2p_receipt_envelope() -> ReceiptEnvelope {
        ReceiptEnvelope {
            realm_id: create_test_realm_id(),
            receipt: create_test_p2p_receipt(),
            timestamp: 12347,
        }
    }
    
    fn create_test_chat_message() -> ChatMessage {
        ChatMessage {
            message_id: "msg_123".to_string(),
            sender: create_test_public_key(15),
            receiver: Some(create_test_public_key(16)),
            content: "Hello, world!".to_string(),
            nonce: 1,
            signature: create_test_signature(15),
            message_hash: create_test_hash(30),
        }
    }
    
    fn create_test_chat_message_envelope() -> ChatMessageEnvelope {
        ChatMessageEnvelope {
            realm_id: create_test_realm_id(),
            message: create_test_chat_message(),
            timestamp: 12348,
        }
    }
    
    fn create_test_node_state() -> NodeState {
        NodeState {
            latest_gov_log_version: 5,
            seen_messages: {
                let mut set = HashSet::new();
                set.insert(create_test_hash(1));
                set.insert(create_test_hash(2));
                set
            },
            peer_states: HashMap::new(),
            pending_tax_obligations: vec![
                create_test_tax_obligation_envelope(),
            ],
            latest_government_log: Some(create_test_government_log(5)),
            chat_messages: vec![
                create_test_chat_message(),
            ],
            government_key_locked: true,
            has_synced_government: true,
        }
    }
    
    // Setup and teardown helpers
    
    fn cleanup_test_files() {
        let _ = std::fs::remove_file("data/snapshot.bin");
        let _ = std::fs::remove_file("data/events.bin");
        let _ = std::fs::remove_dir("data");
    }
    
    // Test Suite
    
    #[test]
    fn test_ensure_data_dir_creates_directory() {
        cleanup_test_files();
        
        assert!(!Path::new("data").exists());
        ensure_data_dir().unwrap();
        assert!(Path::new("data").exists());
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_ensure_data_dir_idempotent() {
        cleanup_test_files();
        
        ensure_data_dir().unwrap();
        ensure_data_dir().unwrap(); // Should not fail if already exists
        assert!(Path::new("data").exists());
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_save_and_load_snapshot_empty_state() {
        cleanup_test_files();
        
        let state = NodeState {
            latest_gov_log_version: 0,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: None,
            chat_messages: Vec::new(),
            government_key_locked: false,
            has_synced_government: false,
        };
        
        save_snapshot(&state).unwrap();
        let loaded = load_snapshot().unwrap();
        
        assert!(loaded.is_some());
        let loaded_state = loaded.unwrap();
        assert_eq!(loaded_state.latest_gov_log_version, 0);
        assert_eq!(loaded_state.seen_messages.len(), 0);
        assert_eq!(loaded_state.pending_tax_obligations.len(), 0);
        assert!(loaded_state.latest_government_log.is_none());
        assert_eq!(loaded_state.chat_messages.len(), 0);
        assert!(!loaded_state.government_key_locked);
        assert!(!loaded_state.has_synced_government);
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_save_and_load_snapshot_with_data() {
        cleanup_test_files();
        
        let state = create_test_node_state();
        
        save_snapshot(&state).unwrap();
        let loaded = load_snapshot().unwrap();
        
        assert!(loaded.is_some());
        let loaded_state = loaded.unwrap();
        assert_eq!(loaded_state.latest_gov_log_version, 5);
        assert_eq!(loaded_state.seen_messages.len(), 2);
        assert_eq!(loaded_state.pending_tax_obligations.len(), 1);
        assert!(loaded_state.latest_government_log.is_some());
        assert_eq!(loaded_state.chat_messages.len(), 1);
        assert!(loaded_state.government_key_locked);
        assert!(loaded_state.has_synced_government);
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_load_snapshot_when_file_does_not_exist() {
        cleanup_test_files();
        
        let loaded = load_snapshot().unwrap();
        assert!(loaded.is_none());
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_save_snapshot_overwrites_existing() {
        cleanup_test_files();
        
        let state1 = NodeState {
            latest_gov_log_version: 1,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: None,
            chat_messages: Vec::new(),
            government_key_locked: false,
            has_synced_government: false,
        };
        
        save_snapshot(&state1).unwrap();
        
        let state2 = NodeState {
            latest_gov_log_version: 2,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: None,
            chat_messages: Vec::new(),
            government_key_locked: true,
            has_synced_government: true,
        };
        
        save_snapshot(&state2).unwrap();
        
        let loaded = load_snapshot().unwrap().unwrap();
        assert_eq!(loaded.latest_gov_log_version, 2);
        assert!(loaded.government_key_locked);
        assert!(loaded.has_synced_government);
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_append_government_log_event() {
        cleanup_test_files();
        
        let log = create_test_government_log(1);
        let event = PersistentRecord::GovernmentLog(log.clone());
        
        append_event(&event).unwrap();
        
        let events = load_events().unwrap();
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            PersistentRecord::GovernmentLog(loaded_log) => {
                assert_eq!(loaded_log.version_number, log.version_number);
                assert_eq!(loaded_log.realm_id, log.realm_id);
            }
            _ => panic!("Expected GovernmentLog"),
        }
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_append_tax_obligation_event() {
        cleanup_test_files();
        
        let envelope = create_test_tax_obligation_envelope();
        let event = PersistentRecord::TaxObligation(envelope.clone());
        
        append_event(&event).unwrap();
        
        let events = load_events().unwrap();
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            PersistentRecord::TaxObligation(loaded_envelope) => {
                assert_eq!(loaded_envelope.realm_id, envelope.realm_id);
                assert_eq!(loaded_envelope.timestamp, envelope.timestamp);
                assert_eq!(loaded_envelope.obligation.coin_amount, envelope.obligation.coin_amount);
            }
            _ => panic!("Expected TaxObligation"),
        }
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_append_tax_receipt_event() {
        cleanup_test_files();
        
        let envelope = create_test_tax_receipt_envelope();
        let event = PersistentRecord::TaxReceipt(envelope.clone());
        
        append_event(&event).unwrap();
        
        let events = load_events().unwrap();
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            PersistentRecord::TaxReceipt(loaded_envelope) => {
                assert_eq!(loaded_envelope.realm_id, envelope.realm_id);
                assert_eq!(loaded_envelope.receipt.coins_transferred, envelope.receipt.coins_transferred);
            }
            _ => panic!("Expected TaxReceipt"),
        }
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_append_p2p_receipt_event() {
        cleanup_test_files();
        
        let envelope = create_test_p2p_receipt_envelope();
        let event = PersistentRecord::P2PReceipt(envelope.clone());
        
        append_event(&event).unwrap();
        
        let events = load_events().unwrap();
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            PersistentRecord::P2PReceipt(loaded_envelope) => {
                assert_eq!(loaded_envelope.realm_id, envelope.realm_id);
                assert_eq!(loaded_envelope.receipt.tx_hash, envelope.receipt.tx_hash);
            }
            _ => panic!("Expected P2PReceipt"),
        }
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_append_chat_message_event() {
        cleanup_test_files();
        
        let envelope = create_test_chat_message_envelope();
        let event = PersistentRecord::ChatMessage(envelope.clone());
        
        append_event(&event).unwrap();
        
        let events = load_events().unwrap();
        assert_eq!(events.len(), 1);
        
        match &events[0] {
            PersistentRecord::ChatMessage(loaded_envelope) => {
                assert_eq!(loaded_envelope.realm_id, envelope.realm_id);
                assert_eq!(loaded_envelope.message.content, envelope.message.content);
                assert_eq!(loaded_envelope.message.message_id, envelope.message.message_id);
            }
            _ => panic!("Expected ChatMessage"),
        }
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_append_multiple_events() {
        cleanup_test_files();
        
        let gov_log = create_test_government_log(1);
        let tax_obligation = create_test_tax_obligation_envelope();
        let tax_receipt = create_test_tax_receipt_envelope();
        let p2p_receipt = create_test_p2p_receipt_envelope();
        let chat_message = create_test_chat_message_envelope();
        
        append_event(&PersistentRecord::GovernmentLog(gov_log)).unwrap();
        append_event(&PersistentRecord::TaxObligation(tax_obligation)).unwrap();
        append_event(&PersistentRecord::TaxReceipt(tax_receipt)).unwrap();
        append_event(&PersistentRecord::P2PReceipt(p2p_receipt)).unwrap();
        append_event(&PersistentRecord::ChatMessage(chat_message)).unwrap();
        
        let events = load_events().unwrap();
        assert_eq!(events.len(), 5);
        
        // Verify order is preserved
        assert!(matches!(events[0], PersistentRecord::GovernmentLog(_)));
        assert!(matches!(events[1], PersistentRecord::TaxObligation(_)));
        assert!(matches!(events[2], PersistentRecord::TaxReceipt(_)));
        assert!(matches!(events[3], PersistentRecord::P2PReceipt(_)));
        assert!(matches!(events[4], PersistentRecord::ChatMessage(_)));
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_load_events_when_file_does_not_exist() {
        cleanup_test_files();
        
        let events = load_events().unwrap();
        assert_eq!(events.len(), 0);
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_clear_events() {
        cleanup_test_files();
        
        let event = PersistentRecord::GovernmentLog(create_test_government_log(1));
        append_event(&event).unwrap();
        
        assert!(Path::new("data/events.bin").exists());
        
        clear_events().unwrap();
        
        assert!(!Path::new("data/events.bin").exists());
        
        let events = load_events().unwrap();
        assert_eq!(events.len(), 0);
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_clear_events_when_file_does_not_exist() {
        cleanup_test_files();
        
        // Should not fail if file doesn't exist
        clear_events().unwrap();
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_event_persistence_across_multiple_appends() {
        cleanup_test_files();
        
        // Append events in batches
        for i in 0..3 {
            let log = create_test_government_log(i);
            append_event(&PersistentRecord::GovernmentLog(log)).unwrap();
        }
        
        // Load and verify
        let events = load_events().unwrap();
        assert_eq!(events.len(), 3);
        
        for (i, event) in events.iter().enumerate() {
            match event {
                PersistentRecord::GovernmentLog(log) => {
                    assert_eq!(log.version_number, i as u64);
                }
                _ => panic!("Expected GovernmentLog"),
            }
        }
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_snapshot_and_events_independence() {
        cleanup_test_files();
        
        let state = create_test_node_state();
        save_snapshot(&state).unwrap();
        
        let event = PersistentRecord::GovernmentLog(create_test_government_log(10));
        append_event(&event).unwrap();
        
        // Verify both exist independently
        assert!(Path::new("data/snapshot.bin").exists());
        assert!(Path::new("data/events.bin").exists());
        
        let loaded_state = load_snapshot().unwrap().unwrap();
        assert_eq!(loaded_state.latest_gov_log_version, 5);
        
        let events = load_events().unwrap();
        assert_eq!(events.len(), 1);
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_large_state_snapshot() {
        cleanup_test_files();
        
        let mut state = NodeState {
            latest_gov_log_version: 1000,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: Some(create_test_government_log(1000)),
            chat_messages: Vec::new(),
            government_key_locked: true,
            has_synced_government: true,
        };
        
        // Add many seen messages
        for i in 0..1000 {
            state.seen_messages.insert(create_test_hash((i % 256) as u8));
        }
        
        // Add many chat messages
        for _ in 0..100 {
            state.chat_messages.push(create_test_chat_message());
        }
        
        save_snapshot(&state).unwrap();
        let loaded = load_snapshot().unwrap().unwrap();
        
        assert_eq!(loaded.latest_gov_log_version, 1000);
        assert!(loaded.seen_messages.len() > 0);
        assert_eq!(loaded.chat_messages.len(), 100);
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_many_events_append_and_load() {
        cleanup_test_files();
        
        let num_events = 100;
        
        for i in 0..num_events {
            let log = create_test_government_log(i);
            append_event(&PersistentRecord::GovernmentLog(log)).unwrap();
        }
        
        let events = load_events().unwrap();
        assert_eq!(events.len(), num_events as usize);
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_event_order_preservation() {
        cleanup_test_files();
        
        let events_to_append = vec![
            PersistentRecord::GovernmentLog(create_test_government_log(1)),
            PersistentRecord::TaxObligation(create_test_tax_obligation_envelope()),
            PersistentRecord::ChatMessage(create_test_chat_message_envelope()),
            PersistentRecord::TaxReceipt(create_test_tax_receipt_envelope()),
            PersistentRecord::P2PReceipt(create_test_p2p_receipt_envelope()),
        ];
        
        for event in &events_to_append {
            append_event(event).unwrap();
        }
        
        let loaded_events = load_events().unwrap();
        assert_eq!(loaded_events.len(), events_to_append.len());
        
        // Verify types are in the same order
        for (appended, loaded) in events_to_append.iter().zip(loaded_events.iter()) {
            assert_eq!(std::mem::discriminant(appended), std::mem::discriminant(loaded));
        }
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_snapshot_file_corruption_handling() {
        cleanup_test_files();
        
        ensure_data_dir().unwrap();
        
        // Write invalid data to snapshot file
        let mut file = File::create("data/snapshot.bin").unwrap();
        file.write_all(b"invalid postcard data").unwrap();
        
        // Should return error, not panic
        let result = load_snapshot();
        assert!(result.is_err());
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_events_file_corruption_handling() {
        cleanup_test_files();
        
        ensure_data_dir().unwrap();
        
        // Write invalid data to events file
        let mut file = File::create("data/events.bin").unwrap();
        file.write_all(b"invalid event data").unwrap();
        
        // Should handle gracefully
        let result = load_events();
        // Depending on implementation, might return empty or error
        // Either is acceptable for corrupted data
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_partial_event_write_handling() {
        cleanup_test_files();
        
        let event = PersistentRecord::GovernmentLog(create_test_government_log(1));
        append_event(&event).unwrap();
        
        // Corrupt the file by truncating it
        let metadata = std::fs::metadata("data/events.bin").unwrap();
        let file_size = metadata.len();
        let file = OpenOptions::new()
            .write(true)
            .open("data/events.bin")
            .unwrap();
        file.set_len(file_size / 2).unwrap(); // Truncate to half size
        
        // Should handle incomplete events gracefully
        let events = load_events().unwrap_or(Vec::new());
        // Should either load nothing or handle the partial data
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_concurrent_snapshot_writes_simulation() {
        cleanup_test_files();
        
        // Simulate sequential writes (real concurrency would need proper test framework)
        let state1 = create_test_node_state();
        let mut state2 = create_test_node_state();
        state2.latest_gov_log_version = 10;
        
        save_snapshot(&state1).unwrap();
        save_snapshot(&state2).unwrap();
        
        let loaded = load_snapshot().unwrap().unwrap();
        assert_eq!(loaded.latest_gov_log_version, 10); // Last write wins
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_event_data_integrity() {
        cleanup_test_files();
        
        let original_log = create_test_government_log(42);
        let event = PersistentRecord::GovernmentLog(original_log.clone());
        
        append_event(&event).unwrap();
        
        let events = load_events().unwrap();
        match &events[0] {
            PersistentRecord::GovernmentLog(loaded_log) => {
                // Verify all fields match
                assert_eq!(loaded_log.version_number, original_log.version_number);
                assert_eq!(loaded_log.effective_slot, original_log.effective_slot);
                assert_eq!(loaded_log.tax_rate_percent, original_log.tax_rate_percent);
                assert_eq!(loaded_log.tax_deadline, original_log.tax_deadline);
                assert_eq!(loaded_log.log_hash, original_log.log_hash);
                assert_eq!(loaded_log.government_signature, original_log.government_signature);
                assert_eq!(loaded_log.realm_id, original_log.realm_id);
                assert_eq!(loaded_log.previous_log_hash, original_log.previous_log_hash);
            }
            _ => panic!("Expected GovernmentLog"),
        }
        
        cleanup_test_files();
    }
}




#[cfg(test)]
mod persistence_integration_tests {
    use crate::types::{ChatMessage, HASH, P2PTransferReceipt, RealmId, ResourceDelta, Resources, SIGNATURE, TaxObligation, TaxReceipt};

    use super::*;
    use std::collections::{BTreeMap, HashMap, HashSet};
    use ed25519_dalek::VerifyingKey as PublicKey;
    use ed25519_dalek::VerifyingKey;

    
    // Helper functions
    
    fn cleanup_test_files() {
        let _ = std::fs::remove_file("data/snapshot.bin");
        let _ = std::fs::remove_file("data/events.bin");
        let _ = std::fs::remove_dir("data");
    }
    
    fn create_test_realm_id() -> RealmId {
        [1u8; 32]
    }
    
    fn create_test_hash(seed: u8) -> HASH {
        [seed; 32]
    }
    
    fn create_test_signature(seed: u8) -> SIGNATURE {
        [seed; 64]
    }
    
    fn create_test_public_key(seed: u8) -> PublicKey {
    use ed25519_dalek::SigningKey;
    
    let mut seed_bytes = [0u8; 32];
    seed_bytes[0] = seed;
    let signing_key = SigningKey::from_bytes(&seed_bytes);
    signing_key.verifying_key()
}
 fn create_test_government_log(version: u64) -> GovernmentRuleLog {
    GovernmentRuleLog {
        version_number: version,
        effective_slot: 100 + version,
        rules: BTreeMap::new(),
        tasks: Vec::new(),
        tax_rate_percent: 10,
        tax_deadline: 1000,
        previous_log_hash: if version > 0 {
            Some(create_test_hash(((version - 1) % 256) as u8))
        } else {
            None
        },
        log_hash: create_test_hash((version % 256) as u8),
        government_signature: create_test_signature((version % 256) as u8),
        realm_id: create_test_realm_id(),
        government_public_key: create_test_public_key(1),
    }
}
    
    #[test]
    fn test_recovery_workflow() {
        cleanup_test_files();
        
        // Simulate node operation with state changes
        let mut state = NodeState {
            latest_gov_log_version: 0,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: None,
            chat_messages: Vec::new(),
            government_key_locked: false,
            has_synced_government: false,
        };
        
        // Process government log
        let gov_log = create_test_government_log(1);
        state.latest_gov_log_version = 1;
        state.latest_government_log = Some(gov_log.clone());
        state.government_key_locked = true;
        state.has_synced_government = true;
        
        // Save snapshot
        save_snapshot(&state).unwrap();
        
        // Log the event
        append_event(&PersistentRecord::GovernmentLog(gov_log)).unwrap();
        
        // Simulate crash and recovery
        let recovered_state = load_snapshot().unwrap().unwrap();
        let events = load_events().unwrap();
        
        assert_eq!(recovered_state.latest_gov_log_version, 1);
        assert!(recovered_state.government_key_locked);
        assert!(recovered_state.has_synced_government);
        assert_eq!(events.len(), 1);
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_event_replay_workflow() {
        cleanup_test_files();
        
        // Start with empty state
        let mut state = NodeState {
            latest_gov_log_version: 0,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: None,
            chat_messages: Vec::new(),
            government_key_locked: false,
            has_synced_government: false,
        };
        
        // Process multiple events
        let events = vec![
            PersistentRecord::GovernmentLog(create_test_government_log(1)),
            PersistentRecord::GovernmentLog(create_test_government_log(2)),
            PersistentRecord::GovernmentLog(create_test_government_log(3)),
        ];
        
        for event in &events {
            append_event(event).unwrap();
            if let PersistentRecord::GovernmentLog(log) = event {
                state.latest_gov_log_version = log.version_number;
                state.latest_government_log = Some(log.clone());
            }
        }
        
        // Save final state
        save_snapshot(&state).unwrap();
        
        // Verify we can replay events
        let loaded_events = load_events().unwrap();
        assert_eq!(loaded_events.len(), 3);
        
        // Simulate rebuilding state from events
        let mut rebuilt_version = 0;
        for event in loaded_events {
            if let PersistentRecord::GovernmentLog(log) = event {
                rebuilt_version = log.version_number;
            }
        }
        
        assert_eq!(rebuilt_version, 3);
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_snapshot_and_incremental_events() {
        cleanup_test_files();
        
        // Create initial state with some data
        let mut state = NodeState {
            latest_gov_log_version: 5,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: Some(create_test_government_log(5)),
            chat_messages: Vec::new(),
            government_key_locked: true,
            has_synced_government: true,
        };
        
        // Save snapshot
        save_snapshot(&state).unwrap();
        
        // Now append incremental events (versions 6, 7, 8)
        for version in 6..=8 {
            let log = create_test_government_log(version);
            append_event(&PersistentRecord::GovernmentLog(log.clone())).unwrap();
            state.latest_gov_log_version = version;
            state.latest_government_log = Some(log);
        }
        
        // Load snapshot and events
        let loaded_state = load_snapshot().unwrap().unwrap();
        let events = load_events().unwrap();
        
        // State should be at version 5 (snapshot)
        assert_eq!(loaded_state.latest_gov_log_version, 5);
        
        // But we have events for versions 6, 7, 8
        assert_eq!(events.len(), 3);
        
        // Apply events to reach version 8
        let mut current_version = loaded_state.latest_gov_log_version;
        for event in events {
            if let PersistentRecord::GovernmentLog(log) = event {
                current_version = log.version_number;
            }
        }
        assert_eq!(current_version, 8);
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_deduplication_with_seen_messages() {
        cleanup_test_files();
        
        let mut state = NodeState {
            latest_gov_log_version: 0,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: None,
            chat_messages: Vec::new(),
            government_key_locked: false,
            has_synced_government: false,
        };
        
        // Add some seen message hashes
        for i in 0..10 {
            state.seen_messages.insert(create_test_hash(i));
        }
        
        save_snapshot(&state).unwrap();
        
        // Load and verify seen messages are preserved
        let loaded = load_snapshot().unwrap().unwrap();
        assert_eq!(loaded.seen_messages.len(), 10);
        
        for i in 0..10 {
            assert!(loaded.seen_messages.contains(&create_test_hash(i)));
        }
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_pending_tax_obligations_persistence() {
        cleanup_test_files();
        
        let obligation1 = TaxObligationEnvelope {
            realm_id: create_test_realm_id(),
            obligation: TaxObligation {
                realm_id: create_test_realm_id(),
                event_hash: create_test_hash(1),
                user_id: create_test_public_key(1),
                coin_amount: 100,
                government_log: create_test_government_log(1),
                sequence_number: 1,
                previous_receipt_hash: None,
                user_signature: create_test_signature(1),
                obligation_hash: create_test_hash(10),
            },
            timestamp: 1000,
        };
        
        let obligation2 = TaxObligationEnvelope {
            realm_id: create_test_realm_id(),
            obligation: TaxObligation {
                realm_id: create_test_realm_id(),
                event_hash: create_test_hash(2),
                user_id: create_test_public_key(2),
                coin_amount: 200,
                government_log: create_test_government_log(1),
                sequence_number: 2,
                previous_receipt_hash: None,
                user_signature: create_test_signature(2),
                obligation_hash: create_test_hash(11),
            },
            timestamp: 2000,
        };
        
        let state = NodeState {
            latest_gov_log_version: 1,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: vec![obligation1.clone(), obligation2.clone()],
            latest_government_log: Some(create_test_government_log(1)),
            chat_messages: Vec::new(),
            government_key_locked: true,
            has_synced_government: true,
        };
        
        save_snapshot(&state).unwrap();
        
        let loaded = load_snapshot().unwrap().unwrap();
        assert_eq!(loaded.pending_tax_obligations.len(), 2);
        assert_eq!(loaded.pending_tax_obligations[0].obligation.coin_amount, 100);
        assert_eq!(loaded.pending_tax_obligations[1].obligation.coin_amount, 200);
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_chat_messages_persistence() {
        cleanup_test_files();
        
        let messages = vec![
            ChatMessage {
                message_id: "msg1".to_string(),
                sender: create_test_public_key(1),
                receiver: Some(create_test_public_key(2)),
                content: "Hello".to_string(),
                nonce: 1,
                signature: create_test_signature(1),
                message_hash: create_test_hash(1),
            },
            ChatMessage {
                message_id: "msg2".to_string(),
                sender: create_test_public_key(2),
                receiver: Some(create_test_public_key(1)),
                content: "World".to_string(),
                nonce: 2,
                signature: create_test_signature(2),
                message_hash: create_test_hash(2),
            },
        ];
        
        let state = NodeState {
            latest_gov_log_version: 0,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: None,
            chat_messages: messages.clone(),
            government_key_locked: false,
            has_synced_government: false,
        };
        
        save_snapshot(&state).unwrap();
        
        let loaded = load_snapshot().unwrap().unwrap();
        assert_eq!(loaded.chat_messages.len(), 2);
        assert_eq!(loaded.chat_messages[0].content, "Hello");
        assert_eq!(loaded.chat_messages[1].content, "World");
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_full_economic_cycle_persistence() {
        cleanup_test_files();
        
        // Simulate full economic cycle
        let mut events = Vec::new();
        
        // 1. Government log
        let gov_log = create_test_government_log(1);
        events.push(PersistentRecord::GovernmentLog(gov_log.clone()));
        
        // 2. Tax obligation
        let tax_obligation = TaxObligationEnvelope {
            realm_id: create_test_realm_id(),
            obligation: TaxObligation {
                realm_id: create_test_realm_id(),
                event_hash: create_test_hash(5),
                user_id: create_test_public_key(5),
                coin_amount: 100,
                government_log: gov_log.clone(),
                sequence_number: 1,
                previous_receipt_hash: None,
                user_signature: create_test_signature(5),
                obligation_hash: create_test_hash(10),
            },
            timestamp: 1000,
        };
        events.push(PersistentRecord::TaxObligation(tax_obligation));
        
        // 3. Tax receipt
        let tax_receipt = TaxReceiptEnvelope {
            realm_id: create_test_realm_id(),
            receipt: TaxReceipt {
                obligation_hash: create_test_hash(10),
                previous_receipt_hash: None,
                payment_slot: 200,
                coins_transferred: 100,
                government_signature: create_test_signature(1),
            },
            timestamp: 2000,
        };
        events.push(PersistentRecord::TaxReceipt(tax_receipt));
        
        // 4. P2P transfer
        // 4. P2P transfer
        let p2p_receipt = ReceiptEnvelope {
            realm_id: create_test_realm_id(),
            receipt: P2PTransferReceipt {
                realm_id: create_test_realm_id(),
                tx_hash: create_test_hash(20),
                sender: create_test_public_key(10),
                receiver: create_test_public_key(11),
                delta: create_test_resource_delta(0, 0, -50),
                sender_prev_state_hash: create_test_hash(21),
                receiver_prev_state_hash: create_test_hash(22),
                sender_balance_before: create_test_resources(100, 100, 100),
                sender_balance_after: create_test_resources(100, 100, 50),
                receiver_balance_before: create_test_resources(50, 50, 50),
                receiver_balance_after: create_test_resources(50, 50, 100),
                nonce: 1,
                sender_signature: create_test_signature(10),
                receiver_signature: create_test_signature(11),
            },
            timestamp: 3000,
        };
        events.push(PersistentRecord::P2PReceipt(p2p_receipt));
        
        // 5. Chat message
        let chat_message = ChatMessageEnvelope {
            realm_id: create_test_realm_id(),
            message: ChatMessage {
                message_id: "msg_tx_complete".to_string(),
                sender: create_test_public_key(10),
                receiver: Some(create_test_public_key(11)),
                content: "Payment sent!".to_string(),
                nonce: 1,
                signature: create_test_signature(10),
                message_hash: create_test_hash(30),
            },
            timestamp: 3100,
        };
        events.push(PersistentRecord::ChatMessage(chat_message));
        
        // Append all events
        for event in &events {
            append_event(event).unwrap();
        }
        
        // Load and verify
        let loaded_events = load_events().unwrap();
        assert_eq!(loaded_events.len(), 5);
        
        // Verify event types in order
        assert!(matches!(loaded_events[0], PersistentRecord::GovernmentLog(_)));
        assert!(matches!(loaded_events[1], PersistentRecord::TaxObligation(_)));
        assert!(matches!(loaded_events[2], PersistentRecord::TaxReceipt(_)));
        assert!(matches!(loaded_events[3], PersistentRecord::P2PReceipt(_)));
        assert!(matches!(loaded_events[4], PersistentRecord::ChatMessage(_)));
        
        cleanup_test_files();
    }

    fn create_test_resources(wheat: u64, wood: u64, coins: u64) -> Resources {
    Resources { wheat, wood, coins }
}

fn create_test_resource_delta(wheat: i64, wood: i64, coins: i64) -> ResourceDelta {
    ResourceDelta { wheat, wood, coins }
}
    
    #[test]
    fn test_state_consistency_after_snapshot_clear_and_rebuild() {
        cleanup_test_files();
        
        // Create and save initial state
        let initial_state = NodeState {
            latest_gov_log_version: 10,
            seen_messages: {
                let mut set = HashSet::new();
                set.insert(create_test_hash(1));
                set.insert(create_test_hash(2));
                set
            },
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: Some(create_test_government_log(10)),
            chat_messages: Vec::new(),
            government_key_locked: true,
            has_synced_government: true,
        };
        
        save_snapshot(&initial_state).unwrap();
        
        // Add new events
        for version in 11..=15 {
            append_event(&PersistentRecord::GovernmentLog(
                create_test_government_log(version)
            )).unwrap();
        }
        
        // Clear events and create new snapshot
        clear_events().unwrap();
        
        let updated_state = NodeState {
            latest_gov_log_version: 15,
            seen_messages: initial_state.seen_messages.clone(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: Some(create_test_government_log(15)),
            chat_messages: Vec::new(),
            government_key_locked: true,
            has_synced_government: true,
        };
        
        save_snapshot(&updated_state).unwrap();
        
        // Verify final state
        let final_state = load_snapshot().unwrap().unwrap();
        assert_eq!(final_state.latest_gov_log_version, 15);
        
        let final_events = load_events().unwrap();
        assert_eq!(final_events.len(), 0); // Events were cleared
        
        cleanup_test_files();
    }
    
    #[test]
    fn test_node_restart_simulation() {
        cleanup_test_files();
        
        // Simulate node startup
        let mut state = NodeState {
            latest_gov_log_version: 0,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: None,
            chat_messages: Vec::new(),
            government_key_locked: false,
            has_synced_government: false,
        };
        
        // Node receives and processes government log
        let gov_log = create_test_government_log(1);
        state.latest_government_log = Some(gov_log.clone());
        state.latest_gov_log_version = 1;
        state.government_key_locked = true;
        state.has_synced_government = true;
        
        append_event(&PersistentRecord::GovernmentLog(gov_log)).unwrap();
        save_snapshot(&state).unwrap();
        
        // Simulate node restart - load state
        let recovered_state = load_snapshot().unwrap();
        assert!(recovered_state.is_some());
        
        let mut restarted_state = recovered_state.unwrap();
        assert_eq!(restarted_state.latest_gov_log_version, 1);
        assert!(restarted_state.has_synced_government);
        
        // Node continues operation - receives new government log
        let gov_log_2 = create_test_government_log(2);
        restarted_state.latest_government_log = Some(gov_log_2.clone());
        restarted_state.latest_gov_log_version = 2;
        
        append_event(&PersistentRecord::GovernmentLog(gov_log_2)).unwrap();
        save_snapshot(&restarted_state).unwrap();
        
        // Verify final state
        let final_state = load_snapshot().unwrap().unwrap();
        assert_eq!(final_state.latest_gov_log_version, 2);
        
        cleanup_test_files();
    }

    #[test]
fn test_diagnostic_file_persistence() {
    cleanup_test_files();
    
    println!("=== Starting diagnostic test ===");
    
    // Create and save a simple state
    let state = NodeState {
        latest_gov_log_version: 42,
        seen_messages: HashSet::new(),
        peer_states: HashMap::new(),
        pending_tax_obligations: Vec::new(),
        latest_government_log: None,
        chat_messages: Vec::new(),
        government_key_locked: false,
        has_synced_government: false,
    };
    
    println!("Saving snapshot...");
    save_snapshot(&state).unwrap();
    
    // Check if file exists
    println!("Checking if snapshot file exists...");
    assert!(Path::new("data/snapshot.bin").exists(), "Snapshot file was not created!");
    
    let file_size = std::fs::metadata("data/snapshot.bin").unwrap().len();
    println!("Snapshot file size: {} bytes", file_size);
    assert!(file_size > 0, "Snapshot file is empty!");
    
    // Load and verify
    println!("Loading snapshot...");
    let loaded = load_snapshot().unwrap();
    assert!(loaded.is_some(), "Loaded snapshot is None!");
    
    let loaded_state = loaded.unwrap();
    println!("Loaded state version: {}", loaded_state.latest_gov_log_version);
    assert_eq!(loaded_state.latest_gov_log_version, 42, "Version mismatch!");
    
    // Test events
    println!("\n=== Testing events ===");
    let gov_log = create_test_government_log(1);
    let event = PersistentRecord::GovernmentLog(gov_log);
    
    println!("Appending event...");
    append_event(&event).unwrap();
    
    println!("Checking if events file exists...");
    assert!(Path::new("data/events.bin").exists(), "Events file was not created!");
    
    let events_size = std::fs::metadata("data/events.bin").unwrap().len();
    println!("Events file size: {} bytes", events_size);
    assert!(events_size > 0, "Events file is empty!");
    
    println!("Loading events...");
    let loaded_events = load_events().unwrap();
    println!("Loaded {} events", loaded_events.len());
    assert_eq!(loaded_events.len(), 1, "Expected 1 event!");
    
    println!("\n=== Test passed! Files are being created correctly ===");
    
    // Don't cleanup so we can inspect the files
    println!("\nFiles left for inspection:");
    println!("  - data/snapshot.bin ({} bytes)", file_size);
    println!("  - data/events.bin ({} bytes)", events_size);
}
}





#[cfg(test)]
mod verification_test {
    use crate::types::{ChatMessage, HASH, P2PTransferReceipt, RealmId, ResourceDelta, Resources, SIGNATURE, TaxObligation, TaxReceipt};

    use super::*;
    use std::collections::{HashMap, HashSet, BTreeMap};
    use std::path::Path;
    use ed25519_dalek::SigningKey;
    use ed25519_dalek::VerifyingKey as PublicKey;

    
    // Helper functions
    fn create_test_realm_id() -> RealmId {
        [1u8; 32]
    }
    
    fn create_test_hash(seed: u8) -> HASH {
        [seed; 32]
    }
    
    fn create_test_signature(seed: u8) -> SIGNATURE {
        [seed; 64]
    }
    
    fn create_test_public_key(seed: u8) -> PublicKey {
        let mut seed_bytes = [0u8; 32];
        seed_bytes[0] = seed;
        let signing_key = SigningKey::from_bytes(&seed_bytes);
        signing_key.verifying_key()
    }
    
    fn create_test_government_log(version: u64) -> GovernmentRuleLog {
        GovernmentRuleLog {
            version_number: version,
            effective_slot: 100 + version,
            rules: BTreeMap::new(),
            tasks: Vec::new(),
            tax_rate_percent: 10,
            tax_deadline: 1000,
            previous_log_hash: if version > 0 {
                Some(create_test_hash(((version - 1) % 256) as u8))
            } else {
                None
            },
            log_hash: create_test_hash((version % 256) as u8),
            government_signature: create_test_signature((version % 256) as u8),
            realm_id: create_test_realm_id(),
            government_public_key: create_test_public_key(1),
        }
    }
    
    fn create_test_resources(wheat: u64, wood: u64, coins: u64) -> Resources {
        Resources { wheat, wood, coins }
    }
    
    fn create_test_resource_delta(wheat: i64, wood: i64, coins: i64) -> ResourceDelta {
        ResourceDelta { wheat, wood, coins }
    }
    
    fn create_test_tax_obligation() -> TaxObligation {
        TaxObligation {
            realm_id: create_test_realm_id(),
            event_hash: create_test_hash(10),
            user_id: create_test_public_key(5),
            coin_amount: 100,
            government_log: create_test_government_log(1),
            sequence_number: 1,
            previous_receipt_hash: None,
            user_signature: create_test_signature(5),
            obligation_hash: create_test_hash(11),
        }
    }
    
    fn create_test_chat_message() -> ChatMessage {
        ChatMessage {
            message_id: "msg_123".to_string(),
            sender: create_test_public_key(15),
            receiver: Some(create_test_public_key(16)),
            content: "Hello, world!".to_string(),
            nonce: 1,
            signature: create_test_signature(15),
            message_hash: create_test_hash(30),
        }
    }
    
    /// This test creates persistent files and DOES NOT clean them up
    /// Use this to manually verify file creation and inspect contents
    /// 
    /// To run: cargo test test_verify_file_persistence -- --nocapture
    /// Files will be left in: data/snapshot.bin and data/events.bin
    /// 
    /// To clean up manually: rm -rf data/
    #[test]
    #[ignore] // Ignored by default since it leaves files behind
    fn test_verify_file_persistence() {
        println!("\n┌─────────────────────────────────────────────────────────┐");
        println!("│  PERSISTENCE VERIFICATION TEST (Files NOT cleaned up)  │");
        println!("└─────────────────────────────────────────────────────────┘\n");
        
        // Clean up any existing files first
        let _ = std::fs::remove_file("data/snapshot.bin");
        let _ = std::fs::remove_file("data/events.bin");
        let _ = std::fs::remove_dir("data");
        
        println!("📁 Creating test data directory...");
        ensure_data_dir().unwrap();
        assert!(Path::new("data").exists());
        println!("   ✓ Directory created\n");
        
        // ============================================================
        // PART 1: Test Snapshot Persistence
        // ============================================================
        println!("═══════════════════════════════════════════════════════════");
        println!("PART 1: SNAPSHOT PERSISTENCE");
        println!("═══════════════════════════════════════════════════════════\n");
        
        let mut state = NodeState {
            latest_gov_log_version: 42,
            seen_messages: {
                let mut set = HashSet::new();
                set.insert(create_test_hash(1));
                set.insert(create_test_hash(2));
                set.insert(create_test_hash(3));
                set
            },
            peer_states: HashMap::new(),
            pending_tax_obligations: vec![
                TaxObligationEnvelope {
                    realm_id: create_test_realm_id(),
                    obligation: create_test_tax_obligation(),
                    timestamp: 1000,
                }
            ],
            latest_government_log: Some(create_test_government_log(42)),
            chat_messages: vec![
                create_test_chat_message(),
            ],
            government_key_locked: true,
            has_synced_government: true,
        };
        
        println!("💾 Saving snapshot with:");
        println!("   - Gov log version: {}", state.latest_gov_log_version);
        println!("   - Seen messages: {}", state.seen_messages.len());
        println!("   - Pending obligations: {}", state.pending_tax_obligations.len());
        println!("   - Chat messages: {}", state.chat_messages.len());
        println!("   - Government key locked: {}", state.government_key_locked);
        
        save_snapshot(&state).unwrap();
        
        assert!(Path::new("data/snapshot.bin").exists(), "❌ Snapshot file was NOT created!");
        let snapshot_size = std::fs::metadata("data/snapshot.bin").unwrap().len();
        println!("   ✓ Snapshot saved ({} bytes)\n", snapshot_size);
        
        println!("📖 Loading snapshot...");
        let loaded = load_snapshot().unwrap();
        assert!(loaded.is_some(), "❌ Loaded snapshot is None!");
        
        let loaded_state = loaded.unwrap();
        println!("   ✓ Snapshot loaded successfully");
        println!("   - Gov log version: {}", loaded_state.latest_gov_log_version);
        println!("   - Seen messages: {}", loaded_state.seen_messages.len());
        println!("   - Pending obligations: {}", loaded_state.pending_tax_obligations.len());
        println!("   - Chat messages: {}", loaded_state.chat_messages.len());
        
        assert_eq!(loaded_state.latest_gov_log_version, 42);
        assert_eq!(loaded_state.seen_messages.len(), 3);
        assert_eq!(loaded_state.pending_tax_obligations.len(), 1);
        assert_eq!(loaded_state.chat_messages.len(), 1);
        assert!(loaded_state.government_key_locked);
        println!("   ✓ All fields verified\n");
        
        // ============================================================
        // PART 2: Test Event Log Persistence
        // ============================================================
        println!("═══════════════════════════════════════════════════════════");
        println!("PART 2: EVENT LOG PERSISTENCE");
        println!("═══════════════════════════════════════════════════════════\n");
        
        println!("📝 Appending events:");
        
        // Event 1: Government Log
        let gov_log = create_test_government_log(1);
        println!("   1. Government Log (version {})", gov_log.version_number);
        append_event(&PersistentRecord::GovernmentLog(gov_log)).unwrap();
        
        // Event 2: Tax Obligation
        let tax_obligation = TaxObligationEnvelope {
            realm_id: create_test_realm_id(),
            obligation: create_test_tax_obligation(),
            timestamp: 1000,
        };
        println!("   2. Tax Obligation ({} coins)", tax_obligation.obligation.coin_amount);
        append_event(&PersistentRecord::TaxObligation(tax_obligation)).unwrap();
        
        // Event 3: Tax Receipt
        let tax_receipt = TaxReceiptEnvelope {
            realm_id: create_test_realm_id(),
            receipt: TaxReceipt {
                obligation_hash: create_test_hash(10),
                previous_receipt_hash: None,
                payment_slot: 200,
                coins_transferred: 100,
                government_signature: create_test_signature(1),
            },
            timestamp: 2000,
        };
        println!("   3. Tax Receipt ({} coins)", tax_receipt.receipt.coins_transferred);
        append_event(&PersistentRecord::TaxReceipt(tax_receipt)).unwrap();
        
        // Event 4: P2P Transfer
        let p2p_receipt = ReceiptEnvelope {
            realm_id: create_test_realm_id(),
            receipt: P2PTransferReceipt {
                realm_id: create_test_realm_id(),
                tx_hash: create_test_hash(20),
                sender: create_test_public_key(10),
                receiver: create_test_public_key(11),
                delta: create_test_resource_delta(0, 0, -50),
                sender_prev_state_hash: create_test_hash(21),
                receiver_prev_state_hash: create_test_hash(22),
                sender_balance_before: create_test_resources(100, 100, 100),
                sender_balance_after: create_test_resources(100, 100, 50),
                receiver_balance_before: create_test_resources(50, 50, 50),
                receiver_balance_after: create_test_resources(50, 50, 100),
                nonce: 1,
                sender_signature: create_test_signature(10),
                receiver_signature: create_test_signature(11),
            },
            timestamp: 3000,
        };
        println!("   4. P2P Transfer (50 coins)");
        append_event(&PersistentRecord::P2PReceipt(p2p_receipt)).unwrap();
        
        // Event 5: Chat Message
        let chat_msg = ChatMessageEnvelope {
            realm_id: create_test_realm_id(),
            message: create_test_chat_message(),
            timestamp: 4000,
        };
        println!("   5. Chat Message (\"{}\")", chat_msg.message.content);
        append_event(&PersistentRecord::ChatMessage(chat_msg)).unwrap();
        
        assert!(Path::new("data/events.bin").exists(), "❌ Events file was NOT created!");
        let events_size = std::fs::metadata("data/events.bin").unwrap().len();
        println!("\n   ✓ All events appended ({} bytes)\n", events_size);
        
        println!("📖 Loading events...");
        let loaded_events = load_events().unwrap();
        println!("   ✓ Loaded {} events", loaded_events.len());
        assert_eq!(loaded_events.len(), 5);
        
        // Verify event types
        println!("\n   Verifying event types:");
        assert!(matches!(loaded_events[0], PersistentRecord::GovernmentLog(_)));
        println!("      [0] GovernmentLog ✓");
        
        assert!(matches!(loaded_events[1], PersistentRecord::TaxObligation(_)));
        println!("      [1] TaxObligation ✓");
        
        assert!(matches!(loaded_events[2], PersistentRecord::TaxReceipt(_)));
        println!("      [2] TaxReceipt ✓");
        
        assert!(matches!(loaded_events[3], PersistentRecord::P2PReceipt(_)));
        println!("      [3] P2PReceipt ✓");
        
        assert!(matches!(loaded_events[4], PersistentRecord::ChatMessage(_)));
        println!("      [4] ChatMessage ✓");
        
        // ============================================================
        // PART 3: Summary
        // ============================================================
        println!("\n═══════════════════════════════════════════════════════════");
        println!("SUMMARY");
        println!("═══════════════════════════════════════════════════════════\n");
        
        println!("✅ All persistence tests passed!\n");
        
        println!("📂 Files created (NOT cleaned up):");
        println!("   📄 data/snapshot.bin ({} bytes)", snapshot_size);
        println!("   📄 data/events.bin ({} bytes)\n", events_size);
        
        println!("🔍 To inspect files:");
        println!("   ls -lh data/");
        println!("   hexdump -C data/snapshot.bin | head");
        println!("   hexdump -C data/events.bin | head\n");
        
        println!("🧹 To clean up:");
        println!("   rm -rf data/\n");
        
        println!("═══════════════════════════════════════════════════════════\n");
        
        // DO NOT CLEANUP - Files left for inspection
        // This is intentional so you can verify persistence
    }
    
    /// Quick test that DOES clean up after itself
    /// Run this to verify basic functionality without leaving files
    #[test]
    fn test_quick_persistence_check() {
        // Clean up first
        let _ = std::fs::remove_file("data/snapshot.bin");
        let _ = std::fs::remove_file("data/events.bin");
        let _ = std::fs::remove_dir("data");
        
        // Test snapshot
        let state = NodeState {
            latest_gov_log_version: 99,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: None,
            chat_messages: Vec::new(),
            government_key_locked: false,
            has_synced_government: false,
        };
        
        save_snapshot(&state).unwrap();
        assert!(Path::new("data/snapshot.bin").exists());
        
        let loaded = load_snapshot().unwrap().unwrap();
        assert_eq!(loaded.latest_gov_log_version, 99);
        
        // Test events
        let event = PersistentRecord::GovernmentLog(create_test_government_log(1));
        append_event(&event).unwrap();
        assert!(Path::new("data/events.bin").exists());
        
        let events = load_events().unwrap();
        assert_eq!(events.len(), 1);
        
        // Clean up
        let _ = std::fs::remove_file("data/snapshot.bin");
        let _ = std::fs::remove_file("data/events.bin");
        let _ = std::fs::remove_dir("data");
    }
}


#[cfg(test)]
mod verification_tests_v2 {
    use crate::types::{ChatMessage, HASH, P2PTransferReceipt, RealmId, ResourceDelta, Resources, SIGNATURE, TaxObligation, TaxReceipt};
    use ed25519_dalek::VerifyingKey as PublicKey;

    use super::*;
    use std::collections::{HashMap, HashSet, BTreeMap};
    use std::path::Path;
    use ed25519_dalek::SigningKey;
    
    // Helper functions
    fn create_test_realm_id() -> RealmId {
        [1u8; 32]
    }
    
    fn create_test_hash(seed: u8) -> HASH {
        [seed; 32]
    }
    
    fn create_test_signature(seed: u8) -> SIGNATURE {
        [seed; 64]
    }
    
    fn create_test_public_key(seed: u8) -> PublicKey {
        let mut seed_bytes = [0u8; 32];
        seed_bytes[0] = seed;
        let signing_key = SigningKey::from_bytes(&seed_bytes);
        signing_key.verifying_key()
    }
    
    fn create_test_government_log(version: u64) -> GovernmentRuleLog {
        GovernmentRuleLog {
            version_number: version,
            effective_slot: 100 + version,
            rules: BTreeMap::new(),
            tasks: Vec::new(),
            tax_rate_percent: 10,
            tax_deadline: 1000,
            previous_log_hash: if version > 0 {
                Some(create_test_hash(((version - 1) % 256) as u8))
            } else {
                None
            },
            log_hash: create_test_hash((version % 256) as u8),
            government_signature: create_test_signature((version % 256) as u8),
            realm_id: create_test_realm_id(),
            government_public_key: create_test_public_key(1),
        }
    }
    
    fn create_test_resources(wheat: u64, wood: u64, coins: u64) -> Resources {
        Resources { wheat, wood, coins }
    }
    
    fn create_test_resource_delta(wheat: i64, wood: i64, coins: i64) -> ResourceDelta {
        ResourceDelta { wheat, wood, coins }
    }
    
    fn create_test_tax_obligation() -> TaxObligation {
        TaxObligation {
            realm_id: create_test_realm_id(),
            event_hash: create_test_hash(10),
            user_id: create_test_public_key(5),
            coin_amount: 100,
            government_log: create_test_government_log(1),
            sequence_number: 1,
            previous_receipt_hash: None,
            user_signature: create_test_signature(5),
            obligation_hash: create_test_hash(11),
        }
    }
    
    fn create_test_chat_message() -> ChatMessage {
        ChatMessage {
            message_id: "msg_123".to_string(),
            sender: create_test_public_key(15),
            receiver: Some(create_test_public_key(16)),
            content: "Hello, world!".to_string(),
            nonce: 1,
            signature: create_test_signature(15),
            message_hash: create_test_hash(30),
        }
    }
    
    /// This test creates multiple snapshots and verifies state evolution
    /// Files are NOT cleaned up so you can inspect them
    /// 
    /// To run: cargo test test_verify_file_persistence -- --nocapture --ignored
    /// Files will be left in: data/snapshot_*.bin and data/events.bin
    /// 
    /// To clean up manually: rm -rf data/
    #[test]
    #[ignore] // Ignored by default since it leaves files behind
    fn test_verify_file_persistence() {
        println!("\n┌──────────────────────────────────────────────────────────────────┐");
        println!("│  MULTI-VERSION STATE PERSISTENCE VERIFICATION (NO CLEANUP)       │");
        println!("└──────────────────────────────────────────────────────────────────┘\n");
        
        // Clean up any existing files first
        let _ = std::fs::remove_file("data/snapshot.bin");
        let _ = std::fs::remove_file("data/snapshot_v1.bin");
        let _ = std::fs::remove_file("data/snapshot_v2.bin");
        let _ = std::fs::remove_file("data/snapshot_v3.bin");
        let _ = std::fs::remove_file("data/snapshot_v4.bin");
        let _ = std::fs::remove_file("data/snapshot_v5.bin");
        let _ = std::fs::remove_file("data/events.bin");
        let _ = std::fs::remove_dir("data");
        
        println!("📁 Creating test data directory...");
        ensure_data_dir().unwrap();
        assert!(Path::new("data").exists());
        println!("   ✓ Directory created\n");
        
        // Vector to store all created states for verification
        let mut created_states: Vec<(String, NodeState)> = Vec::new();
        
        // ============================================================
        // STATE EVOLUTION: Version 1 - Genesis State
        // ============================================================
        println!("═══════════════════════════════════════════════════════════════════");
        println!("STATE v1: GENESIS (Empty initial state)");
        println!("═══════════════════════════════════════════════════════════════════\n");
        
        let state_v1 = NodeState {
            latest_gov_log_version: 0,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: None,
            chat_messages: Vec::new(),
            government_key_locked: false,
            has_synced_government: false,
        };
        
        println!("💾 Saving v1 snapshot:");
        println!("   - Gov log version: {}", state_v1.latest_gov_log_version);
        println!("   - Seen messages: {}", state_v1.seen_messages.len());
        println!("   - Government synced: {}", state_v1.has_synced_government);
        
        // Save to versioned file
        let v1_data = postcard::to_allocvec(&state_v1).unwrap();
        std::fs::write("data/snapshot_v1.bin", &v1_data).unwrap();
        let v1_size = v1_data.len();
        println!("   ✓ Saved to snapshot_v1.bin ({} bytes)\n", v1_size);
        
        created_states.push(("v1".to_string(), state_v1.clone()));
        
        // ============================================================
        // STATE EVOLUTION: Version 2 - First Government Log
        // ============================================================
        println!("═══════════════════════════════════════════════════════════════════");
        println!("STATE v2: FIRST GOVERNMENT LOG RECEIVED");
        println!("═══════════════════════════════════════════════════════════════════\n");
        
        let state_v2 = NodeState {
            latest_gov_log_version: 1,
            seen_messages: {
                let mut set = HashSet::new();
                set.insert(create_test_hash(1));
                set
            },
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: Some(create_test_government_log(1)),
            chat_messages: Vec::new(),
            government_key_locked: true,
            has_synced_government: true,
        };
        
        println!("💾 Saving v2 snapshot:");
        println!("   - Gov log version: {}", state_v2.latest_gov_log_version);
        println!("   - Seen messages: {}", state_v2.seen_messages.len());
        println!("   - Government synced: {}", state_v2.has_synced_government);
        println!("   - Government key locked: {}", state_v2.government_key_locked);
        
        let v2_data = postcard::to_allocvec(&state_v2).unwrap();
        std::fs::write("data/snapshot_v2.bin", &v2_data).unwrap();
        let v2_size = v2_data.len();
        println!("   ✓ Saved to snapshot_v2.bin ({} bytes)\n", v2_size);
        
        created_states.push(("v2".to_string(), state_v2.clone()));
        
        // ============================================================
        // STATE EVOLUTION: Version 3 - Tax Obligations Pending
        // ============================================================
        println!("═══════════════════════════════════════════════════════════════════");
        println!("STATE v3: TAX OBLIGATIONS SUBMITTED");
        println!("═══════════════════════════════════════════════════════════════════\n");
        
        let state_v3 = NodeState {
            latest_gov_log_version: 2,
            seen_messages: {
                let mut set = HashSet::new();
                set.insert(create_test_hash(1));
                set.insert(create_test_hash(2));
                set.insert(create_test_hash(10));
                set
            },
            peer_states: HashMap::new(),
            pending_tax_obligations: vec![
                TaxObligationEnvelope {
                    realm_id: create_test_realm_id(),
                    obligation: create_test_tax_obligation(),
                    timestamp: 1000,
                }
            ],
            latest_government_log: Some(create_test_government_log(2)),
            chat_messages: Vec::new(),
            government_key_locked: true,
            has_synced_government: true,
        };
        
        println!("💾 Saving v3 snapshot:");
        println!("   - Gov log version: {}", state_v3.latest_gov_log_version);
        println!("   - Seen messages: {}", state_v3.seen_messages.len());
        println!("   - Pending obligations: {}", state_v3.pending_tax_obligations.len());
        
        let v3_data = postcard::to_allocvec(&state_v3).unwrap();
        std::fs::write("data/snapshot_v3.bin", &v3_data).unwrap();
        let v3_size = v3_data.len();
        println!("   ✓ Saved to snapshot_v3.bin ({} bytes)\n", v3_size);
        
        created_states.push(("v3".to_string(), state_v3.clone()));
        
        // ============================================================
        // STATE EVOLUTION: Version 4 - Active Chat
        // ============================================================
        println!("═══════════════════════════════════════════════════════════════════");
        println!("STATE v4: CHAT MESSAGES EXCHANGED");
        println!("═══════════════════════════════════════════════════════════════════\n");
        
        let state_v4 = NodeState {
            latest_gov_log_version: 3,
            seen_messages: {
                let mut set = HashSet::new();
                for i in 1..=10 {
                    set.insert(create_test_hash(i));
                }
                set
            },
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(), // Cleared after processing
            latest_government_log: Some(create_test_government_log(3)),
            chat_messages: vec![
                create_test_chat_message(),
                ChatMessage {
                    message_id: "msg_456".to_string(),
                    sender: create_test_public_key(20),
                    receiver: Some(create_test_public_key(21)),
                    content: "Response message!".to_string(),
                    nonce: 2,
                    signature: create_test_signature(20),
                    message_hash: create_test_hash(31),
                },
            ],
            government_key_locked: true,
            has_synced_government: true,
        };
        
        println!("💾 Saving v4 snapshot:");
        println!("   - Gov log version: {}", state_v4.latest_gov_log_version);
        println!("   - Seen messages: {}", state_v4.seen_messages.len());
        println!("   - Chat messages: {}", state_v4.chat_messages.len());
        println!("   - Pending obligations: {}", state_v4.pending_tax_obligations.len());
        
        let v4_data = postcard::to_allocvec(&state_v4).unwrap();
        std::fs::write("data/snapshot_v4.bin", &v4_data).unwrap();
        let v4_size = v4_data.len();
        println!("   ✓ Saved to snapshot_v4.bin ({} bytes)\n", v4_size);
        
        created_states.push(("v4".to_string(), state_v4.clone()));
        
        // ============================================================
        // STATE EVOLUTION: Version 5 - High Activity
        // ============================================================
        println!("═══════════════════════════════════════════════════════════════════");
        println!("STATE v5: HIGH ACTIVITY (Many messages and obligations)");
        println!("═══════════════════════════════════════════════════════════════════\n");
        
        let state_v5 = NodeState {
            latest_gov_log_version: 10,
            seen_messages: {
                let mut set = HashSet::new();
                for i in 1..=50 {
                    set.insert(create_test_hash((i % 256) as u8));
                }
                set
            },
            peer_states: HashMap::new(),
            pending_tax_obligations: vec![
                TaxObligationEnvelope {
                    realm_id: create_test_realm_id(),
                    obligation: create_test_tax_obligation(),
                    timestamp: 5000,
                },
                TaxObligationEnvelope {
                    realm_id: create_test_realm_id(),
                    obligation: TaxObligation {
                        realm_id: create_test_realm_id(),
                        event_hash: create_test_hash(11),
                        user_id: create_test_public_key(6),
                        coin_amount: 200,
                        government_log: create_test_government_log(10),
                        sequence_number: 2,
                        previous_receipt_hash: None,
                        user_signature: create_test_signature(6),
                        obligation_hash: create_test_hash(12),
                    },
                    timestamp: 5100,
                },
            ],
            latest_government_log: Some(create_test_government_log(10)),
            chat_messages: vec![
                create_test_chat_message(),
                ChatMessage {
                    message_id: "msg_789".to_string(),
                    sender: create_test_public_key(25),
                    receiver: None, // Broadcast
                    content: "Broadcast to realm!".to_string(),
                    nonce: 3,
                    signature: create_test_signature(25),
                    message_hash: create_test_hash(35),
                },
                ChatMessage {
                    message_id: "msg_999".to_string(),
                    sender: create_test_public_key(30),
                    receiver: Some(create_test_public_key(31)),
                    content: "Private message".to_string(),
                    nonce: 4,
                    signature: create_test_signature(30),
                    message_hash: create_test_hash(40),
                },
            ],
            government_key_locked: true,
            has_synced_government: true,
        };
        
        println!("💾 Saving v5 snapshot:");
        println!("   - Gov log version: {}", state_v5.latest_gov_log_version);
        println!("   - Seen messages: {}", state_v5.seen_messages.len());
        println!("   - Chat messages: {}", state_v5.chat_messages.len());
        println!("   - Pending obligations: {}", state_v5.pending_tax_obligations.len());
        
        let v5_data = postcard::to_allocvec(&state_v5).unwrap();
        std::fs::write("data/snapshot_v5.bin", &v5_data).unwrap();
        let v5_size = v5_data.len();
        println!("   ✓ Saved to snapshot_v5.bin ({} bytes)\n", v5_size);
        
        created_states.push(("v5".to_string(), state_v5.clone()));
        
        // ============================================================
        // VERIFICATION: Load and Verify All States
        // ============================================================
        println!("═══════════════════════════════════════════════════════════════════");
        println!("VERIFICATION: LOADING AND COMPARING ALL STATES");
        println!("═══════════════════════════════════════════════════════════════════\n");
        
        for (version, original_state) in &created_states {
            println!("🔍 Verifying state {}...", version);
            
            let filename = format!("data/snapshot_{}.bin", version);
            let loaded_data = std::fs::read(&filename).unwrap();
            let loaded_state: NodeState = postcard::from_bytes(&loaded_data).unwrap();
            
            // Verify all fields
            assert_eq!(
                loaded_state.latest_gov_log_version,
                original_state.latest_gov_log_version,
                "{}: Gov log version mismatch", version
            );
            println!("   ✓ Gov log version: {}", loaded_state.latest_gov_log_version);
            
            assert_eq!(
                loaded_state.seen_messages.len(),
                original_state.seen_messages.len(),
                "{}: Seen messages count mismatch", version
            );
            println!("   ✓ Seen messages: {}", loaded_state.seen_messages.len());
            
            assert_eq!(
                loaded_state.pending_tax_obligations.len(),
                original_state.pending_tax_obligations.len(),
                "{}: Pending obligations count mismatch", version
            );
            println!("   ✓ Pending obligations: {}", loaded_state.pending_tax_obligations.len());
            
            assert_eq!(
                loaded_state.chat_messages.len(),
                original_state.chat_messages.len(),
                "{}: Chat messages count mismatch", version
            );
            println!("   ✓ Chat messages: {}", loaded_state.chat_messages.len());
            
            assert_eq!(
                loaded_state.government_key_locked,
                original_state.government_key_locked,
                "{}: Government key locked mismatch", version
            );
            println!("   ✓ Government key locked: {}", loaded_state.government_key_locked);
            
            assert_eq!(
                loaded_state.has_synced_government,
                original_state.has_synced_government,
                "{}: Has synced government mismatch", version
            );
            println!("   ✓ Has synced government: {}", loaded_state.has_synced_government);
            
            // Verify government log version if present
            match (&loaded_state.latest_government_log, &original_state.latest_government_log) {
                (Some(loaded_log), Some(orig_log)) => {
                    assert_eq!(
                        loaded_log.version_number,
                        orig_log.version_number,
                        "{}: Government log version number mismatch", version
                    );
                    println!("   ✓ Government log present: v{}", loaded_log.version_number);
                }
                (None, None) => {
                    println!("   ✓ No government log (as expected)");
                }
                _ => panic!("{}: Government log presence mismatch", version),
            }
            
            println!("   ✅ State {} verified completely!\n", version);
        }
        
        // ============================================================
        // BONUS: Test Event Log
        // ============================================================
        println!("═══════════════════════════════════════════════════════════════════");
        println!("BONUS: EVENT LOG PERSISTENCE");
        println!("═══════════════════════════════════════════════════════════════════\n");
        
        println!("📝 Appending 10 diverse events...");
        
        for i in 0..10 {
            let event = match i % 5 {
                0 => {
                    println!("   [{}] Government Log v{}", i, i);
                    PersistentRecord::GovernmentLog(create_test_government_log(i as u64))
                }
                1 => {
                    println!("   [{}] Tax Obligation", i);
                    PersistentRecord::TaxObligation(TaxObligationEnvelope {
                        realm_id: create_test_realm_id(),
                        obligation: create_test_tax_obligation(),
                        timestamp: 1000 + i as u64,
                    })
                }
                2 => {
                    println!("   [{}] Tax Receipt", i);
                    PersistentRecord::TaxReceipt(TaxReceiptEnvelope {
                        realm_id: create_test_realm_id(),
                        receipt: TaxReceipt {
                            obligation_hash: create_test_hash(10),
                            previous_receipt_hash: None,
                            payment_slot: 200 + i as u64,
                            coins_transferred: 100,
                            government_signature: create_test_signature(1),
                        },
                        timestamp: 2000 + i as u64,
                    })
                }
                3 => {
                    println!("   [{}] P2P Transfer", i);
                    PersistentRecord::P2PReceipt(ReceiptEnvelope {
                        realm_id: create_test_realm_id(),
                        receipt: P2PTransferReceipt {
                            realm_id: create_test_realm_id(),
                            tx_hash: create_test_hash(20 + i as u8),
                            sender: create_test_public_key(10),
                            receiver: create_test_public_key(11),
                            delta: create_test_resource_delta(0, 0, -50),
                            sender_prev_state_hash: create_test_hash(21),
                            receiver_prev_state_hash: create_test_hash(22),
                            sender_balance_before: create_test_resources(100, 100, 100),
                            sender_balance_after: create_test_resources(100, 100, 50),
                            receiver_balance_before: create_test_resources(50, 50, 50),
                            receiver_balance_after: create_test_resources(50, 50, 100),
                            nonce: i as u64,
                            sender_signature: create_test_signature(10),
                            receiver_signature: create_test_signature(11),
                        },
                        timestamp: 3000 + i as u64,
                    })
                }
                _ => {
                    println!("   [{}] Chat Message", i);
                    PersistentRecord::ChatMessage(ChatMessageEnvelope {
                        realm_id: create_test_realm_id(),
                        message: ChatMessage {
                            message_id: format!("msg_{}", i),
                            sender: create_test_public_key(15),
                            receiver: Some(create_test_public_key(16)),
                            content: format!("Message #{}", i),
                            nonce: i as u64,
                            signature: create_test_signature(15),
                            message_hash: create_test_hash(30 + i as u8),
                        },
                        timestamp: 4000 + i as u64,
                    })
                }
            };
            append_event(&event).unwrap();
        }
        
        let events_size = std::fs::metadata("data/events.bin").unwrap().len();
        println!("\n   ✓ Events appended ({} bytes)\n", events_size);
        
        println!("📖 Loading and verifying events...");
        let loaded_events = load_events().unwrap();
        assert_eq!(loaded_events.len(), 10, "Event count mismatch!");
        println!("   ✓ Loaded {} events", loaded_events.len());
        
        // Count event types
        let mut type_counts = HashMap::new();
        for event in &loaded_events {
            let type_name = match event {
                PersistentRecord::GovernmentLog(_) => "GovernmentLog",
                PersistentRecord::TaxObligation(_) => "TaxObligation",
                PersistentRecord::TaxReceipt(_) => "TaxReceipt",
                PersistentRecord::P2PReceipt(_) => "P2PReceipt",
                PersistentRecord::ChatMessage(_) => "ChatMessage",
            };
            *type_counts.entry(type_name).or_insert(0) += 1;
        }
        
        println!("\n   Event type distribution:");
        for (event_type, count) in type_counts {
            println!("      {} events: {}", count, event_type);
        }
        
        // ============================================================
        // FINAL SUMMARY
        // ============================================================
        println!("\n═══════════════════════════════════════════════════════════════════");
        println!("✅ ALL VERIFICATION TESTS PASSED!");
        println!("═══════════════════════════════════════════════════════════════════\n");
        
        println!("📂 Files created (NOT cleaned up for inspection):\n");
        
        let files = vec![
            ("snapshot_v1.bin", v1_size),
            ("snapshot_v2.bin", v2_size),
            ("snapshot_v3.bin", v3_size),
            ("snapshot_v4.bin", v4_size),
            ("snapshot_v5.bin", v5_size),
            ("events.bin", events_size as usize),
        ];
        
        for (filename, size) in files {
            println!("   📄 data/{} ({} bytes)", filename, size);
        }
        
        println!("\n🔍 To inspect files:");
        println!("   ls -lh data/");
        println!("   hexdump -C data/snapshot_v1.bin | head -20");
        println!("   hexdump -C data/snapshot_v5.bin | head -20");
        println!("   hexdump -C data/events.bin | head -20\n");
        
        println!("🧹 To clean up:");
        println!("   rm -rf data/\n");
        
        println!("═══════════════════════════════════════════════════════════════════\n");
        
        // DO NOT CLEANUP - Files left for inspection
    }
    
    /// Quick test that DOES clean up after itself
    /// Run this to verify basic functionality without leaving files
    #[test]
    fn test_quick_persistence_check() {
        // Clean up first
        let _ = std::fs::remove_file("data/snapshot.bin");
        let _ = std::fs::remove_file("data/events.bin");
        let _ = std::fs::remove_dir("data");
        
        // Test snapshot
        let state = NodeState {
            latest_gov_log_version: 99,
            seen_messages: HashSet::new(),
            peer_states: HashMap::new(),
            pending_tax_obligations: Vec::new(),
            latest_government_log: None,
            chat_messages: Vec::new(),
            government_key_locked: false,
            has_synced_government: false,
        };
        
        save_snapshot(&state).unwrap();
        assert!(Path::new("data/snapshot.bin").exists());
        
        let loaded = load_snapshot().unwrap().unwrap();
        assert_eq!(loaded.latest_gov_log_version, 99);
        
        // Test events
        let event = PersistentRecord::GovernmentLog(create_test_government_log(1));
        append_event(&event).unwrap();
        assert!(Path::new("data/events.bin").exists());
        
        let events = load_events().unwrap();
        assert_eq!(events.len(), 1);
        
        // Clean up
        let _ = std::fs::remove_file("data/snapshot.bin");
        let _ = std::fs::remove_file("data/events.bin");
        let _ = std::fs::remove_dir("data");
    }
}