use anyhow::{Result, bail};
use libp2p::{
    identity,
    request_response,
    swarm::NetworkBehaviour,
    SwarmBuilder,
    PeerId, Multiaddr, Swarm,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io;
use ed25519_dalek::VerifyingKey;
use futures::prelude::*;

use crate::types::{
    HASH, GovernmentRuleLog, TaxObligationEnvelope,
    TaxReceiptEnvelope, ReceiptEnvelope, NodeRole, HelloMessage,
};
use crate::network::protocol::{RealmRequest, RealmResponse, NodeHead, validate_message};
use crate::network::PeerInfo;
use crate::storage::ObjectStore;

//
// - - -- - -- - -- - -- - -- Codec - - -- - -- - -- - -- - --
//

#[derive(Debug, Clone, Default)]
pub struct RealmCodec;

impl RealmCodec {
    async fn read_length_prefixed<T: AsyncRead + Unpin>(io: &mut T) -> io::Result<Vec<u8>> {
        let mut len_bytes = [0u8; 4];
        io.read_exact(&mut len_bytes).await?;
        let len = u32::from_be_bytes(len_bytes) as usize;

        if len > 10 * 1024 * 1024 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "message too large"));
        }

        let mut buf = vec![0u8; len];
        io.read_exact(&mut buf).await?;
        Ok(buf)
    }

    async fn write_length_prefixed<T: AsyncWrite + Unpin>(
        io: &mut T,
        data: &[u8],
    ) -> io::Result<()> {
        io.write_all(&(data.len() as u32).to_be_bytes()).await?;
        io.write_all(data).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl request_response::Codec for RealmCodec {
    type Protocol = RealmProtocol;
    type Request = RealmRequest;
    type Response = RealmResponse;

    async fn read_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let buf = Self::read_length_prefixed(io).await?;
        serde_json::from_slice(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let buf = Self::read_length_prefixed(io).await?;
        serde_json::from_slice(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let bytes = serde_json::to_vec(&req)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Self::write_length_prefixed(io, &bytes).await?;
        io.close().await
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let bytes = serde_json::to_vec(&res)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Self::write_length_prefixed(io, &bytes).await?;
        io.close().await
    }
}

//
// - - -- - -- - -- - -- - -- Protocol - - -- - -- - -- - -- - --
//

#[derive(Debug, Clone)]
pub struct RealmProtocol;

impl AsRef<str> for RealmProtocol {
    fn as_ref(&self) -> &str {
        "/realm/sync/1.0.0"
    }
}

//
// - - -- - -- - -- - -- - -- Behaviour - - -- - -- - -- - -- - --
//

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "RealmBehaviourEvent")]  
pub struct RealmBehaviour {
    pub request_response: request_response::Behaviour<RealmCodec>,
}

/// This is the enum that was missing. 
/// It wraps the events produced by the individual fields in your Behaviour.
#[derive(Debug)]
pub enum RealmBehaviourEvent {
    RequestResponse(request_response::Event<RealmRequest, RealmResponse>),
}

/// The NetworkBehaviour derive macro requires From implementations 
/// to route inner events into your custom event enum.
impl From<request_response::Event<RealmRequest, RealmResponse>> for RealmBehaviourEvent {
    fn from(event: request_response::Event<RealmRequest, RealmResponse>) -> Self {
        RealmBehaviourEvent::RequestResponse(event)
    }
}

//
// - - -- - -- - -- - -- - -- State - - -- - -- - -- - -- - --
//

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NodeState {
    pub realm_id: HASH,
    pub latest_government_log: Option<GovernmentRuleLog>,
    pub latest_gov_log_version: u64,
    pub previous_government_log: Option<GovernmentRuleLog>,
    pub tax_obligations: Vec<TaxObligationEnvelope>,
    pub tax_receipts: Vec<TaxReceiptEnvelope>,
    pub p2p_receipts: Vec<ReceiptEnvelope>,
    pub pending_obligations: HashMap<HASH, TaxObligationEnvelope>,
    pub seen_receipts: HashSet<HASH>,
    pub coins: i64,
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            realm_id: [0u8; 32],
            latest_government_log: None,
            latest_gov_log_version: 0,
            previous_government_log: None,
            tax_obligations: Vec::new(),
            tax_receipts: Vec::new(),
            p2p_receipts: Vec::new(),
            pending_obligations: HashMap::new(),
            seen_receipts: HashSet::new(),
            coins: 0,
        }
    }
}

//
// - - -- - -- - -- - -- - -- Node - - -- - -- - -- - -- - --
//

pub struct NetworkConfig {
    pub realm_id: HASH,
    pub keypair: identity::Keypair,
    pub government_public_key: Option<VerifyingKey>,
    pub is_government: bool,
    pub node_role: NodeRole,
}

pub struct P2PNode {
    pub swarm: Swarm<RealmBehaviour>,
    pub state: NodeState,
    pub object_store: ObjectStore,
    pub config: NetworkConfig,
    pub peer_heads: HashMap<PeerId, NodeHead>,
    pub peer_info: HashMap<PeerId, PeerInfo>,
    pub pending_hellos: HashSet<PeerId>,
    pub log_index: crate::storage::LogVersionIndex,
}

impl P2PNode {
    pub fn new(config: NetworkConfig) -> Result<Self> {
        let behaviour = RealmBehaviour {
            request_response: request_response::Behaviour::new(
                [(RealmProtocol, request_response::ProtocolSupport::Full)],
                request_response::Config::default(),
            ),
        };

        // Note: .with_async_std() requires the 'async-std' feature in your Cargo.toml
        let swarm = SwarmBuilder::with_existing_identity(config.keypair.clone())
            .with_async_std()
            .with_tcp(
                libp2p::tcp::Config::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )?
            .with_behaviour(|_key| Ok(behaviour))?
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(std::time::Duration::from_secs(u64::MAX))
            })
            .build();

        let mut state = NodeState::default();
        state.realm_id = config.realm_id;

        let object_store = ObjectStore::new("./objects")?;
        
        // Build log index from existing objects
        let log_index = match crate::storage::build_index_from_store(&object_store) {
            Ok(idx) => {
                log::info!("[INIT] Built log index with {} entries, max version: {}", 
                    idx.len(), idx.max_version());
                idx
            }
            Err(e) => {
                log::warn!("[INIT] Failed to build log index: {}, starting empty", e);
                crate::storage::LogVersionIndex::new()
            }
        };

        Ok(Self {
            swarm,
            state,
            object_store,
            config,
            peer_heads: HashMap::new(),
            peer_info: HashMap::new(),
            pending_hellos: HashSet::new(),
            log_index,
        })
    }


    /// Send Hello message to a newly connected peer
    pub fn send_hello(&mut self, peer: PeerId) {
        log::info!("[HANDSHAKE] Sending Hello to peer {} as {}", peer, self.config.node_role.as_str());
        
        let hello = HelloMessage {
            role: self.config.node_role,
            realm_id: self.config.realm_id,
        };
        
        let _ = self.swarm.behaviour_mut()
            .request_response
            .send_request(&peer, RealmRequest::Hello(hello));
        
        self.pending_hellos.insert(peer);
    }

    /// Handle incoming Hello message
    pub fn handle_hello(&mut self, peer: PeerId, hello: HelloMessage) -> Result<()> {
        log::info!(
            "[HANDSHAKE] Received Hello from peer {} as {}, realm {:?}",
            peer,
            hello.role.as_str(),
            hex::encode(&hello.realm_id[..8])
        );

        // Validate realm ID
        if hello.realm_id != self.config.realm_id {
            log::warn!(
                "[SECURITY] Peer {} has different realm_id, disconnecting",
                peer
            );
            let _ = self.swarm.disconnect_peer_id(peer);
            return Ok(());
        }

        // Enforce: Government ↔ Government connections are forbidden
        if self.config.node_role == NodeRole::Government && hello.role == NodeRole::Government {
            log::warn!(
                "[SECURITY] Government-to-Government connection rejected from peer {}",
                peer
            );
            let _ = self.swarm.disconnect_peer_id(peer);
            return Ok(());
        }

        // Store peer info
        let mut peer_info = PeerInfo::new(hello.role);
        peer_info.handshake_complete = true;
        self.peer_info.insert(peer, peer_info);
        
        self.pending_hellos.remove(&peer);

        log::info!(
            "[HANDSHAKE] Handshake complete with peer {} ({})",
            peer,
            hello.role.as_str()
        );

        Ok(())
    }

    /// Validate outbound message before sending
    fn validate_outbound_message(&self, peer: PeerId, request: &RealmRequest) -> bool {
        let peer_info = match self.peer_info.get(&peer) {
            Some(info) => info,
            None => {
                log::warn!("[SECURITY] Attempted to send to peer {} before handshake complete", peer);
                return false;
            }
        };

        let valid = validate_message(self.config.node_role, peer_info.role, request);
        
        if !valid {
            log::warn!(
                "[SECURITY] Blocked outbound message: {} → {} attempting to send {:?}",
                self.config.node_role.as_str(),
                peer_info.role.as_str(),
                request
            );
        }
        
        valid
    }

    /// Validate inbound message
    fn validate_inbound_message(&self, peer: PeerId, request: &RealmRequest) -> bool {
        // Hello messages don't need prior handshake
        if matches!(request, RealmRequest::Hello(_)) {
            return true;
        }

        let peer_info = match self.peer_info.get(&peer) {
            Some(info) => info,
            None => {
                log::warn!("[SECURITY] Received message from peer {} before handshake complete", peer);
                return false;
            }
        };

        let valid = validate_message(peer_info.role, self.config.node_role, request);
        
        if !valid {
            log::warn!(
                "[SECURITY] Blocked inbound message: {} → {} sent {:?}",
                peer_info.role.as_str(),
                self.config.node_role.as_str(),
                request
            );
        }
        
        valid
    }

    pub fn should_request_from_peer(&self, _peer: PeerId) -> bool {
        // If we are government, we should never request objects from users
        if self.config.is_government {
            return false;
        }
        // Users can request from anyone
        true
    }

    pub fn listen(&mut self, addr: Multiaddr) -> Result<()> {
        self.swarm.listen_on(addr)?;
        Ok(())
    }

    pub fn dial(&mut self, addr: Multiaddr) -> Result<()> {
        self.swarm.dial(addr)?;
        Ok(())
    }

    pub fn get_head(&self) -> NodeHead {
        let (version, hash) = self.state
            .latest_government_log
            .as_ref()
            .map(|l| (l.version_number, l.log_hash))
            .unwrap_or((0, [0u8; 32]));

        let receipts_head = self.state.tax_receipts.last()
            .map(|r| *blake3::hash(&serde_json::to_vec(&r.receipt).unwrap()).as_bytes());

        NodeHead::new(version, hash, receipts_head)
    }

    pub fn request_head(&mut self, peer: PeerId) {
        if !self.validate_outbound_message(peer, &RealmRequest::GetHead) {
            return;
        }
        
        let _ = self.swarm.behaviour_mut()
            .request_response
            .send_request(&peer, RealmRequest::GetHead);
    }

    pub fn request_object(&mut self, peer: PeerId, hash: HASH) {
        let request = RealmRequest::GetObject(hash);
        if !self.validate_outbound_message(peer, &request) {
            return;
        }
        
        let _ = self.swarm.behaviour_mut()
            .request_response
            .send_request(&peer, request);
    }

    pub fn request_logs(&mut self, peer: PeerId, start_version: u64) {
        let request = RealmRequest::LogRequest { start_version };
        if !self.validate_outbound_message(peer, &request) {
            return;
        }
        
        log::info!("[SYNC] Requesting logs from peer {} starting at version {}", peer, start_version);
        
        let _ = self.swarm.behaviour_mut()
            .request_response
            .send_request(&peer, request);
    }

     pub fn push_object(&mut self, peer: PeerId, object_bytes: Vec<u8>) {
        let request = RealmRequest::PushObject(object_bytes);
        if !self.validate_outbound_message(peer, &request) {
            return;
        }
        
        log::info!("[PUSH] Sending object to peer {}", peer);
        
        let _ = self.swarm.behaviour_mut()
            .request_response
            .send_request(&peer, request);
    }

    pub fn broadcast_object(&mut self, object_bytes: Vec<u8>) {
        let peers: Vec<PeerId> = self.connected_peers();
        log::info!("[PUSH] Broadcasting object to {} peers", peers.len());
        
        for peer in peers {
            if let Some(peer_info) = self.peer_info.get(&peer) {
                if peer_info.handshake_complete {
                    self.push_object(peer, object_bytes.clone());
                }
            }
        }
    }

    pub fn store_object(&mut self, hash: HASH, bytes: Vec<u8>) -> Result<()> {
        if blake3::hash(&bytes).as_bytes() != &hash {
            bail!("hash mismatch");
        }
        self.object_store.put(hash, bytes)?;
        Ok(())
    }

    pub fn local_addresses(&self) -> Vec<Multiaddr> {
        self.swarm.listeners().cloned().collect()
    }

    pub fn connected_peers(&self) -> Vec<PeerId> {
        self.swarm.connected_peers().cloned().collect()
    }
    
    pub fn sync_with_peer(&mut self, peer: PeerId) -> Result<()> {
        // Ensure handshake is complete before syncing
        if !self.peer_info.get(&peer).map(|p| p.handshake_complete).unwrap_or(false) {
            log::warn!("[SYNC] Skipping sync with peer {} - handshake not complete", peer);
            return Ok(());
        }
        
        self.request_head(peer);
        Ok(())
    }

   pub fn handle_peer_head(&mut self, peer: PeerId, peer_head: NodeHead) -> Result<()> {
        use crate::network::sync::reconcile_heads;
        
        log::info!(
            "[SYNC] Received head from peer {}: version={}, hash={}",
            peer,
            peer_head.gov_log_version,
            hex::encode(&peer_head.gov_log_hash[..8])
        );
        
        // Update peer info with latest version
        if let Some(peer_info) = self.peer_info.get_mut(&peer) {
            peer_info.update_version(peer_head.gov_log_version);
        }
        
        self.peer_heads.insert(peer, peer_head.clone());
        
        // Government nodes should never request objects from peers
        if !self.should_request_from_peer(peer) {
            log::debug!("[SYNC] Government node skipping sync requests");
            return Ok(());
        }
        
        let local_head = self.get_head();
        
        // CRITICAL FIX: Special case for first-time genesis sync
        // If we have no state (version 0, zero hash) and peer has anything, request from version 0
        if local_head.gov_log_version == 0 
            && local_head.gov_log_hash == [0u8; 32]
            && peer_head.gov_log_version >= 0 {
            log::info!(
                "[SYNC] First-time sync detected (no local state), requesting logs from version 0"
            );
            self.request_logs(peer, 0);
            return Ok(());
        }
        
        // If peer is ahead, request delta logs
        if peer_head.gov_log_version > local_head.gov_log_version {
            log::info!(
                "[SYNC] Peer {} is ahead (local: {}, peer: {}), requesting logs",
                peer,
                local_head.gov_log_version,
                peer_head.gov_log_version
            );
            self.request_logs(peer, local_head.gov_log_version + 1);
        } else {
            // Fallback to object-based sync for backwards compatibility
            let plan = reconcile_heads(&local_head, &peer_head, &self.object_store);
            
            if !plan.is_empty() {
                log::info!("[SYNC] Object-based sync: requesting {} objects", plan.all_hashes().len());
            }
            
            for hash in plan.all_hashes() {
                self.request_object(peer, hash);
            }
        }
        
        Ok(())
    }
    
    pub fn handle_received_object(&mut self, object_bytes: Vec<u8>) -> Result<()> {
        use crate::network::sync::ingest_object;
        use crate::network::state_apply::apply_object_to_state;
        
        let gov_key = self.config.government_public_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No government key configured"))?;
        
        let hash = match ingest_object(&object_bytes, gov_key, &self.object_store) {
            Ok(h) => h,
            Err(e) => {
                log::warn!("[SYNC] Failed to ingest object: {}", e);
                return Ok(());
            }
        };
        
        let object: crate::types::RealmObject = match serde_json::from_slice(&object_bytes) {
            Ok(obj) => obj,
            Err(e) => {
                log::warn!("[SYNC] Failed to deserialize object: {}", e);
                return Ok(());
            }
        };
        
        // Index government logs
        if let crate::types::RealmObject::GovernmentRuleLog(ref log) = object {
            self.log_index.index_log(log.version_number, hash);
            log::debug!("[SYNC] Indexed log version {} with hash {}", 
                log.version_number, hex::encode(&hash[..8]));
        }
        
        apply_object_to_state(object, &mut self.state)?;
        
        Ok(())
    }

    /// Handle received logs chunk from delta sync
    pub fn handle_logs_chunk(&mut self, logs: Vec<crate::types::RealmObject>) -> Result<()> {
        use crate::network::state_apply::apply_object_to_state;
        use crate::network::sync::validate_log_chain;
        
        log::info!("[SYNC] Received logs chunk with {} entries", logs.len());
        
        let gov_key = self.config.government_public_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No government key configured"))?;
        
        // FIX #2: Track expected version for sequential validation
        // If we have no state (latest_government_log is None), expect version 0 (genesis)
        // Otherwise, expect the next version after our current state
        let mut expected_version = if self.state.latest_government_log.is_none() {
            0  // Start from genesis
        } else {
            self.state.latest_gov_log_version + 1
        };
        
        let mut previous_log: Option<crate::types::GovernmentRuleLog> = 
            self.state.latest_government_log.clone();
        
        for object in logs {
            // Verify signature
            if !object.verify_signature(gov_key) {
                log::warn!("[SYNC] Signature verification failed for object, skipping");
                continue;
            }
            
            // Additional validation for government logs
            if let crate::types::RealmObject::GovernmentRuleLog(ref log) = object {
                // Validate sequential version
                if log.version_number != expected_version {
                    anyhow::bail!(
                        "Version gap detected: expected {}, got {}",
                        expected_version,
                        log.version_number
                    );
                }
                
                // Validate chain integrity (if we have a previous log)
                if let Some(ref prev_log) = previous_log {
                    if let Err(e) = validate_log_chain(log, prev_log) {
                        anyhow::bail!("Log chain validation failed: {}", e);
                    }
                }
                
                // Validate realm ID
                if log.realm_id != self.config.realm_id {
                    anyhow::bail!(
                        "Realm ID mismatch: expected {}, got {}",
                        hex::encode(self.config.realm_id),
                        hex::encode(log.realm_id)
                    );
                }
                
                // Update tracking for next iteration
                expected_version += 1;
                previous_log = Some(log.clone());
            }
            
            // Store object
            let object_bytes = serde_json::to_vec(&object)?;
            let hash = *blake3::hash(&object_bytes).as_bytes();
            
            if !self.object_store.has_object(&hash) {
                self.object_store.store_object(&hash, &object_bytes)?;
                
                // Index government logs
                if let crate::types::RealmObject::GovernmentRuleLog(ref log) = object {
                    self.log_index.index_log(log.version_number, hash);
                    log::debug!("[SYNC] Indexed log version {} with hash {}", 
                        log.version_number, hex::encode(&hash[..8]));
                }
            }
            
            // Apply to state
            apply_object_to_state(object, &mut self.state)?;
        }
        
        log::info!(
            "[SYNC] Successfully applied logs chunk, current version: {}",
            self.state.latest_gov_log_version
        );
        
        Ok(())
    }

    /// Check if we can sync with a peer (based on role)
    pub fn can_sync_with_peer(&self, peer: PeerId) -> bool {
        match self.peer_info.get(&peer) {
            Some(info) if info.handshake_complete => {
                // Government never initiates sync
                if self.config.node_role == NodeRole::Government {
                    return false;
                }
                // Users can sync with anyone (Government or User)
                true
            }
            _ => false,
        }
    }
}

pub fn subscribe_topics() -> Result<()> {
    Ok(())
}