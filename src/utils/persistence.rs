use std::fs::File;
use anyhow::Result;
use std::path::Path;
use std::io::{BufReader, BufWriter};
use serde::{Deserialize, Serialize};
use crate::network::libp2p::NodeState;
use crate::utils::file_protection::ProtectedFile;
use crate::types::{
    ChatMessageEnvelope, GovernmentRuleLog, ReceiptEnvelope, TaxObligationEnvelope,
    TaxReceiptEnvelope,
};

const SNAPSHOT_PATH: &str = "node-snapshot.json";
const EVENT_LOG_PATH: &str = "node-events.jsonl";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PersistentRecord {
    GovernmentLog(GovernmentRuleLog),
    TaxObligation(TaxObligationEnvelope),
    TaxReceipt(TaxReceiptEnvelope),
    P2PReceipt(ReceiptEnvelope),
    ChatMessage(ChatMessageEnvelope),
    TaskStarted(TaskStartedEvent),
    TaskHarvested(TaskHarvestedEvent),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaskStartedEvent {
    pub task_name: String,
    pub start_time: u64,
    pub duration: u64,
    pub resources_locked: crate::types::Resources,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaskHarvestedEvent {
    pub task_name: String,
    pub completed_at: u64,
    pub coins_earned: u64,
    pub vdf_iterations: u64,
    pub vdf_compute_time_ms: u64,
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
    let protected = ProtectedFile::new(EVENT_LOG_PATH);
    let json = serde_json::to_string(record)?;
    protected.append_line(&json)?;
    Ok(())
}

pub fn load_events() -> Result<Vec<PersistentRecord>> {
    let protected = ProtectedFile::new(EVENT_LOG_PATH);

    if !protected.exists() {
        return Ok(Vec::new());
    }

    let lines = protected.read_lines()?;
    let mut records = Vec::new();

    for line in lines {
        if !line.is_empty() {
            let record: PersistentRecord = serde_json::from_str(&line)?;
            records.push(record);
        }
    }

    Ok(records)
}
