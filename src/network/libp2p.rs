use libp2p::{
    gossipsub::{self, IdentTopic, MessageAuthenticity},
    identity::Keypair,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId, StreamProtocol, SwarmBuilder,
};
use serde::{Deserialize, Serialize};
use futures::prelude::*;
use std::{collections::{HashMap, HashSet}, time::Duration};
use anyhow::{Result, bail, anyhow};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use crate::types::{
    HASH, RealmId, GovernmentRuleLog, TaxObligationEnvelope, TaxReceiptEnvelope,
    ReceiptEnvelope, ChatMessageEnvelope, ChatMessage,
};
use crate::utils::serialize;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NetworkMessage {
    GovernmentLog { realm_id: RealmId, log: GovernmentRuleLog },
    TaxObligation(TaxObligationEnvelope),
    TaxReceipt(TaxReceiptEnvelope),
    P2PReceipt(ReceiptEnvelope),
    ChatMessage(ChatMessageEnvelope),
}

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub realm_id: RealmId,
    pub keypair: Keypair,
    pub bootstrap_peers: Vec<Multiaddr>,
    pub gossip_heartbeat: Duration,
    pub max_transmit_size: usize,
    pub government_public_key: Option<VerifyingKey>,
    pub is_government: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            realm_id: [0; 32],
            keypair: Keypair::generate_ed25519(),
            bootstrap_peers: vec![],
            gossip_heartbeat: Duration::from_secs(1),
            max_transmit_size: 1024 * 1024,
            government_public_key: None,
            is_government: false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum DirectRequest {
    Hello { realm_id: RealmId },
    GetLatestGovLog { realm_id: RealmId },
    SubmitTaxObligation { obligation: TaxObligationEnvelope },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum DirectResponse {
    HelloAck { realm_id: RealmId },
    GovernmentLog { log: Option<GovernmentRuleLog> },
    TaxObligationAck,
}

#[derive(NetworkBehaviour)]
pub struct RealmBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub request_response: request_response::cbor::Behaviour<DirectRequest, DirectResponse>,
}

#[derive(Debug, Clone)]
pub struct PeerState {
    pub last_seen: std::time::SystemTime,
    pub realm_id: Option<RealmId>,
    pub gov_log_version: Option<u64>,
    pub is_government: bool,
    pub handshake_complete: bool,
}

pub struct NodeState {
    pub latest_gov_log_version: u64,
    pub seen_messages: HashSet<HASH>,
    pub peer_states: HashMap<PeerId, PeerState>,
    pub pending_tax_obligations: Vec<TaxObligationEnvelope>,
    pub latest_government_log: Option<GovernmentRuleLog>,
    pub chat_messages: Vec<ChatMessage>,
    pub government_key_locked: bool,
    pub has_synced_government: bool,
}

pub struct P2PNode {
    pub config: NetworkConfig,
    pub swarm: libp2p::Swarm<RealmBehaviour>,
    pub state: NodeState,
    pub topics: RealmTopics,
}

pub struct RealmTopics {
    pub gov_logs: IdentTopic,
    pub tax_obligations: IdentTopic,
    pub tax_receipts: IdentTopic,
    pub p2p_transfers: IdentTopic,
    pub chat: IdentTopic,
}

impl RealmTopics {
    pub fn new(realm_id: &RealmId) -> Self {
        let realm_str = hex::encode(realm_id);
        Self {
            gov_logs: IdentTopic::new(format!("/realm/{}/gov-logs/1.0.0", realm_str)),
            tax_obligations: IdentTopic::new(format!("/realm/{}/tax-obligations/1.0.0", realm_str)),
            tax_receipts: IdentTopic::new(format!("/realm/{}/tax-receipts/1.0.0", realm_str)),
            p2p_transfers: IdentTopic::new(format!("/realm/{}/p2p-transfers/1.0.0", realm_str)),
            chat: IdentTopic::new(format!("/realm/{}/chat/1.0.0", realm_str)),
        }
    }
}

impl P2PNode {
    pub fn new(config: NetworkConfig) -> Result<Self> {
        let gossip_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(config.gossip_heartbeat)
            .build()
            .map_err(|e| anyhow!(e))?;

        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(config.keypair.clone()),
            gossip_config,
        )
        .map_err(|e| anyhow!(e))?;

        let protocols = std::iter::once((
            StreamProtocol::new("/realm/direct/1.0.0"),
            ProtocolSupport::Full,
        ));

        let req_resp_config = request_response::Config::default();
        let request_response = request_response::cbor::Behaviour::<DirectRequest, DirectResponse>::new(
            protocols,
            req_resp_config,
        );

        let behaviour = RealmBehaviour {
            gossipsub,
            request_response,
        };

        let topics = RealmTopics::new(&config.realm_id);
        let bootstrap_peers = config.bootstrap_peers.clone();
        let government_key_locked = config.government_public_key.is_some();

        let mut swarm = SwarmBuilder::with_existing_identity(config.keypair.clone())
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )?
            .with_behaviour(|_| behaviour)?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        for addr in &bootstrap_peers {
            swarm.dial(addr.clone())?;
        }

        Ok(Self {
            config,
            swarm,
            topics,
            state: NodeState {
                latest_gov_log_version: 0,
                seen_messages: HashSet::new(),
                peer_states: HashMap::new(),
                pending_tax_obligations: Vec::new(),
                latest_government_log: None,
                chat_messages: Vec::new(),
                government_key_locked,
                has_synced_government: false,
            },
        })
    }

    pub fn subscribe_topics(&mut self) -> Result<()> {
        self.swarm.behaviour_mut().gossipsub.subscribe(&self.topics.gov_logs)?;
        self.swarm.behaviour_mut().gossipsub.subscribe(&self.topics.tax_obligations)?;
        self.swarm.behaviour_mut().gossipsub.subscribe(&self.topics.tax_receipts)?;
        self.swarm.behaviour_mut().gossipsub.subscribe(&self.topics.p2p_transfers)?;
        self.swarm.behaviour_mut().gossipsub.subscribe(&self.topics.chat)?;
        Ok(())
    }

    pub fn publish_message(&mut self, message: NetworkMessage) -> Result<()> {
        let mut data = Vec::new();
        ciborium::into_writer(&message, &mut data)
            .map_err(|e| anyhow!("CBOR serialization failed: {}", e))?;
        
        if data.len() > self.config.max_transmit_size {
            bail!("Message too large: {} bytes", data.len());
        }

        let topic = match &message {
            NetworkMessage::GovernmentLog { .. } => {
                if !self.config.is_government {
                    bail!("Only government nodes can publish government logs");
                }
                &self.topics.gov_logs
            }
            NetworkMessage::TaxObligation(_) => &self.topics.tax_obligations,
            NetworkMessage::TaxReceipt(_) => &self.topics.tax_receipts,
            NetworkMessage::P2PReceipt(_) => &self.topics.p2p_transfers,
            NetworkMessage::ChatMessage(_) => &self.topics.chat,
        };
        
        self.swarm.behaviour_mut().gossipsub.publish(topic.clone(), data)?;
        Ok(())
    }

    pub fn send_direct_request(&mut self, peer: PeerId, request: DirectRequest) -> request_response::OutboundRequestId {
        self.swarm
            .behaviour_mut()
            .request_response
            .send_request(&peer, request)
    }

    fn verify_signature(&self, data: &[u8], signature: &[u8; 64], public_key: &VerifyingKey) -> Result<()> {
        let sig = Signature::from_bytes(signature);
        public_key
            .verify(data, &sig)
            .map_err(|_| anyhow!("Signature verification failed"))?;
        Ok(())
    }

    pub fn canonical_hash<T: Serialize>(&self, payload: &T) -> Result<HASH> {
        let mut serialized = Vec::new();
        ciborium::into_writer(payload, &mut serialized)
            .map_err(|e| anyhow!("Serialization failed: {}", e))?;
        Ok(*blake3::hash(&serialized).as_bytes())
    }

    fn is_peer_in_realm(&self, peer: &PeerId) -> bool {
        if let Some(peer_state) = self.state.peer_states.get(peer) {
            if let Some(peer_realm) = peer_state.realm_id {
                return peer_realm == self.config.realm_id && peer_state.handshake_complete;
            }
        }
        false
    }

    fn can_process_economic_action(&self) -> bool {
        self.state.latest_government_log.is_some()
    }

    async fn handle_behaviour_event(&mut self, event: RealmBehaviourEvent) -> Result<()> {
        match event {
            RealmBehaviourEvent::Gossipsub(gossipsub::Event::Message { message, propagation_source, .. }) => {
                if !self.is_peer_in_realm(&propagation_source) {
                    return Ok(());
                }

                match ciborium::from_reader::<NetworkMessage, _>(&message.data[..]) {
                    Ok(msg) => {
                        let hash_bytes: HASH = *blake3::hash(&message.data).as_bytes();
                        if !self.state.seen_messages.insert(hash_bytes) {
                            return Ok(());
                        }

                        match msg {
                            NetworkMessage::GovernmentLog { realm_id, log } => {
                                if realm_id == self.config.realm_id {
                                    if let Err(e) = self.handle_government_log(log, propagation_source).await {
                                        eprintln!("Government log handling error: {:?}", e);
                                    }
                                }
                            }
                            NetworkMessage::TaxObligation(envelope) => {
                                if self.can_process_economic_action() {
                                    if let Err(e) = self.handle_tax_obligation(envelope, propagation_source).await {
                                        eprintln!("Tax obligation handling error: {:?}", e);
                                    }
                                }
                            }
                            NetworkMessage::TaxReceipt(envelope) => {
                                if self.can_process_economic_action() {
                                    if let Err(e) = self.handle_tax_receipt(envelope, propagation_source).await {
                                        eprintln!("Tax receipt handling error: {:?}", e);
                                    }
                                }
                            }
                            NetworkMessage::P2PReceipt(envelope) => {
                                if self.can_process_economic_action() {
                                    if let Err(e) = self.handle_p2p_receipt(envelope, propagation_source).await {
                                        eprintln!("P2P receipt handling error: {:?}", e);
                                    }
                                }
                            }
                            NetworkMessage::ChatMessage(envelope) => {
                                if let Err(e) = self.handle_chat_message(envelope, propagation_source).await {
                                    eprintln!("Chat message handling error: {:?}", e);
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("Gossip Deserialization Error: {}", e),
                }
            }
            RealmBehaviourEvent::RequestResponse(request_response::Event::Message { peer, message }) => {
                match message {
                    request_response::Message::Request { request, channel, .. } => {
                        match self.process_direct_request(request, peer).await {
                            Ok(response) => {
                                let _ = self.swarm.behaviour_mut().request_response.send_response(channel, response);
                            }
                            Err(e) => {
                                eprintln!("Direct request processing error: {:?}", e);
                            }
                        }
                    }
                    request_response::Message::Response { response, .. } => {
                        if let Err(e) = self.process_direct_response(response, peer).await {
                            eprintln!("Direct response processing error: {:?}", e);
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(b_event) => {
                            if let Err(e) = self.handle_behaviour_event(b_event).await {
                                eprintln!("Behaviour error: {:?}", e);
                            }
                        }
                        SwarmEvent::NewListenAddr { address, .. } => {
                            println!("Listening on {:?}", address);
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            println!("Connected to {:?}", peer_id);
                            self.state.peer_states.insert(peer_id, PeerState {
                                last_seen: std::time::SystemTime::now(),
                                realm_id: None,
                                gov_log_version: None,
                                is_government: false,
                                handshake_complete: false,
                            });
                            
                            let _ = self.send_direct_request(peer_id, DirectRequest::Hello { realm_id: self.config.realm_id });
                            
                            if !self.state.has_synced_government {
                                let _ = self.send_direct_request(peer_id, DirectRequest::GetLatestGovLog { realm_id: self.config.realm_id });
                            }
                        }
                        SwarmEvent::ConnectionClosed { peer_id, .. } => {
                            println!("Disconnected from {:?}", peer_id);
                            self.state.peer_states.remove(&peer_id);
                        }
                        SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                            eprintln!("Connection to {:?} failed: {:?}", peer_id, error);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    pub fn listen_on(&mut self, addr: Multiaddr) -> Result<()> {
        self.swarm.listen_on(addr)?;
        Ok(())
    }

   async fn handle_government_log(&mut self, log: GovernmentRuleLog, source: PeerId) -> Result<()> {
        if log.realm_id != self.config.realm_id {
            bail!("Government log realm mismatch");
        }

        if self.state.latest_government_log.is_none() && log.version_number != 0 {
            bail!("First government log must be genesis (version 0)");
        }

        if let Some(_) = self.state.latest_government_log {
            if log.version_number <= self.state.latest_gov_log_version {
                return Ok(());
            }
            if log.version_number != self.state.latest_gov_log_version + 1 {
                bail!(
                    "Government log version gap detected: expected {}, got {}",
                    self.state.latest_gov_log_version + 1,
                    log.version_number
                );
            }
        }

        let mut unsigned_log = log.clone();
        unsigned_log.log_hash = [0; 32];
        unsigned_log.government_signature = [0; 64];
        let computed_hash = blake3::hash(&serialize(&unsigned_log)?);
        if computed_hash.as_bytes() != &log.log_hash {
            bail!("Government log hash mismatch");
        }

        if self.state.government_key_locked {
            if let Some(gov_key) = &self.config.government_public_key {
                self.verify_signature(&log.log_hash, &log.government_signature, gov_key)?;
            }
        } else {
            let gov_key = &log.government_public_key;
            self.verify_signature(&log.log_hash, &log.government_signature, gov_key)?;

            if self.state.government_key_locked {
                if let Some(locked_key) = &self.config.government_public_key {
                    if locked_key != gov_key {
                        bail!("Government public key mismatch");
                    }
                }
            } else {
                self.config.government_public_key = Some(*gov_key);
                self.state.government_key_locked = true;
            }
        }

        self.state.latest_gov_log_version = log.version_number;
        self.state.latest_government_log = Some(log.clone());
        self.state.has_synced_government = true;

        if let Some(peer_state) = self.state.peer_states.get_mut(&source) {
            peer_state.gov_log_version = Some(log.version_number);
            peer_state.is_government = true;
        }

        println!("Updated government log to version {}", log.version_number);

        self.relay_pending_obligations().await?;
        Ok(())
    }


    async fn handle_tax_obligation(&mut self, envelope: TaxObligationEnvelope, source: PeerId) -> Result<()> {
        if envelope.realm_id != self.config.realm_id {
            bail!("Wrong realm for tax obligation");
        }

        let obligation = &envelope.obligation;

        let mut unsigned = obligation.clone();
        unsigned.user_signature = [0; 64];
        unsigned.obligation_hash = [0; 32];

        let computed_hash: HASH = *blake3::hash(&serialize(&unsigned)?).as_bytes();

        if computed_hash != obligation.obligation_hash {
            bail!("Invalid obligation hash");
        }

        self.verify_signature(&computed_hash, &obligation.user_signature, &obligation.user_id)?;

        if self.config.is_government {
            self.state.pending_tax_obligations.push(envelope);
            println!("Stored tax obligation from {:?}", source);
        }

        Ok(())
    }

     async fn handle_tax_receipt(&mut self, envelope: TaxReceiptEnvelope, source: PeerId) -> Result<()> {
        if envelope.realm_id != self.config.realm_id {
            bail!("Wrong realm for tax receipt");
        }

        let receipt = &envelope.receipt;

        if let Some(gov_key) = &self.config.government_public_key {
            let mut unsigned_receipt = receipt.clone();
            unsigned_receipt.government_signature = [0; 64];

            let receipt_hash = blake3::hash(&serialize(&unsigned_receipt)?);
            self.verify_signature(receipt_hash.as_bytes(), &receipt.government_signature, gov_key)?;
        }

        let hash_bytes: HASH = self.canonical_hash(&receipt)?;
        if !self.state.seen_messages.insert(hash_bytes) {
            return Ok(());
        }

        self.state.pending_tax_obligations.retain(|o| o.obligation.obligation_hash != receipt.obligation_hash);
        println!("Received valid tax receipt from {:?}: {} coins transferred", source, receipt.coins_transferred);

        Ok(())
    }
    
    async fn handle_p2p_receipt(&mut self, envelope: ReceiptEnvelope, source: PeerId) -> Result<()> {
        if envelope.realm_id != self.config.realm_id {
            bail!("Wrong realm for P2P receipt");
        }

        let receipt = &envelope.receipt;

        // Verify hash
        let mut temp_receipt = receipt.clone();
        temp_receipt.sender_signature = [0; 64];
        temp_receipt.receiver_signature = [0; 64];
        temp_receipt.tx_hash = [0; 32];

        let expected_hash = blake3::hash(&serialize(&temp_receipt)?);
        if expected_hash.as_bytes() != &receipt.tx_hash {
            bail!("Receipt hash verification failed");
        }

        self.verify_signature(&receipt.tx_hash, &receipt.sender_signature, &receipt.sender)?;
        self.verify_signature(&receipt.tx_hash, &receipt.receiver_signature, &receipt.receiver)?;

        let hash_bytes: HASH = self.canonical_hash(&receipt)?;
        if !self.state.seen_messages.insert(hash_bytes) {
            return Ok(());
        }

        println!(
            "Processed P2P transfer from {:?}: {} -> {}",
            source,
            hex::encode(&receipt.sender.as_bytes()[..8]),
            hex::encode(&receipt.receiver.as_bytes()[..8])
        );

        Ok(())
    }

    async fn handle_chat_message(&mut self, envelope: ChatMessageEnvelope, source: PeerId) -> Result<()> {
        if envelope.realm_id != self.config.realm_id {
            bail!("Wrong realm for chat message");
        }

        let message = &envelope.message;

        let mut unsigned_message = message.clone();
        unsigned_message.signature = [0; 64];
        unsigned_message.message_hash = [0; 32];
        let computed_hash: HASH = *blake3::hash(&serialize(&unsigned_message)?).as_bytes();
        if computed_hash != message.message_hash {
            bail!("Chat message hash mismatch");
        }

        self.verify_signature(&message.message_hash, &message.signature, &message.sender)?;

        let hash_bytes: HASH = self.canonical_hash(&envelope)?;
        if !self.state.seen_messages.insert(hash_bytes) {
            return Ok(());
        }

        self.state.chat_messages.push(message.clone());
        println!("Received chat message from {:?}: {}", source, message.content);
        Ok(())
    }


    async fn process_direct_request(&mut self, request: DirectRequest, peer: PeerId) -> Result<DirectResponse> {
        match request {
            DirectRequest::Hello { realm_id } => {
                if let Some(peer_state) = self.state.peer_states.get_mut(&peer) {
                    peer_state.realm_id = Some(realm_id);
                    peer_state.handshake_complete = true;
                    peer_state.last_seen = std::time::SystemTime::now();
                }
                println!("Handshake complete with {:?} (realm: {})", peer, hex::encode(&realm_id[..4]));
                Ok(DirectResponse::HelloAck { realm_id: self.config.realm_id })
            }
            DirectRequest::GetLatestGovLog { realm_id } => {
                if realm_id != self.config.realm_id {
                    return Ok(DirectResponse::GovernmentLog { log: None });
                }
                Ok(DirectResponse::GovernmentLog { 
                    log: self.state.latest_government_log.clone() 
                })
            }
            DirectRequest::SubmitTaxObligation { obligation } => {
                if self.config.is_government {
                    if let Err(e) = self.handle_tax_obligation(obligation, peer).await {
                        eprintln!("Failed to process tax obligation: {:?}", e);
                    }
                }
                Ok(DirectResponse::TaxObligationAck)
            }
        }
    }

    async fn process_direct_response(&mut self, response: DirectResponse, peer: PeerId) -> Result<()> {
        match response {
            DirectResponse::HelloAck { realm_id } => {
                if let Some(peer_state) = self.state.peer_states.get_mut(&peer) {
                    peer_state.realm_id = Some(realm_id);
                    peer_state.handshake_complete = true;
                }
                println!("Handshake ACK from {:?} (realm: {})", peer, hex::encode(&realm_id[..4]));
            }
            DirectResponse::GovernmentLog { log: Some(log) } => {
                if log.version_number > self.state.latest_gov_log_version {
                    if let Err(e) = self.handle_government_log(log, peer).await {
                        eprintln!("Failed to apply synced government log: {:?}", e);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn relay_pending_obligations(&mut self) -> Result<()> {
        let gov_peers: Vec<PeerId> = self.state.peer_states.iter()
            .filter(|(_, s)| s.is_government && s.handshake_complete)
            .map(|(peer_id, _)| *peer_id)
            .collect();

        if gov_peers.is_empty() {
            return Ok(());
        }

        let obligations = self.state.pending_tax_obligations.clone();

        for peer in gov_peers {
            for obligation in &obligations {
                let _ = self.send_direct_request(
                    peer, 
                    DirectRequest::SubmitTaxObligation { obligation: obligation.clone() }
                );
            }
        }

        Ok(())
    }

    pub fn compute_message_hash<T: Serialize>(&self, msg: &T) -> Result<HASH> {
        self.canonical_hash(msg)
    }

    pub fn local_peer_id(&self) -> &PeerId {
        self.swarm.local_peer_id()
    }

    pub fn get_chat_messages(&self) -> &[ChatMessage] {
        &self.state.chat_messages
    }

    pub fn get_realm_peers(&self) -> Vec<PeerId> {
        self.state.peer_states.iter()
            .filter(|(_, state)| {
                state.realm_id == Some(self.config.realm_id) && state.handshake_complete
            })
            .map(|(peer_id, _)| *peer_id)
            .collect()
    }
}


#[cfg(test)]
mod p2p_node_tests {
    use crate::types::{ActionRule, P2PTransferReceipt, ResourceDelta, Resources, TaxObligation, TaxReceipt};

    use super::*;
    use std::collections::BTreeMap;
    use ed25519_dalek::{SigningKey,Signer};
    use rand::rngs::OsRng;

    // --- Helpers ---

    fn setup_keys() -> (SigningKey, VerifyingKey) {
        let sk = SigningKey::generate(&mut OsRng);
        let vk = sk.verifying_key();
        (sk, vk)
    }

    fn mock_gov_log(realm_id: RealmId, version: u64, sk: &SigningKey) -> GovernmentRuleLog {
        let vk = sk.verifying_key();
        let mut log = GovernmentRuleLog {
            version_number: version,
            effective_slot: 100,
            rules: BTreeMap::new(),
            tasks: Vec::new(),
            tax_rate_percent: 10,
            tax_deadline: 1000,
            previous_log_hash: None,
            log_hash: [0; 32],
            government_signature: [0; 64],
            realm_id,
            government_public_key: vk,
        };

        let mut unsigned = log.clone();
        unsigned.log_hash = [0; 32];
        unsigned.government_signature = [0; 64];
        
        let mut data = Vec::new();
        ciborium::into_writer(&unsigned, &mut data).unwrap();
        let hash = blake3::hash(&data);
        
        log.log_hash = *hash.as_bytes();
        log.government_signature = sk.sign(hash.as_bytes()).to_bytes();
        log
    }

    fn setup_node(realm_id: RealmId, is_gov: bool, gov_vk: Option<VerifyingKey>) -> P2PNode {
        let config = NetworkConfig {
            realm_id,
            keypair: Keypair::generate_ed25519(),
            bootstrap_peers: vec![],
            gossip_heartbeat: Duration::from_millis(10),
            max_transmit_size: 1024,
            government_public_key: gov_vk,
            is_government: is_gov,
        };
        P2PNode::new(config).expect("Failed to create node")
    }

    // --- Tests ---

    #[tokio::test]
    async fn test_node_initialization() {
        let realm_id = [1; 32];
        let node = setup_node(realm_id, false, None);
        
        assert_eq!(node.config.realm_id, realm_id);
        assert!(!node.state.government_key_locked);
        assert!(!node.state.has_synced_government);
    }

    #[tokio::test]
    async fn test_handshake_logic() {
        let realm_id = [1; 32];
        let mut node = setup_node(realm_id, false, None);
        let peer = PeerId::random();

        // Simulate ConnectionEstablished entry
        node.state.peer_states.insert(peer, PeerState {
            last_seen: std::time::SystemTime::now(),
            realm_id: None,
            gov_log_version: None,
            is_government: false,
            handshake_complete: false,
        });

        // Process Hello Request
        let req = DirectRequest::Hello { realm_id };
        let res = node.process_direct_request(req, peer).await.unwrap();

        match res {
            DirectResponse::HelloAck { realm_id: ack_realm } => assert_eq!(ack_realm, realm_id),
            _ => panic!("Expected HelloAck"),
        }

        let peer_state = node.state.peer_states.get(&peer).unwrap();
        assert!(peer_state.handshake_complete);
        assert_eq!(peer_state.realm_id, Some(realm_id));
    }

   #[tokio::test]
async fn test_government_log_versioning_and_signatures() {
    let realm_id = [1u8; 32];
    let (sk, vk) = setup_keys();
    let mut node = setup_node(realm_id, false, Some(vk));

    // Use separate peers for clarity
    let peer0 = PeerId::random();
    let peer1 = PeerId::random();
    let peer2 = PeerId::random();

    // -----------------------------
    // Set up initial peer states FIRST (CRITICAL!)
    // -----------------------------
    // Create initial peer states (like handshake completed)
    node.state.peer_states.insert(
        peer0,
        PeerState {
            last_seen: std::time::SystemTime::now(),
            realm_id: Some(realm_id),
            gov_log_version: None,
            is_government: false,
            handshake_complete: true,
        },
    );
    
    node.state.peer_states.insert(
        peer1,
        PeerState {
            last_seen: std::time::SystemTime::now(),
            realm_id: Some(realm_id),
            gov_log_version: None,
            is_government: false,
            handshake_complete: true,
        },
    );
    
    node.state.peer_states.insert(
        peer2,
        PeerState {
            last_seen: std::time::SystemTime::now(),
            realm_id: Some(realm_id),
            gov_log_version: None,
            is_government: false,
            handshake_complete: true,
        },
    );

    // -----------------------------
    // 1. Valid Genesis Log (v0)
    // -----------------------------
    let log_v0 = mock_gov_log(realm_id, 0, &sk);
    let res = node.handle_government_log(log_v0.clone(), peer0).await;
    assert!(res.is_ok(), "Genesis log should be accepted");
    assert_eq!(node.state.latest_gov_log_version, 0);

    // Check peer0 is marked as government
    let peer_state = node
        .state
        .peer_states
        .get(&peer0)
        .expect("Peer0 should still be in peer states");
    assert!(peer_state.is_government, "Genesis peer should be marked as government");
    assert_eq!(peer_state.gov_log_version, Some(0), "Peer0 should have gov_log_version=0");

    // -----------------------------
    // 2. Invalid version (gap: v2)
    // -----------------------------
    let log_v2 = mock_gov_log(realm_id, 2, &sk);
    let res = node.handle_government_log(log_v2, peer2).await;
    assert!(res.is_err(), "Version gap should be rejected");

    // peer2 should NOT be marked as government since the log was rejected
    let peer_state = node
        .state
        .peer_states
        .get(&peer2)
        .expect("Peer2 should still be in peer states");
    assert!(!peer_state.is_government, "Peer2 should NOT be marked as government after rejected log");
    assert_eq!(peer_state.gov_log_version, None, "Peer2 should not have gov_log_version after rejection");

    // -----------------------------
    // 3. Valid v1 log
    // -----------------------------
    let log_v1 = mock_gov_log(realm_id, 1, &sk);
    let res = node.handle_government_log(log_v1.clone(), peer1).await;
    assert!(res.is_ok(), "v1 log should be accepted after genesis");
    assert_eq!(node.state.latest_gov_log_version, 1);

    // peer1 should be marked as government
    let peer_state = node
        .state
        .peer_states
        .get(&peer1)
        .expect("Peer1 should still be in peer states");
    assert!(peer_state.is_government, "Peer1 should be marked government after valid log");
    assert_eq!(peer_state.gov_log_version, Some(1), "Peer1 should have gov_log_version=1");
}

    #[tokio::test]
    async fn test_economic_gate_protection() {
        let realm_id = [1; 32];
        let node = setup_node(realm_id, false, None);

        // Should be false initially
        assert!(!node.can_process_economic_action());

        // Process economic message (should be ignored in real event loop)
        // This confirms the internal logic gate
    }

    #[tokio::test]
    async fn test_duplicate_message_prevention() {
        let realm_id = [1; 32];
        let mut node = setup_node(realm_id, false, None);
        let hash: HASH = [7; 32];

        assert!(node.state.seen_messages.insert(hash));
        assert!(!node.state.seen_messages.insert(hash)); // Second attempt fails
    }

    #[tokio::test]
    async fn test_malformed_government_log_signature() {
        let realm_id = [1; 32];
        let (sk, vk) = setup_keys();
        let (bad_sk, _) = setup_keys();
        let mut node = setup_node(realm_id, false, Some(vk));

        // Signed with wrong key
        let bad_log = mock_gov_log(realm_id, 0, &bad_sk);
        let res = node.handle_government_log(bad_log, PeerId::random()).await;
        
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Signature verification failed"));
    }

    #[tokio::test]
    async fn test_publish_permissions() {
        let realm_id = [1; 32];
        let (sk, _) = setup_keys();
        
        // Normal node trying to publish gov log
        let mut node = setup_node(realm_id, false, None);
        let log = mock_gov_log(realm_id, 0, &sk);
        let msg = NetworkMessage::GovernmentLog { realm_id, log };

        let res = node.publish_message(msg);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Only government nodes"));
    }

    #[tokio::test]
    async fn test_realm_isolation() {
        let realm_id_a = [1; 32];
        let realm_id_b = [2; 32];
        let node_a = setup_node(realm_id_a, false, None);
        let peer_b = PeerId::random();

        // Node A gets message from Peer B who is in Realm B
        let mut state_b = PeerState {
            last_seen: std::time::SystemTime::now(),
            realm_id: Some(realm_id_b),
            gov_log_version: None,
            is_government: false,
            handshake_complete: true,
        };
        
        // This helper should return false
        assert!(!node_a.is_peer_in_realm(&peer_b));
    }

    #[cfg(test)]


    // ========================================================================
    // HELPER FUNCTIONS
    // ========================================================================

    fn create_test_keypair() -> (SigningKey, VerifyingKey) {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = VerifyingKey::from(&signing_key);
        (signing_key, verifying_key)
    }

    fn create_test_realm_id() -> RealmId {
        let mut realm = [0u8; 32];
        realm[0] = 1;
        realm[1] = 2;
        realm[2] = 3;
        realm
    }

    fn create_government_log(
        version: u64,
        realm_id: RealmId,
        signing_key: &SigningKey,
    ) -> GovernmentRuleLog {
        let mut rules = BTreeMap::new();
        rules.insert(
            "harvest:wheat".to_string(),
            ActionRule {
                energy_cost: 5,
                resource_reward: ResourceDelta { wheat: 50, wood: 0, coins: 20 },
                maturation_iterations: 1000,
            },
        );

        let verifying_key = VerifyingKey::from(signing_key);

        let mut log = GovernmentRuleLog {
            version_number: version,
            effective_slot: 0,
            rules,
            tasks: vec![],
            tax_rate_percent: 10,
            tax_deadline: 100,
            previous_log_hash: if version > 0 { Some([1; 32]) } else { None },
            log_hash: [0; 32],
            government_signature: [0; 64],
            realm_id,
            government_public_key: verifying_key,
        };

        let mut unsigned = log.clone();
        unsigned.log_hash = [0; 32];
        unsigned.government_signature = [0; 64];

        let hash = blake3::hash(&serialize(&unsigned).unwrap());
        log.log_hash = *hash.as_bytes();
        log.government_signature = signing_key.sign(hash.as_bytes()).to_bytes();

        log
    }

    fn create_tax_obligation(
        user_key: &SigningKey,
        gov_log: &GovernmentRuleLog,
        coin_amount: u64,
        sequence: u64,
    ) -> TaxObligation {
        let verifying_key = VerifyingKey::from(user_key);

        let mut obligation = TaxObligation {
            realm_id: gov_log.realm_id,
            event_hash: [1; 32],
            user_id: verifying_key,
            coin_amount,
            government_log: gov_log.clone(),
            sequence_number: sequence,
            previous_receipt_hash: None,
            user_signature: [0; 64],
            obligation_hash: [0; 32],
        };

        let mut unsigned = obligation.clone();
        unsigned.user_signature = [0; 64];
        unsigned.obligation_hash = [0; 32];

        let hash = blake3::hash(&serialize(&unsigned).unwrap());
        obligation.obligation_hash = *hash.as_bytes();
        obligation.user_signature = user_key.sign(hash.as_bytes()).to_bytes();

        obligation
    }

    fn create_tax_receipt(
        gov_key: &SigningKey,
        obligation: &TaxObligation,
        payment_slot: u64,
    ) -> TaxReceipt {
        let mut receipt = TaxReceipt {
            obligation_hash: obligation.obligation_hash,
            previous_receipt_hash: None,
            payment_slot,
            coins_transferred: obligation.coin_amount,
            government_signature: [0; 64],
        };

        let mut unsigned = receipt.clone();
        unsigned.government_signature = [0; 64];

        let hash = blake3::hash(&serialize(&unsigned).unwrap());
        receipt.government_signature = gov_key.sign(hash.as_bytes()).to_bytes();

        receipt
    }

    fn create_p2p_receipt(
        sender_key: &SigningKey,
        receiver_key: &SigningKey,
        realm_id: RealmId,
        delta: ResourceDelta,
    ) -> P2PTransferReceipt {
        let sender_pub = VerifyingKey::from(sender_key);
        let receiver_pub = VerifyingKey::from(receiver_key);

        let sender_before = Resources { wheat: 100, wood: 50, coins: 200 };
        let receiver_before = Resources { wheat: 50, wood: 25, coins: 100 };

        let sender_after = Resources {
            wheat: sender_before.wheat.saturating_add_signed(-delta.wheat),
            wood: sender_before.wood.saturating_add_signed(-delta.wood),
            coins: sender_before.coins.saturating_add_signed(-delta.coins),
        };

        let receiver_after = Resources {
            wheat: receiver_before.wheat.saturating_add_signed(delta.wheat),
            wood: receiver_before.wood.saturating_add_signed(delta.wood),
            coins: receiver_before.coins.saturating_add_signed(delta.coins),
        };

        let mut receipt = P2PTransferReceipt {
            realm_id,
            tx_hash: [0; 32],
            sender: sender_pub,
            receiver: receiver_pub,
            delta,
            sender_prev_state_hash: [2; 32],
            receiver_prev_state_hash: [3; 32],
            sender_balance_before: sender_before,
            sender_balance_after: sender_after,
            receiver_balance_before: receiver_before,
            receiver_balance_after: receiver_after,
            nonce: 1,
            sender_signature: [0; 64],
            receiver_signature: [0; 64],
        };

        let mut unsigned = receipt.clone();
        unsigned.sender_signature = [0; 64];
        unsigned.receiver_signature = [0; 64];
        unsigned.tx_hash = [0; 32];

        let hash = blake3::hash(&serialize(&unsigned).unwrap());
        receipt.tx_hash = *hash.as_bytes();
        receipt.sender_signature = sender_key.sign(hash.as_bytes()).to_bytes();
        receipt.receiver_signature = receiver_key.sign(hash.as_bytes()).to_bytes();

        receipt
    }

    fn create_chat_message(
        sender_key: &SigningKey,
        content: String,
        nonce: u64,
    ) -> ChatMessage {
        let sender_pub = VerifyingKey::from(sender_key);

        let mut message = ChatMessage {
            message_id: format!("msg_{}", nonce),
            sender: sender_pub,
            receiver: None,
            content,
            nonce,
            signature: [0; 64],
            message_hash: [0; 32],
        };

        let mut unsigned = message.clone();
        unsigned.signature = [0; 64];
        unsigned.message_hash = [0; 32];

        let hash = blake3::hash(&serialize(&unsigned).unwrap());
        message.message_hash = *hash.as_bytes();
        message.signature = sender_key.sign(hash.as_bytes()).to_bytes();

        message
    }

    // ========================================================================
    // CANONICAL HASH TESTS
    // ========================================================================

    #[test]
    fn test_canonical_hash_consistency() {
        let config = NetworkConfig::default();
        let node = P2PNode::new(config).unwrap();

        let (sender_key, _) = create_test_keypair();
        let (receiver_key, _) = create_test_keypair();
        let realm_id = create_test_realm_id();

        let delta = ResourceDelta { wheat: 10, wood: 5, coins: 20 };
        let receipt = create_p2p_receipt(&sender_key, &receiver_key, realm_id, delta);

        let hash1 = node.canonical_hash(&receipt).unwrap();
        let hash2 = node.canonical_hash(&receipt).unwrap();

        assert_eq!(hash1, hash2, "Canonical hash should be deterministic");
    }

    #[test]
    fn test_canonical_hash_different_for_different_data() {
        let config = NetworkConfig::default();
        let node = P2PNode::new(config).unwrap();

        let (sender_key, _) = create_test_keypair();
        let (receiver_key, _) = create_test_keypair();
        let realm_id = create_test_realm_id();

        let delta1 = ResourceDelta { wheat: 10, wood: 5, coins: 20 };
        let delta2 = ResourceDelta { wheat: 15, wood: 5, coins: 20 };

        let receipt1 = create_p2p_receipt(&sender_key, &receiver_key, realm_id, delta1);
        let receipt2 = create_p2p_receipt(&sender_key, &receiver_key, realm_id, delta2);

        let hash1 = node.canonical_hash(&receipt1).unwrap();
        let hash2 = node.canonical_hash(&receipt2).unwrap();

        assert_ne!(hash1, hash2, "Different data should produce different hashes");
    }
#[tokio::test]
async fn test_government_log_genesis_validation() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);

    let mut node = P2PNode::new(config).unwrap();
    let log = create_government_log(0, realm_id, &gov_key);

    let result = node.handle_government_log(log, PeerId::random()).await;
    assert!(result.is_ok(), "Genesis log (version 0) should be accepted");
    assert_eq!(node.state.latest_gov_log_version, 0);
}

#[tokio::test]
async fn test_government_log_rejects_non_genesis_first() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);

    let mut node = P2PNode::new(config).unwrap();
    let log = create_government_log(1, realm_id, &gov_key);

    let result = node.handle_government_log(log, PeerId::random()).await;
    assert!(result.is_err(), "Non-genesis first log should be rejected");
    assert!(result.unwrap_err().to_string().contains("genesis"));
}

#[tokio::test]
async fn test_government_log_sequential_versions() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);

    let mut node = P2PNode::new(config).unwrap();

    let log0 = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(log0, PeerId::random()).await.unwrap();

    let log1 = create_government_log(1, realm_id, &gov_key);
    node.handle_government_log(log1, PeerId::random()).await.unwrap();

    assert_eq!(node.state.latest_gov_log_version, 1);
}

#[tokio::test]
async fn test_government_log_rejects_version_gap() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);

    let mut node = P2PNode::new(config).unwrap();

    let log0 = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(log0, PeerId::random()).await.unwrap();

    let log3 = create_government_log(3, realm_id, &gov_key);
    let result = node.handle_government_log(log3, PeerId::random()).await;

    assert!(result.is_err(), "Version gap should be rejected");
    assert!(result.unwrap_err().to_string().contains("gap"));
}

#[tokio::test]
async fn test_government_log_rejects_duplicate_version() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);

    let mut node = P2PNode::new(config).unwrap();

    let log0 = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(log0.clone(), PeerId::random()).await.unwrap();

    let result = node.handle_government_log(log0, PeerId::random()).await;
    assert!(result.is_ok(), "Duplicate version should be silently ignored");
    assert_eq!(node.state.latest_gov_log_version, 0);
}

#[tokio::test]
async fn test_government_log_wrong_realm() {
    let realm_id = create_test_realm_id();
    let mut wrong_realm = realm_id;
    wrong_realm[0] = 99;

    let (gov_key, gov_pub) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);

    let mut node = P2PNode::new(config).unwrap();
    let log = create_government_log(0, wrong_realm, &gov_key);

    let result = node.handle_government_log(log, PeerId::random()).await;
    assert!(result.is_err(), "Wrong realm should be rejected");
    assert!(result.unwrap_err().to_string().contains("realm"));
}

#[tokio::test]
async fn test_government_log_invalid_signature() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();
    let (wrong_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);

    let mut node = P2PNode::new(config).unwrap();
    let mut log = create_government_log(0, realm_id, &gov_key);

    log.government_signature = wrong_key.sign(&log.log_hash).to_bytes();

    let result = node.handle_government_log(log, PeerId::random()).await;
    assert!(result.is_err(), "Invalid signature should be rejected");
}

#[tokio::test]
async fn test_government_log_tampered_hash() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);

    let mut node = P2PNode::new(config).unwrap();
    let mut log = create_government_log(0, realm_id, &gov_key);

    log.tax_rate_percent = 50;

    let result = node.handle_government_log(log, PeerId::random()).await;
    assert!(result.is_err(), "Tampered data should be rejected");
}

#[tokio::test]
async fn test_government_key_locking_on_first_log() {
    let realm_id = create_test_realm_id();
    let (gov_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = None;

    let mut node = P2PNode::new(config).unwrap();
    assert!(!node.state.government_key_locked);

    let log = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(log, PeerId::random()).await.unwrap();

    assert!(node.state.government_key_locked);
    assert!(node.config.government_public_key.is_some());
}

// ========================================================================
// TAX OBLIGATION TESTS
// ========================================================================

#[tokio::test]
async fn test_tax_obligation_valid() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();
    let (user_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);
    config.is_government = true;

    let mut node = P2PNode::new(config).unwrap();

    let gov_log = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(gov_log.clone(), PeerId::random()).await.unwrap();

    let obligation = create_tax_obligation(&user_key, &gov_log, 100, 1);
    let envelope = TaxObligationEnvelope {
        realm_id,
        obligation,
        timestamp: 12345,
    };

    let result = node.handle_tax_obligation(envelope, PeerId::random()).await;
    assert!(result.is_ok());
    assert_eq!(node.state.pending_tax_obligations.len(), 1);
}

#[tokio::test]
async fn test_tax_obligation_wrong_realm() {
    let realm_id = create_test_realm_id();
    let mut wrong_realm = realm_id;
    wrong_realm[0] = 99;

    let (gov_key, gov_pub) = create_test_keypair();
    let (user_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);
    config.is_government = true;

    let mut node = P2PNode::new(config).unwrap();

    let gov_log = create_government_log(0, realm_id, &gov_key);
    let obligation = create_tax_obligation(&user_key, &gov_log, 100, 1);
    let envelope = TaxObligationEnvelope {
        realm_id: wrong_realm,
        obligation,
        timestamp: 12345,
    };

    let result = node.handle_tax_obligation(envelope, PeerId::random()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("realm"));
}

#[tokio::test]
async fn test_tax_obligation_invalid_hash() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();
    let (user_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);
    config.is_government = true;

    let mut node = P2PNode::new(config).unwrap();

    let gov_log = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(gov_log.clone(), PeerId::random()).await.unwrap();

    let mut obligation = create_tax_obligation(&user_key, &gov_log, 100, 1);
    obligation.coin_amount = 200;

    let envelope = TaxObligationEnvelope {
        realm_id,
        obligation,
        timestamp: 12345,
    };

    let result = node.handle_tax_obligation(envelope, PeerId::random()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("hash"));
}

#[tokio::test]
async fn test_tax_obligation_invalid_signature() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();
    let (user_key, _) = create_test_keypair();
    let (wrong_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);
    config.is_government = true;

    let mut node = P2PNode::new(config).unwrap();

    let gov_log = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(gov_log.clone(), PeerId::random()).await.unwrap();

    let mut obligation = create_tax_obligation(&user_key, &gov_log, 100, 1);
    obligation.user_signature = wrong_key.sign(&obligation.obligation_hash).to_bytes();

    let envelope = TaxObligationEnvelope {
        realm_id,
        obligation,
        timestamp: 12345,
    };

    let result = node.handle_tax_obligation(envelope, PeerId::random()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Signature"));
}

#[tokio::test]
async fn test_tax_obligation_non_government_node_ignores() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();
    let (user_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);
    config.is_government = false;

    let mut node = P2PNode::new(config).unwrap();

    let gov_log = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(gov_log.clone(), PeerId::random()).await.unwrap();

    let obligation = create_tax_obligation(&user_key, &gov_log, 100, 1);
    let envelope = TaxObligationEnvelope {
        realm_id,
        obligation,
        timestamp: 12345,
    };

    let result = node.handle_tax_obligation(envelope, PeerId::random()).await;
    assert!(result.is_ok());
    assert_eq!(node.state.pending_tax_obligations.len(), 0);
}

// ========================================================================
// TAX RECEIPT TESTS
// ========================================================================

#[tokio::test]
async fn test_tax_receipt_valid() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();
    let (user_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);
    config.is_government = true;

    let mut node = P2PNode::new(config).unwrap();

    let gov_log = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(gov_log.clone(), PeerId::random()).await.unwrap();

    let obligation = create_tax_obligation(&user_key, &gov_log, 100, 1);
    let obligation_envelope = TaxObligationEnvelope {
        realm_id,
        obligation: obligation.clone(),
        timestamp: 12345,
    };
    node.handle_tax_obligation(obligation_envelope, PeerId::random()).await.unwrap();

    let receipt = create_tax_receipt(&gov_key, &obligation, 50);
    let receipt_envelope = TaxReceiptEnvelope {
        realm_id,
        receipt,
        timestamp: 12346,
    };

    let result = node.handle_tax_receipt(receipt_envelope, PeerId::random()).await;
    assert!(result.is_ok());
    assert_eq!(node.state.pending_tax_obligations.len(), 0);
}

#[tokio::test]
async fn test_tax_receipt_removes_matching_obligation() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();
    let (user_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);
    config.is_government = true;

    let mut node = P2PNode::new(config).unwrap();

    let gov_log = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(gov_log.clone(), PeerId::random()).await.unwrap();

    let obligation1 = create_tax_obligation(&user_key, &gov_log, 100, 1);
    let obligation2 = create_tax_obligation(&user_key, &gov_log, 200, 2);

    node.handle_tax_obligation(
        TaxObligationEnvelope {
            realm_id,
            obligation: obligation1.clone(),
            timestamp: 12345,
        },
        PeerId::random(),
    ).await.unwrap();

    node.handle_tax_obligation(
        TaxObligationEnvelope {
            realm_id,
            obligation: obligation2.clone(),
            timestamp: 12346,
        },
        PeerId::random(),
    ).await.unwrap();

    assert_eq!(node.state.pending_tax_obligations.len(), 2);

    let receipt = create_tax_receipt(&gov_key, &obligation1, 50);
    node.handle_tax_receipt(
        TaxReceiptEnvelope {
            realm_id,
            receipt,
            timestamp: 12347,
        },
        PeerId::random(),
    ).await.unwrap();

    assert_eq!(node.state.pending_tax_obligations.len(), 1);
    assert_eq!(node.state.pending_tax_obligations[0].obligation.sequence_number, 2);
}

#[tokio::test]
async fn test_tax_receipt_deduplication() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();
    let (user_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);

    let mut node = P2PNode::new(config).unwrap();

    let gov_log = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(gov_log.clone(), PeerId::random()).await.unwrap();

    let obligation = create_tax_obligation(&user_key, &gov_log, 100, 1);
    let receipt = create_tax_receipt(&gov_key, &obligation, 50);
    let envelope = TaxReceiptEnvelope {
        realm_id,
        receipt: receipt.clone(),
        timestamp: 12345,
    };

    node.handle_tax_receipt(envelope.clone(), PeerId::random()).await.unwrap();
    let result = node.handle_tax_receipt(envelope, PeerId::random()).await;

    assert!(result.is_ok());
    let hash = node.canonical_hash(&receipt).unwrap();
    assert!(node.state.seen_messages.contains(&hash));
}

// ========================================================================
// P2P RECEIPT TESTS
// ========================================================================

#[tokio::test]
async fn test_p2p_receipt_valid() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();
    let (sender_key, _) = create_test_keypair();
    let (receiver_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);

    let mut node = P2PNode::new(config).unwrap();

    let gov_log = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(gov_log, PeerId::random()).await.unwrap();

    let delta = ResourceDelta { wheat: 10, wood: 5, coins: 20 };
    let receipt = create_p2p_receipt(&sender_key, &receiver_key, realm_id, delta);
    let envelope = ReceiptEnvelope {
        realm_id,
        receipt,
        timestamp: 12345,
    };

    let result = node.handle_p2p_receipt(envelope, PeerId::random()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_p2p_receipt_wrong_realm() {
    let realm_id = create_test_realm_id();
    let mut wrong_realm = realm_id;
    wrong_realm[0] = 99;

    let (gov_key, gov_pub) = create_test_keypair();
    let (sender_key, _) = create_test_keypair();
    let (receiver_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);

    let mut node = P2PNode::new(config).unwrap();

    let gov_log = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(gov_log, PeerId::random()).await.unwrap();

    let delta = ResourceDelta { wheat: 10, wood: 5, coins: 20 };
    let receipt = create_p2p_receipt(&sender_key, &receiver_key, realm_id, delta);
    let envelope = ReceiptEnvelope {
        realm_id: wrong_realm,
        receipt,
        timestamp: 12345,
    };

    let result = node.handle_p2p_receipt(envelope, PeerId::random()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("realm"));
}

#[tokio::test]
async fn test_p2p_receipt_invalid_signature() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();
    let (sender_key, _) = create_test_keypair();
    let (receiver_key, _) = create_test_keypair();
    let (malicious_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);

    let mut node = P2PNode::new(config).unwrap();
    let gov_log = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(gov_log, PeerId::random()).await.unwrap();

    let delta = ResourceDelta { wheat: 10, wood: 5, coins: 20 };
    let mut receipt = create_p2p_receipt(&sender_key, &receiver_key, realm_id, delta);
    
    // Tamper with sender signature
    receipt.sender_signature = malicious_key.sign(&receipt.tx_hash).to_bytes();

    let envelope = ReceiptEnvelope { realm_id, receipt, timestamp: 12345 };
    let result = node.handle_p2p_receipt(envelope, PeerId::random()).await;
    assert!(result.is_err());
}

// ========================================================================
// CHAT MESSAGE TESTS
// ========================================================================

#[tokio::test]
async fn test_chat_message_valid() {
    let realm_id = create_test_realm_id();
    let (user_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    let mut node = P2PNode::new(config).unwrap();

    let chat = create_chat_message(&user_key, "Hello Realm".to_string(), 1);
    let envelope = ChatMessageEnvelope {
        realm_id,
        message: chat,
        timestamp: 12345,
    };

    let result = node.handle_chat_message(envelope, PeerId::random()).await;
    assert!(result.is_ok());
    assert_eq!(node.get_chat_messages().len(), 1);
    assert_eq!(node.get_chat_messages()[0].content, "Hello Realm");
}

#[tokio::test]
async fn test_chat_message_deduplication() {
    let realm_id = create_test_realm_id();
    let (user_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    let mut node = P2PNode::new(config).unwrap();

    let chat = create_chat_message(&user_key, "Duplicate".to_string(), 1);
    let envelope = ChatMessageEnvelope {
        realm_id,
        message: chat,
        timestamp: 12345,
    };

    node.handle_chat_message(envelope.clone(), PeerId::random()).await.unwrap();
    node.handle_chat_message(envelope, PeerId::random()).await.unwrap();

    assert_eq!(node.get_chat_messages().len(), 1, "Duplicate chat should not be stored twice");
}

// ========================================================================
// STARTUP SYNC & DIRECT REQUEST TESTS
// ========================================================================

#[tokio::test]
async fn test_process_get_latest_gov_log_request() {
    let realm_id = create_test_realm_id();
    let (gov_key, gov_pub) = create_test_keypair();
    
    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    config.government_public_key = Some(gov_pub);
    let mut node = P2PNode::new(config).unwrap();

    let log = create_government_log(0, realm_id, &gov_key);
    node.handle_government_log(log.clone(), PeerId::random()).await.unwrap();

    let req = DirectRequest::GetLatestGovLog { realm_id };
    let response = node.process_direct_request(req, PeerId::random()).await.unwrap();

    match response {
        DirectResponse::GovernmentLog { log: Some(returned_log) } => {
            assert_eq!(returned_log.version_number, 0);
            assert_eq!(returned_log.log_hash, log.log_hash);
        }
        _ => panic!("Expected GovernmentLog response"),
    }
}

#[test]
fn test_economic_actions_blocked_until_gov_log_received() {
    let realm_id = create_test_realm_id();
    let (sender_key, _) = create_test_keypair();
    let (receiver_key, _) = create_test_keypair();

    let mut config = NetworkConfig::default();
    config.realm_id = realm_id;
    let mut node = P2PNode::new(config).unwrap();

    // Node has NO gov log yet
    assert!(!node.can_process_economic_action());

    let delta = ResourceDelta { wheat: 1, wood: 1, coins: 1 };
    let receipt = create_p2p_receipt(&sender_key, &receiver_key, realm_id, delta);
    let envelope = ReceiptEnvelope { realm_id, receipt, timestamp: 12345 };

    // This would be filtered out in the main event loop match arm:
    // if self.can_process_economic_action() { ... }
    let should_process = node.can_process_economic_action();
    assert!(!should_process, "Economic actions must be blocked without a gov log");
}
}