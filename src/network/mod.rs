pub mod libp2p;
pub mod protocol;
pub mod sync;
pub mod state_apply;
pub use libp2p::*;
pub use protocol::*;
use crate::types::NodeRole;

#[derive(Clone, Debug)]
pub struct PeerInfo {
    pub role: NodeRole,
    pub head_version: u64,
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