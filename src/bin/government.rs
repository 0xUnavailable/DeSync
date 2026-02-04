use anyhow::Result;
use clap::{Parser, Subcommand};
use ed25519_dalek::{SigningKey, VerifyingKey, Signer};
use std::fs;
use std::path::Path;

use realm::gov::log_manager::GovernmentNode;
use realm::gov::receipt_signing::sign_tax_receipt;
use realm::types::{HASH, RealmObject, TaxReceiptEnvelope, ActionRule, Resources};
use realm::storage::ObjectStore;
use realm::network::libp2p::{P2PNode, NetworkConfig};
use libp2p::Multiaddr;
use futures::StreamExt;

#[derive(Parser)]
#[command(name = "government")]
#[command(about = "Realm Government Node", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new government (creates keys and genesis log)
    Init {
        /// Path to store government keys
        #[arg(long, default_value = "gov_keys")]
        keys_dir: String,
    },
    
    /// Start the government node
    Run {
        /// Path to government keys
        #[arg(long, default_value = "gov_keys")]
        keys_dir: String,
        
        /// Address to listen on
        #[arg(long, default_value = "/ip4/0.0.0.0/tcp/4001")]
        listen: String,
    },
    
    /// Process pending obligations and sign receipts
    ProcessObligations {
        /// Path to government keys
        #[arg(long, default_value = "gov_keys")]
        keys_dir: String,
    },
    
    /// Show government info
    Info {
        /// Path to government keys
        #[arg(long, default_value = "gov_keys")]
        keys_dir: String,
    },
}

fn main() -> Result<()> {
    env_logger::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Init { keys_dir } => {
            init_government(&keys_dir)?;
        }
        
        Commands::Run { keys_dir, listen } => {
            tokio::runtime::Runtime::new()?.block_on(async {
                run_government(&keys_dir, &listen).await
            })?;
        }
        
        Commands::ProcessObligations { keys_dir } => {
            process_obligations(&keys_dir)?;
        }
        
        Commands::Info { keys_dir } => {
            show_info(&keys_dir)?;
        }
    }
    
    Ok(())
}

fn init_government(keys_dir: &str) -> Result<()> {
    println!("Initializing government node...");
    
    // Create keys directory
    fs::create_dir_all(keys_dir)?;
    
    // Generate new signing key
    let signing_key = SigningKey::generate(&mut rand::thread_rng());
    let verifying_key = signing_key.verifying_key();
    
    // Save signing key (KEEP SECURE!)
    fs::write(
        Path::new(keys_dir).join("signing_key.bin"),
        signing_key.to_bytes(),
    )?;
    println!("[OK] Generated signing key (saved to {}/signing_key.bin)", keys_dir);
    println!("  WARNING: Keep this file secure!");
    
    // Save verifying key (public)
    let verifying_key_hex = hex::encode(verifying_key.to_bytes());
    fs::write(
        Path::new(keys_dir).join("verifying_key.txt"),
        &verifying_key_hex,
    )?;
    println!("[OK] Public key: {}", verifying_key_hex);
    
    // Generate realm ID (deterministic from public key for reproducibility)
    let realm_id: HASH = *blake3::hash(&verifying_key.to_bytes()).as_bytes();
    let realm_id_hex = hex::encode(realm_id);
    fs::write(
        Path::new(keys_dir).join("realm_id.txt"),
        &realm_id_hex,
    )?;
    println!("[OK] Realm ID: {}", realm_id_hex);
    
    // Generate genesis government log
    let genesis_log = GovernmentNode::generate_genesis_government_log(
        realm_id,
        &signing_key,
    )?;
    
    println!("\n[OK] Genesis log created:");
    println!("  Version: {}", genesis_log.version_number);
    println!("  Rules:");
    for rule in &genesis_log.state_rules {
        println!("    - {}: {} coins (land={}, labor={}, capital={})",
            rule.task_name,
            rule.coin_multiplier,
            rule.required_resources.land,
            rule.required_resources.labor,
            rule.required_resources.capital,
        );
    }
    
    // Save genesis log as RealmObject
    let genesis_object = RealmObject::GovernmentRuleLog(genesis_log.clone());
    let genesis_bytes = serde_json::to_vec(&genesis_object)?;
    let genesis_hash = blake3::hash(&genesis_bytes);
    
    fs::write(
        Path::new(keys_dir).join("genesis.json"),
        serde_json::to_string_pretty(&genesis_object)?,
    )?;
    println!("\n[OK] Genesis saved to {}/genesis.json", keys_dir);
    println!("  Hash: {}", hex::encode(genesis_hash.as_bytes()));
    
    // Store in object store
    let object_store = ObjectStore::new("./objects")?;
    object_store.store_object(genesis_hash.as_bytes(), &genesis_bytes)?;
    println!("[OK] Genesis loaded into object store");
    
    // Save government state
    let mut gov_node = GovernmentNode::new(signing_key);
    gov_node.latest_log = Some(genesis_log);
    
    let gov_state = serde_json::to_string_pretty(&gov_node)?;
    fs::write(
        Path::new(keys_dir).join("state.json"),
        gov_state,
    )?;
    
    println!("\n=== SETUP COMPLETE ===");
    println!("\nShare with users:");
    println!("  Realm ID:     {}", realm_id_hex);
    println!("  Gov Key:      {}", verifying_key_hex);
    println!("\nTo start the government node:");
    println!("  government run --keys-dir {}", keys_dir);
    
    Ok(())
}

async fn run_government(keys_dir: &str, listen_addr: &str) -> Result<()> {
    println!("Starting government node...");
    
    // Load keys
    let signing_key_bytes = fs::read(Path::new(keys_dir).join("signing_key.bin"))?;
    let signing_key = SigningKey::from_bytes(&signing_key_bytes.try_into()
        .map_err(|_| anyhow::anyhow!("Invalid signing key"))?);
    
    let realm_id_hex = fs::read_to_string(Path::new(keys_dir).join("realm_id.txt"))?;
    let realm_id_bytes = hex::decode(realm_id_hex.trim())?;
    let mut realm_id = [0u8; 32];
    realm_id.copy_from_slice(&realm_id_bytes);
    
    println!("[OK] Loaded government keys");
    println!("  Realm ID: {}", hex::encode(realm_id));
    println!("  Public key: {}", hex::encode(signing_key.verifying_key().to_bytes()));
    
    // FIX #1: Load genesis log from object store and initialize state
    let object_store = ObjectStore::new("./objects")?;
    
    // Read genesis hash from genesis.json
    let genesis_str = fs::read_to_string(Path::new(keys_dir).join("genesis.json"))?;
    let genesis_object: RealmObject = serde_json::from_str(&genesis_str)?;
    let genesis_bytes = serde_json::to_vec(&genesis_object)?;
    let genesis_hash = blake3::hash(&genesis_bytes);
    
    println!("[INIT] Loading genesis log from object store...");
    println!("  Genesis hash: {}", hex::encode(genesis_hash.as_bytes()));
    
    // Verify genesis exists in object store
    if !object_store.has_object(genesis_hash.as_bytes()) {
        return Err(anyhow::anyhow!(
            "Genesis log not found in object store. Run 'government init' first."
        ));
    }
    
    println!("[OK] Genesis log verified in object store");
    
    // Create libp2p keypair from ed25519 key
    let libp2p_keypair = libp2p::identity::Keypair::ed25519_from_bytes(signing_key.to_bytes())?;
    
    let config = NetworkConfig {
        realm_id,
        keypair: libp2p_keypair,
        government_public_key: Some(signing_key.verifying_key()),
        is_government: true,
        node_role: realm::types::NodeRole::Government,
    };
    
    let mut node = P2PNode::new(config)?;
    
    // FIX #1 (continued): Apply genesis log to node state
    if let RealmObject::GovernmentRuleLog(genesis_log) = genesis_object {
        use realm::network::state_apply::apply_object_to_state;
        
        println!("[INIT] Applying genesis log to state...");
        println!("  Version: {}", genesis_log.version_number);
        println!("  Hash: {}", hex::encode(genesis_log.log_hash));
        
        // FIX #4b: Try to recover state first, fallback to genesis if no recovery
        println!("[INIT] Attempting to recover previous state...");
        match realm::user::recovery::recover_state() {
            Ok(recovered_state) if recovered_state.latest_government_log.is_some() => {
                println!("[OK] Recovered previous state:");
                println!("  Gov log version: {}", recovered_state.latest_gov_log_version);
                println!("  Using recovered state instead of genesis");
                node.state = recovered_state;
            }
            Ok(_) | Err(_) => {
                println!("[INFO] No previous state found, initializing with genesis");
                apply_object_to_state(
                    RealmObject::GovernmentRuleLog(genesis_log.clone()),
                    &mut node.state
                )?;
                
                println!("[OK] Genesis log applied to state");
                println!("  Current state version: {}", node.state.latest_gov_log_version);
                
                // Index the genesis log
                node.log_index.index_log(genesis_log.version_number, *genesis_hash.as_bytes());
            }
        }
    } else {
        return Err(anyhow::anyhow!("Genesis object is not a GovernmentRuleLog"));
    }
    
    // Listen
    let addr: Multiaddr = listen_addr.parse()?;
    node.listen(addr.clone())?;
    
    println!("\n[OK] Government node running");
    println!("  Listening on: {}", listen_addr);
    println!("\nWaiting for connections...\n");
    
    // Event loop
    loop {
        tokio::select! {
            event = node.swarm.next() => {
                if let Some(event) = event {
                    if let Err(e) = handle_government_event(&mut node, event).await {
                        eprintln!("Error handling event: {}", e);
                    }
                }
            }
        }
    }
}

async fn handle_government_event(
    node: &mut P2PNode,
    event: libp2p::swarm::SwarmEvent<realm::network::libp2p::RealmBehaviourEvent>,
) -> Result<()> {
    use libp2p::swarm::SwarmEvent;
    use libp2p::request_response::Event as RREvent;
    use realm::network::protocol::{RealmRequest, RealmResponse};
    
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            let peer_id = node.swarm.local_peer_id();
            let full_addr = format!("{}/p2p/{}", address, peer_id);
            println!("[NET] Listening on {}", full_addr);
        }
        
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            println!("[NET] Peer connected: {}", peer_id);
            // Send Hello message
            node.send_hello(peer_id);
        }
        
        SwarmEvent::ConnectionClosed { peer_id, .. } => {
            println!("[NET] Peer disconnected: {}", peer_id);
        }
        
        SwarmEvent::Behaviour(realm::network::libp2p::RealmBehaviourEvent::RequestResponse(rr_event)) => {
            match rr_event {
                RREvent::Message { peer, message } => {
                    match message {
                        libp2p::request_response::Message::Request { request, channel, .. } => {
                            let response = match request {
                                RealmRequest::Hello(hello) => {
                                    if let Err(e) = node.handle_hello(peer, hello.clone()) {
                                        eprintln!("[HANDSHAKE] Error handling hello: {}", e);
                                    }
                                    RealmResponse::HelloAck {
                                        role: realm::types::NodeRole::Government,
                                        realm_id: node.config.realm_id,
                                    }
                                }
                                
                                RealmRequest::GetHead => {
                                    let head = node.get_head();
                                    println!("[SYNC] Sending head to {}: version={}", peer, head.gov_log_version);
                                    RealmResponse::Head(head)
                                }
                                
                                RealmRequest::GetObject(hash) => {
                                    match node.object_store.get_object(&hash)? {
                                        Some(bytes) => {
                                            println!("[SYNC] Sending object to {}: {} bytes", peer, bytes.len());
                                            RealmResponse::Object(bytes)
                                        }
                                        None => {
                                            println!("[WARN]  Object not found: {}", hex::encode(hash));
                                            RealmResponse::Error("Object not found".to_string())
                                        }
                                    }
                                }
                                
                                RealmRequest::LogRequest { start_version } => {
                                    // Government serves log requests from users
                                    use realm::network::sync::build_logs_chunk;
                                    match build_logs_chunk(start_version, 100, &node.object_store, &node.log_index) {
                                        Ok(logs) => {
                                            println!("[SYNC] Sending {} logs to {} starting from version {}", 
                                                logs.len(), peer, start_version);
                                            RealmResponse::LogsChunk(logs)
                                        }
                                        Err(e) => {
                                            eprintln!("[ERROR] Failed to build logs chunk: {}", e);
                                            RealmResponse::Error(format!("Failed to build logs: {}", e))
                                        }
                                    }
                                }
                                
                                RealmRequest::PushObject(bytes) => {
                                    println!("[PUSH] Received pushed object from {}", peer);
                                    
                                    // Verify it's not a government log (users can't create those)
                                    if let Ok(obj) = serde_json::from_slice::<RealmObject>(&bytes) {
                                        match obj {
                                            RealmObject::GovernmentRuleLog(_) => {
                                                println!("[SECURITY] Rejected government log from user {}", peer);
                                                RealmResponse::Error("Users cannot create government logs".to_string())
                                            }
                                            _ => {
                                                match node.handle_received_object(bytes) {
                                                    Ok(_) => {
                                                        println!("[PUSH] Object accepted and stored");
                                                        RealmResponse::PushAck
                                                    }
                                                    Err(e) => {
                                                        eprintln!("[ERROR] Failed to process pushed object: {}", e);
                                                        RealmResponse::Error(format!("Invalid object: {}", e))
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        eprintln!("[ERROR] Failed to parse pushed object");
                                        RealmResponse::Error("Failed to parse object".to_string())
                                    }
                                }
                            };
                            
                            let _ = node.swarm.behaviour_mut()
                                .request_response
                                .send_response(channel, response);
                        }
                        
                        libp2p::request_response::Message::Response { response, .. } => {
                            match response {
                                RealmResponse::HelloAck { role, realm_id } => {
                                    println!("[HANDSHAKE] Received HelloAck from {} as {:?}, realm {:?}", 
                                        peer, role, hex::encode(&realm_id[..8]));
                                    // Initiate sync after handshake
                                    if let Err(e) = node.sync_with_peer(peer) {
                                        eprintln!("[SYNC] Error starting sync: {}", e);
                                    }
                                }
                                
                                RealmResponse::Head(peer_head) => {
                                    println!("[SYNC] Received head from {}: version={}", peer, peer_head.gov_log_version);
                                    node.handle_peer_head(peer, peer_head)?;
                                }
                                
                                RealmResponse::Object(bytes) => {
                                    // Government nodes should not receive GovernmentRuleLog objects from peers
                                    // But they can accept TaxObligationEnvelope and other objects
                                    if let Ok(obj) = serde_json::from_slice::<RealmObject>(&bytes) {
                                        match obj {
                                            RealmObject::GovernmentRuleLog(_) => {
                                                println!("[WARN] Government received GovernmentRuleLog from {} - rejecting", peer);
                                            }
                                            _ => {
                                                println!("[SYNC] Received object from {}: {} bytes", peer, bytes.len());
                                                node.handle_received_object(bytes)?;
                                            }
                                        }
                                    }
                                }
                                
                                RealmResponse::LogsChunk(_) => {
                                    // Government should never receive logs chunks
                                    println!("[WARN] Government received LogsChunk from {} - unexpected", peer);
                                }
                                
                                RealmResponse::PushAck => {
                                    println!("[PUSH] Object push acknowledged by {}", peer);
                                }
                                
                                RealmResponse::Error(err) => {
                                    eprintln!("[NET] Error from peer {}: {}", peer, err);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        
        _ => {}
    }
    
    Ok(())
}

fn process_obligations(keys_dir: &str) -> Result<()> {
    println!("Processing obligations...\n");
    
    // Load government key
    let signing_key_bytes = fs::read(Path::new(keys_dir).join("signing_key.bin"))?;
    let signing_key = SigningKey::from_bytes(&signing_key_bytes.try_into()
        .map_err(|_| anyhow::anyhow!("Invalid signing key"))?);
    
    // Load government state
    let state_str = fs::read_to_string(Path::new(keys_dir).join("state.json"))?;
    let mut gov_node: GovernmentNode = serde_json::from_str(&state_str)?;
    
    // Load object store
    let object_store = ObjectStore::new("./objects")?;
    
    // Find all obligations
    let all_hashes = object_store.list_all()?;
    let mut processed = 0;
    
    for hash in &all_hashes {
        if let Some(bytes) = object_store.get_object(hash)? {
            if let Ok(obj) = serde_json::from_slice::<RealmObject>(&bytes) {
                if let RealmObject::TaxObligationEnvelope(env) = obj {
                    let obligation_hash = env.obligation.obligation_hash;
                    
                    // Check if already processed
                    let receipt_exists = all_hashes.iter().any(|h| {
                        if let Some(b) = object_store.get_object(h).ok().flatten() {
                            if let Ok(RealmObject::TaxReceiptEnvelope(r)) = 
                                serde_json::from_slice::<RealmObject>(&b) {
                                return r.receipt.obligation_hash == obligation_hash;
                            }
                        }
                        false
                    });
                    
                    if receipt_exists {
                        continue;
                    }
                    
                    println!("[TASK] Processing obligation:");
                    println!("   User: {}", hex::encode(env.obligation.user_id.to_bytes()));
                    println!("   Amount: {} coins", env.obligation.coin_amount);
                    
                    // Sign receipt
                    let receipt = sign_tax_receipt(
                        &gov_node,
                        &env.obligation,
                        None,
                        1,
                    )?;
                    
                    // Create envelope
                    let receipt_bytes = serde_json::to_vec(&receipt)?;
                    let receipt_sig = signing_key.sign(&receipt_bytes);
                    
                    let receipt_envelope = TaxReceiptEnvelope {
                        receipt: receipt.clone(),
                        signature: receipt_sig.to_bytes(),
                    };
                    
                    // Store as object
                    let receipt_obj = RealmObject::TaxReceiptEnvelope(receipt_envelope);
                    let receipt_obj_bytes = serde_json::to_vec(&receipt_obj)?;
                    let receipt_hash = blake3::hash(&receipt_obj_bytes);
                    
                    object_store.store_object(receipt_hash.as_bytes(), &receipt_obj_bytes)?;
                    
                    println!("   [OK] Receipt signed: {}", hex::encode(receipt_hash.as_bytes()));
                    println!("   [OK] Coins transferred: {}\n", receipt.coins_transferred);
                    
                    processed += 1;
                }
            }
        }
    }
    
    println!("=== SUMMARY ===");
    println!("Processed {} new obligations", processed);
    
    Ok(())
}

fn show_info(keys_dir: &str) -> Result<()> {
    let verifying_key_hex = fs::read_to_string(Path::new(keys_dir).join("verifying_key.txt"))?;
    let realm_id_hex = fs::read_to_string(Path::new(keys_dir).join("realm_id.txt"))?;
    
    println!("=== GOVERNMENT NODE INFO ===\n");
    println!("Realm ID:          {}", realm_id_hex.trim());
    println!("Public Key:        {}", verifying_key_hex.trim());
    println!("Keys Directory:    {}", keys_dir);
    
    // Load and show genesis
    if let Ok(genesis_str) = fs::read_to_string(Path::new(keys_dir).join("genesis.json")) {
        if let Ok(genesis_obj) = serde_json::from_str::<RealmObject>(&genesis_str) {
            if let RealmObject::GovernmentRuleLog(log) = genesis_obj {
                println!("\nGenesis Log:");
                println!("  Version: {}", log.version_number);
                println!("  Rules:");
                for rule in &log.state_rules {
                    println!("    - {}: {} coins", rule.task_name, rule.coin_multiplier);
                }
            }
        }
    }
    
    // Check object store
    if let Ok(store) = ObjectStore::new("./objects") {
        if let Ok(objects) = store.list_all() {
            println!("\nObject Store:");
            println!("  Total objects: {}", objects.len());
        }
    }
    
    Ok(())
}