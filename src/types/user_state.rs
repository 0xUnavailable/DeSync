use serde::{Serialize, Deserialize};
use crate::crypto::ed25519_utils::Signature;

#[derive(Serialize, Deserialize, Clone)]
pub struct ResourceDelta {
    pub gold: i64,
    pub wheat: i64,
    pub wood: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Snapshot {
    pub resources: ResourceDelta,
    pub energy: u64,
    pub land: Vec<Plot>,
    pub tasks: Vec<Task>,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Plot {
    pub plot_id: u64,
    pub plot_type: String,
    pub crop: Option<String>,
    pub planted_at: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Task {
    pub task_id: String,
    pub action: String,
    pub started: u64,
    pub duration: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Event {
    pub action: String,
    pub gov_log_version: u64,
    pub plot_id: u64,
    pub task_id: String,
    pub deltas: ResourceDelta,
    pub timestamp: u64,
    pub user_signature: Signature,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UserState {
    pub snapshot: Snapshot,
    pub events: Vec<Event>,
    pub previous_hash: [u8;32],
    pub sequence_number: u64,
    pub signature: Signature,
}
