
use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

use crate::types::{
    GovernmentRuleLog, TaxObligationEnvelope, TaxReceiptEnvelope,
    ReceiptEnvelope, ChatMessageEnvelope,
};
use crate::network::libp2p::NodeState;

const SNAPSHOT_PATH: &str = "node-snapshot.json";
const EVENT_LOG_PATH: &str = "node-events.jsonl";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PersistentRecord {
    GovernmentLog(GovernmentRuleLog),
    TaxObligation(TaxObligationEnvelope),
    TaxReceipt(TaxReceiptEnvelope),
    P2PReceipt(ReceiptEnvelope),
    ChatMessage(ChatMessageEnvelope),
}

pub fn save_snapshot(state: &NodeState) -> Result<()> {
    let file = File::create(SNAPSHOT_PATH)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, state)?;
    Ok(())
}

pub fn load_snapshot() -> Result<Option<NodeState>> {
    if !Path::new(SNAPSHOT_PATH).exists() {
        return Ok(None);
    }
    
    let file = File::open(SNAPSHOT_PATH)?;
    let reader = BufReader::new(file);
    let state = serde_json::from_reader(reader)?;
    Ok(Some(state))
}

pub fn append_event(record: &PersistentRecord) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(EVENT_LOG_PATH)?;
    
    let json = serde_json::to_string(record)?;
    writeln!(file, "{}", json)?;
    
    Ok(())
}

pub fn load_events() -> Result<Vec<PersistentRecord>> {
    if !Path::new(EVENT_LOG_PATH).exists() {
        return Ok(Vec::new());
    }
    
    let file = File::open(EVENT_LOG_PATH)?;
    let reader = BufReader::new(file);
    
    use std::io::BufRead;
    let mut records = Vec::new();
    
    for line in reader.lines() {
        let line = line?;
        if !line.is_empty() {
            let record: PersistentRecord = serde_json::from_str(&line)?;
            records.push(record);
        }
    }
    
    Ok(records)
}
