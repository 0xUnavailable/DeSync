
//#[tokio::main]
fn main()  {
//     env_logger::init();
    
//     let cli = Cli::parse();
    
//     match cli.command {
//         Commands::Init { realm_id, gov_key } => {
//             println!("Initializing node...");
            
//             // Parse realm_id
//             let realm_id_bytes = hex::decode(&realm_id)?;
//             if realm_id_bytes.len() != 32 {
//                 anyhow::bail!("Realm ID must be 32 bytes");
//             }
//             let mut realm_id_hash = [0u8; 32];
//             realm_id_hash.copy_from_slice(&realm_id_bytes);
            
//             // Generate keypair
//             let signing_key = SigningKey::generate(&mut rand::thread_rng());
//             let keypair = libp2p::identity::Keypair::ed25519_from_bytes(signing_key.to_bytes())?;
            
//             // Parse government key if provided
//             let government_key = if let Some(gov_hex) = gov_key {
//                 let gov_bytes = hex::decode(&gov_hex)?;
//                 Some(ed25519_dalek::VerifyingKey::from_bytes(
//                     gov_bytes.as_slice().try_into()?
//                 )?)
//             } else {
//                 None
//             };
            
//             let config = NetworkConfig {
//                 realm_id: realm_id_hash,
//                 keypair,
//                 government_public_key: government_key,
//                 is_government: false,
//             };
            
//             let _node = P2PNode::new(config)?;
//             println!("Node initialized successfully");
//         }
        
//         Commands::Listen { addr } => {
//             println!("Starting node and listening on {}", addr);
//             run_node(Some(addr.parse()?), None).await?;
//         }
        
//         Commands::Dial { addr } => {
//             println!("Dialing peer at {}", addr);
//             run_node(None, Some(addr.parse()?)).await?;
//         }
        
//         Commands::Sync => {
//             println!("Triggering manual sync...");
//             run_node(None, None).await?;
//         }
        
//         Commands::Balance => {
//             println!("Balance: 0 (state recovery not implemented in this command)");
//         }
        
//         Commands::StartTask { name } => {
//             println!("Starting task: {}", name);
//         }
        
//         Commands::HarvestTask { name } => {
//             println!("Harvesting task: {}", name);
//         }
//     }
    
//     Ok(())
// }

// async fn run_node(listen_addr: Option<Multiaddr>, dial_addr: Option<Multiaddr>) -> Result<()> {
//     // Create default config for demo
//     let signing_key = SigningKey::generate(&mut rand::thread_rng());
//     let keypair = libp2p::identity::Keypair::ed25519_from_bytes(signing_key.to_bytes())?;
    
//     let config = NetworkConfig {
//         realm_id: [0u8; 32],
//         keypair,
//         government_public_key: None,
//         is_government: false,
//     };
    
//     let mut node = P2PNode::new(config)?;
    
//     if let Some(addr) = listen_addr {
//         node.listen(addr)?;
//     }
    
//     if let Some(addr) = dial_addr {
//         node.dial(addr)?;
//     }
    
//     // Event loop with sync-on-connect trigger
//     loop {
//         tokio::select! {
//             event = node.swarm.next() => {
//                 if let Some(event) = event {
//                     if let Err(e) = handle_swarm_event(&mut node, event).await {
//                         eprintln!("Error handling event: {}", e);
//                     }
//                 }
//             }
//         }
//     }
// }

// async fn handle_swarm_event(
//     node: &mut P2PNode,
//     event: libp2p::swarm::SwarmEvent<RealmBehaviourEvent>,
// ) -> Result<()> {
//     use libp2p::swarm::SwarmEvent;
    
//     match event {
//         SwarmEvent::ConnectionEstablished { peer_id, .. } => {
//             println!("Connected to peer: {}", peer_id);
            
//             // SYNC TRIGGER: On connection, immediately request peer head
//             node.sync_with_peer(peer_id)?;
//         }
        
//         SwarmEvent::Behaviour(behaviour_event) => {
//             handle_behaviour_event(node, behaviour_event).await?;
//         }
        
//         _ => {}
//     }
    
//     Ok(())
// }

// async fn handle_behaviour_event(
//     node: &mut P2PNode,
//     event: RealmBehaviourEvent,
// ) -> Result<()> {
//     use libp2p::request_response::Event as RREvent;
    
//     // Extract the request_response event from the generated enum
//     let rr_event = match event {
//         RealmBehaviourEvent::RequestResponse(inner) => inner,
//     };
    
//     match rr_event {
//         RREvent::Message { peer, message } => {
//             match message {
//                 libp2p::request_response::Message::Request { request, channel, .. } => {
//                     handle_request(node, peer, request, channel).await?;
//                 }
//                 libp2p::request_response::Message::Response { response, .. } => {
//                     handle_response(node, peer, response).await?;
//                 }
//             }
//         }
//         _ => {}
//     }
    
//     Ok(())
// }

// async fn handle_request(
//     node: &mut P2PNode,
//     _peer: libp2p::PeerId,
//     request: RealmRequest,
//     channel: libp2p::request_response::ResponseChannel<RealmResponse>,
// ) -> Result<()> {
//     let response = match request {
//         RealmRequest::GetHead => {
//             let head = node.get_head();
//             RealmResponse::Head(head)
//         }
        
//         RealmRequest::GetObject(hash) => {
//             match node.object_store.get_object(&hash)? {
//                 Some(bytes) => RealmResponse::Object(bytes),
//                 None => RealmResponse::Error("Object not found".to_string()),
//             }
//         }
//     };
    
//     let _ = node.swarm.behaviour_mut()
//         .request_response
//         .send_response(channel, response);
    
//     Ok(())
// }

// async fn handle_response(
//     node: &mut P2PNode,
//     peer: libp2p::PeerId,
//     response: RealmResponse,
// ) -> Result<()> {
//     match response {
//         RealmResponse::Head(peer_head) => {
//             println!("Received head from {}: version={}", peer, peer_head.gov_log_version);
//             node.handle_peer_head(peer, peer_head)?;
//         }
        
//         RealmResponse::Object(bytes) => {
//             println!("Received object from {}: {} bytes", peer, bytes.len());
//             node.handle_received_object(bytes)?;
//         }
        
//         RealmResponse::Error(err) => {
//             eprintln!("Error from peer {}: {}", peer, err);
//         }
//     }
    
//     Ok(())
// }

}