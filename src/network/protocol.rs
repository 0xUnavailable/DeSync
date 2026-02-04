use serde::{Serialize, Deserialize};
use crate::types::{HASH, HelloMessage, NodeRole, RealmObject};

/// Request messages for the Realm sync protocol
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RealmRequest {
    /// Initial handshake - sent immediately after connection
    Hello(HelloMessage),
    /// Request the current head state from a peer
    GetHead,
    /// Request a specific object by its hash
    GetObject(HASH),
    /// Request log entries starting from a specific version
    LogRequest { start_version: u64 },
    /// Push an object to a peer (for active propagation of user-generated objects)
    PushObject(Vec<u8>),
}

/// Response messages for the Realm sync protocol
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RealmResponse {
    /// Acknowledgment of Hello message with peer info
    HelloAck { role: NodeRole, realm_id: HASH },
    /// Response containing the node's current head state
    Head(NodeHead),
    /// Response containing a requested object
    Object(Vec<u8>),
    /// Response containing a chunk of log entries
    LogsChunk(Vec<RealmObject>),
    /// Acknowledgment that a pushed object was received
    PushAck,
    /// Error response
    Error(String),
}
/// Represents the current head state of a node
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeHead {
    /// Version number of the government log
    pub gov_log_version: u64,
    /// Hash of the current government log
    pub gov_log_hash: HASH,
    /// Optional hash of the receipts head
    pub receipts_head: Option<HASH>,
}

impl NodeHead {
    pub fn new(gov_log_version: u64, gov_log_hash: HASH, receipts_head: Option<HASH>) -> Self {
        Self {
            gov_log_version,
            gov_log_hash,
            receipts_head,
        }
    }
    
    /// Check if this node is ahead of another node
    pub fn is_ahead_of(&self, other: &NodeHead) -> bool {
        self.gov_log_version > other.gov_log_version
    }
}
pub fn validate_message(
    sender_role: NodeRole,
    receiver_role: NodeRole,
    request: &RealmRequest,
) -> bool {
    use NodeRole::*;
    use RealmRequest::*;
    
    match (sender_role, receiver_role, request) {
        // Hello is always allowed as first message
        (_, _, Hello(_)) => true,
        
        // Government → User: GetHead, GetObject for verification, PushObject for receipts
        (Government, User, GetHead | GetObject(_) | PushObject(_)) => true,
        
        // User → Government: GetHead, GetObject, LogRequest, PushObject allowed
        (User, Government, GetHead | GetObject(_) | LogRequest { .. } | PushObject(_)) => true,
        
        // User → User: All sync messages allowed including PushObject
        (User, User, GetHead | GetObject(_) | LogRequest { .. } | PushObject(_)) => true,
        
        // Government → Government: Should never happen (connections rejected)
        (Government, Government, _) => false,
        
        // Anything else not explicitly allowed
        _ => false,
    }
}

/// Validate response based on sender and receiver roles

pub fn validate_response(
    sender_role: NodeRole,
    receiver_role: NodeRole,
    response: &RealmResponse,
) -> bool {
    use NodeRole::*;
    use RealmResponse::*;
    
    match (sender_role, receiver_role, response) {
        // HelloAck is always allowed in response to Hello
        (_, _, HelloAck { .. }) => true,
        
        // Government → User: Can send Head, Object, LogsChunk, PushAck, Error
        (Government, User, Head(_) | Object(_) | LogsChunk(_) | PushAck | Error(_)) => true,
        
        // User → Government: Can send Head, Object, PushAck, Error (government doesn't request chunks)
        (User, Government, Head(_) | Object(_) | PushAck | Error(_)) => true,
        
        // User → User: Can send any response
        (User, User, Head(_) | Object(_) | LogsChunk(_) | PushAck | Error(_)) => true,
        
        // Government → Government: Should never happen
        (Government, Government, _) => false,
        
        // Anything else not explicitly allowed
        _ => false,
    }
}