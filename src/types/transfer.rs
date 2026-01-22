use serde::{Serialize, Deserialize};
use crate::crypto::ed25519_utils::Signature;
use ed25519_dalek::PublicKey;
use crate::types::user_state::ResourceDelta;

#[derive(Serialize, Deserialize, Clone)]
pub struct P2PTransferReceipt {
    pub sender: PublicKey,
    pub receiver: PublicKey,
    pub delta: ResourceDelta,
    pub sender_prev_hash: [u8;32],
    pub receiver_prev_hash: [u8;32],
    pub nonce: u64,
    pub signatures: Vec<Signature>, // [sender_sig, receiver_sig]
}
