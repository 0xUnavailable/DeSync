pub mod libp2p;
pub mod protocol;
pub mod sync;
pub mod state_apply;

pub use libp2p::*;
pub use protocol::*;

use crate::types::NodeRole;

/// Information tracked about each connected peer
#[derive(Clone, Debug)]
pub struct PeerInfo {
    /// Role of this peer in the network
    pub role: NodeRole,
    /// Last known head version
    pub head_version: u64,
    /// Whether handshake is complete
    pub handshake_complete: bool,
}

impl PeerInfo {
    pub fn new(role: NodeRole) -> Self {
        Self {
            role,
            head_version: 0,
            handshake_complete: false,
        }
    }
    
    pub fn with_handshake_complete(mut self) -> Self {
        self.handshake_complete = true;
        self
    }
    
    pub fn update_version(&mut self, version: u64) {
        self.head_version = version;
    }
}