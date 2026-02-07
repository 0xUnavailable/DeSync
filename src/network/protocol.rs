use crate::types::{HelloMessage, NodeRole, RealmObject, HASH};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RealmRequest {
    Hello(HelloMessage),
    GetHead,
    GetObject(HASH),
    LogRequest { start_version: u64 },
    PushObject(Vec<u8>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RealmResponse {
    HelloAck { role: NodeRole, realm_id: HASH },
    Head(NodeHead),
    Object(Vec<u8>),
    LogsChunk(Vec<RealmObject>),
    PushAck,
    Error(String),
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeHead {
    pub gov_log_version: u64,
    pub gov_log_hash: HASH,
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
        (_, _, Hello(_)) => true,

        (Government, User, GetHead | GetObject(_) | PushObject(_)) => true,

        (User, Government, GetHead | GetObject(_) | LogRequest { .. } | PushObject(_)) => true,

        (User, User, GetHead | GetObject(_) | LogRequest { .. } | PushObject(_)) => true,

        (Government, Government, _) => false,

        _ => false,
    }
}

pub fn validate_response(
    sender_role: NodeRole,
    receiver_role: NodeRole,
    response: &RealmResponse,
) -> bool {
    use NodeRole::*;
    use RealmResponse::*;

    match (sender_role, receiver_role, response) {
        (_, _, HelloAck { .. }) => true,

        (Government, User, Head(_) | Object(_) | LogsChunk(_) | PushAck | Error(_)) => true,

        (User, Government, Head(_) | Object(_) | PushAck | Error(_)) => true,

        (User, User, Head(_) | Object(_) | LogsChunk(_) | PushAck | Error(_)) => true,

        (Government, Government, _) => false,

        _ => false,
    }
}
