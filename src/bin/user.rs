use anyhow::Result;
use clap::{Parser, Subcommand};
use ed25519_dalek::{SigningKey, VerifyingKey, Signer};
use std::fs;
use std::path::Path;

use realm::types::{
    HASH, RealmObject, TaxObligation, TaxObligationEnvelope,
    GovernmentLogRef, ActionRule, P2PTransferReceipt, ReceiptEnvelope,
};
use realm::storage::ObjectStore;
use realm::network::libp2p::{P2PNode, NetworkConfig};
use realm::user::state_manager::UserStateManager;
use libp2p::Multiaddr;
use futures::StreamExt;

#[derive(Parser)]
#[command(name = "user")]
#[command(about = "Realm User Node", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new user node
    Init {
        /// Realm ID (from government)
        #[arg(long)]
        realm_id: String,
        
        /// Government public key (from government)
        #[arg(long)]
        gov_key: String,
        
        /// Path to store user data
        #[arg(long, default_value = "user_data")]
        data_dir: String,
    },
    
    /// Start the user node
    Run {
        /// Path to user data
        #[arg(long, default_value = "user_data")]
        data_dir: String,
        
        /// Address to listen on
        #[arg(long, default_value = "/ip4/0.0.0.0/tcp/0")]
        listen: String,
        
        /// Optional peer to connect to on startup
        #[arg(long)]
        dial: Option<String>,
    },
    
    /// Connect to a peer
    Connect {
        /// Peer multiaddr
        peer: String,
        
        /// Path to user data
        #[arg(long, default_value = "user_data")]
        data_dir: String,
    },
    
    /// Start a task
    StartTask {
        /// Task name (e.g., "farming", "mining")
        name: String,
        
        /// Path to user data
        #[arg(long, default_value = "user_data")]
        data_dir: String,
    },
    
    /// Harvest a completed task
    HarvestTask {
        /// Task name
        name: String,
        
        /// Path to user data
        #[arg(long, default_value = "user_data")]
        data_dir: String,
    },
    
    /// Create obligation and submit to government
    SubmitObligation {
        /// Amount in coins
        amount: u64,
        
        /// Path to user data
        #[arg(long, default_value = "user_data")]
        data_dir: String,
    },
    
    /// Send coins to another user
    SendCoins {
        /// Recipient public key (hex)
        #[arg(long)]
        to: String,
        
        /// Amount to send
        amount: i64,
        
        /// Path to user data
        #[arg(long, default_value = "user_data")]
        data_dir: String,
    },
    
    /// Show status
    Status {
        /// Path to user data
        #[arg(long, default_value = "user_data")]
        data_dir: String,
    },
}

fn main() -> Result<()> {
    env_logger::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Init { realm_id, gov_key, data_dir } => {
            init_user(&realm_id, &gov_key, &data_dir)?;
        }
        
        Commands::Run { data_dir, listen, dial } => {
            tokio::runtime::Runtime::new()?.block_on(async {
                run_user(&data_dir, &listen, dial.as_deref()).await
            })?;
        }
        
        Commands::Connect { peer, data_dir: _ } => {
            println!("Connect to peer: {}", peer);
            println!("This should be run with the 'run' command in a future update");
            println!("Use: user run --data-dir <dir> --dial {}", peer);
        }
        
        Commands::StartTask { name, data_dir } => {
            start_task(&data_dir, &name)?;
        }
        
        Commands::HarvestTask { name, data_dir } => {
            harvest_task(&data_dir, &name)?;
        }
        
        Commands::SubmitObligation { amount, data_dir } => {
            submit_obligation(&data_dir, amount)?;
        }
        
        Commands::SendCoins { to, amount, data_dir } => {
            send_coins(&data_dir, &to, amount)?;
        }
        
        Commands::Status { data_dir } => {
            show_status(&data_dir)?;
        }
    }
    
    Ok(())
}

fn init_user(realm_id_hex: &str, gov_key_hex: &str, data_dir: &str) -> Result<()> {
    println!("Initializing user node...\n");
    
    // Create data directory
    fs::create_dir_all(data_dir)?;
    
    // Generate user signing key
    let signing_key = SigningKey::generate(&mut rand::thread_rng());
    let verifying_key = signing_key.verifying_key();
    
    // Save signing key
    fs::write(
        Path::new(data_dir).join("signing_key.bin"),
        signing_key.to_bytes(),
    )?;
    println!("[OK] Generated user key");
    println!("  Public key: {}", hex::encode(verifying_key.to_bytes()));
    
    // Save config
    fs::write(
        Path::new(data_dir).join("realm_id.txt"),
        realm_id_hex,
    )?;
    
    fs::write(
        Path::new(data_dir).join("gov_key.txt"),
        gov_key_hex,
    )?;
    
    println!("[OK] Saved realm config");
    println!("  Realm ID: {}", realm_id_hex);
    println!("  Gov key: {}", gov_key_hex);
    
    // Initialize user state manager
    let user_state = UserStateManager::new();
    fs::write(
        Path::new(data_dir).join("user_state.json"),
        serde_json::to_string_pretty(&user_state)?,
    )?;
    
    println!("\n[OK] User node initialized");
    println!("\nTo start:");
    println!("  user run --data-dir {}", data_dir);
    println!("\nTo connect to government:");
    println!("  user run --data-dir {} --dial <government-multiaddr>", data_dir);
    
    Ok(())
}

fn broadcast_pending_objects(node: &mut realm::network::libp2p::P2PNode) -> Result<()> {
    let pending_dir = Path::new("./pending_push");
    
    if !pending_dir.exists() {
        return Ok(());
    }
    
    println!("\n[PUSH] Checking for pending objects to broadcast...");
    
    let mut broadcast_count = 0;
    for entry in fs::read_dir(pending_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        
        println!("[PUSH] Found pending object: {:?}", entry.file_name());
        
        match fs::read(&path) {
            Ok(bytes) => {
                // Broadcast to all connected peers
                node.broadcast_object(bytes);
                broadcast_count += 1;
                
                // Delete after successful broadcast
                if let Err(e) = fs::remove_file(&path) {
                    eprintln!("[WARN] Could not delete {}: {}", path.display(), e);
                }
            }
            Err(e) => {
                eprintln!("[ERROR] Failed to read pending object: {}", e);
            }
        }
    }
    
    if broadcast_count > 0 {
        println!("[PUSH] Broadcast {} pending object(s)", broadcast_count);
    }
    
    Ok(())
}

async fn run_user(data_dir: &str, listen_addr: &str, dial_addr: Option<&str>) -> Result<()> {
    println!("Starting user node...\n");
    
    // Load keys and config
    let signing_key_bytes = fs::read(Path::new(data_dir).join("signing_key.bin"))?;
    let signing_key = SigningKey::from_bytes(&signing_key_bytes.try_into()
        .map_err(|_| anyhow::anyhow!("Invalid signing key"))?);
    
    let realm_id_hex = fs::read_to_string(Path::new(data_dir).join("realm_id.txt"))?;
    let realm_id_bytes = hex::decode(realm_id_hex.trim())?;
    let mut realm_id = [0u8; 32];
    realm_id.copy_from_slice(&realm_id_bytes);
    
    let gov_key_hex = fs::read_to_string(Path::new(data_dir).join("gov_key.txt"))?;
    let gov_key_bytes = hex::decode(gov_key_hex.trim())?;
    let gov_key = VerifyingKey::from_bytes(gov_key_bytes.as_slice().try_into()?)?;
    
    println!("[OK] Loaded user keys");
    println!("  Realm ID: {}", hex::encode(realm_id));
    println!("  Public key: {}", hex::encode(signing_key.verifying_key().to_bytes()));
    
    // Create P2P node
    let libp2p_keypair = libp2p::identity::Keypair::ed25519_from_bytes(signing_key.to_bytes())?;
    
    let config = NetworkConfig {
        realm_id,
        keypair: libp2p_keypair,
        government_public_key: Some(gov_key),
        is_government: false,
        node_role: realm::types::NodeRole::User,
    };
    
    let mut node = P2PNode::new(config)?;
    
    // FIX #4a: Recover state from persistent storage (loads previously synced government logs)
    println!("[INIT] Recovering state from persistent storage...");
    match realm::user::recovery::recover_state() {
        Ok(recovered_state) => {
            if recovered_state.latest_government_log.is_some() {
                println!("[OK] Recovered state:");
                println!("  Gov log version: {}", recovered_state.latest_gov_log_version);
                println!("  Tax receipts: {}", recovered_state.tax_receipts.len());
                println!("  Coins: {}", recovered_state.coins);
                
                // Apply recovered state
                node.state = recovered_state;
            } else {
                println!("[INFO] No previous state found, starting fresh");
            }
        }
        Err(e) => {
            println!("[WARN] Could not recover state: {} - starting fresh", e);
        }
    }
    
    // Listen
    let addr: Multiaddr = listen_addr.parse()?;
    node.listen(addr.clone())?;
    
    println!("\n[OK] User node running");
    println!("  Listening on: {}", listen_addr);
    
    // Dial peer if specified
    if let Some(dial) = dial_addr {
        println!("\n[DIAL] Connecting to peer: {}", dial);
        let peer_addr: Multiaddr = dial.parse()?;
        node.dial(peer_addr)?;
    }
    
    println!("\nWaiting for connections...\n");
    
    // FIX #6: Create interval timer to check for pending objects to broadcast
    let mut broadcast_interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
    broadcast_interval.tick().await; // First tick completes immediately
    
    // Event loop
    loop {
        tokio::select! {
            event = node.swarm.next() => {
                if let Some(event) = event {
                    if let Err(e) = handle_user_event(&mut node, event).await {
                        eprintln!("Error handling event: {}", e);
                    }
                }
            }
            
            // FIX #6: Periodically check for pending objects to broadcast
            _ = broadcast_interval.tick() => {
                if let Err(e) = broadcast_pending_objects(&mut node) {
                    eprintln!("[PUSH] Error broadcasting pending objects: {}", e);
                }
            }
        }
    }
}

async fn handle_user_event(
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
                                    } else {
                                        // FIX #3: Trigger sync after successfully handling Hello
                                        // This ensures sync happens regardless of which Hello message arrives last
                                        println!("[HANDSHAKE] Hello handled successfully, initiating sync");
                                        if let Err(e) = node.sync_with_peer(peer) {
                                            eprintln!("[SYNC] Error starting sync after Hello: {}", e);
                                        }
                                    }
                                    RealmResponse::HelloAck {
                                        role: realm::types::NodeRole::User,
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
                                        None => RealmResponse::Error("Object not found".to_string()),
                                    }
                                }
                                
                                RealmRequest::LogRequest { start_version } => {
                                    // Users can serve log requests to other users
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
                                    
                                    // NEW: Broadcast pending objects after first successful handshake
                                    if let Err(e) = broadcast_pending_objects(node) {
                                        eprintln!("[ERROR] Failed to broadcast pending objects: {}", e);
                                    }
                                }
                                
                                RealmResponse::Head(peer_head) => {
                                    println!("[SYNC] Received head from {}: version={}", peer, peer_head.gov_log_version);
                                    node.handle_peer_head(peer, peer_head)?;
                                }
                                
                                RealmResponse::Object(bytes) => {
                                    println!("[SYNC] Received object from {}: {} bytes", peer, bytes.len());
                                    match node.handle_received_object(bytes) {
                                        Ok(_) => println!("[SYNC] Object stored and applied to state"),
                                        Err(e) => eprintln!("[ERROR] Failed to process object: {}", e),
                                    }
                                }
                                
                                RealmResponse::LogsChunk(logs) => {
                                    println!("[SYNC] Received logs chunk with {} entries", logs.len());
                                    match node.handle_logs_chunk(logs) {
                                        Ok(_) => {
                                            println!("[SYNC] Logs chunk applied successfully");
                                            
                                            // FIX #6: Broadcast pending objects after successful sync
                                            // This ensures obligations created before the node started get sent
                                            if let Err(e) = broadcast_pending_objects(node) {
                                                eprintln!("[PUSH] Error broadcasting pending objects after sync: {}", e);
                                            }
                                        }
                                        Err(e) => eprintln!("[ERROR] Failed to process logs chunk: {}", e),
                                    }
                                }
                                
                                RealmResponse::PushAck => {
                                    println!("[PUSH] Object push acknowledged by {}", peer);
                                }
                                
                                RealmResponse::Error(err) => {
                                    eprintln!("[ERROR] From peer {}: {}", peer, err);
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

fn start_task(data_dir: &str, task_name: &str) -> Result<()> {
    println!("Starting task: {}\n", task_name);
    
    // Load user state
    let state_str = fs::read_to_string(Path::new(data_dir).join("user_state.json"))?;
    let mut user_state: UserStateManager = serde_json::from_str(&state_str)?;
    
    // Load government log to get task rules
    let object_store = ObjectStore::new("./objects")?;
    let objects = object_store.list_all()?;
    
    let mut task_rule: Option<ActionRule> = None;
    for hash in objects {
        if let Some(bytes) = object_store.get_object(&hash)? {
            if let Ok(RealmObject::GovernmentRuleLog(log)) = serde_json::from_slice(&bytes) {
                for rule in log.state_rules {
                    if rule.task_name == task_name {
                        task_rule = Some(rule);
                        break;
                    }
                }
            }
        }
    }
    
    let rule = task_rule.ok_or_else(|| anyhow::anyhow!("Task '{}' not found in government rules", task_name))?;
    
    println!("Task: {}", rule.task_name);
    println!("  Coins: {}", rule.coin_multiplier);
    println!("  Required resources:");
    println!("    Land: {}", rule.required_resources.land);
    println!("    Labor: {}", rule.required_resources.labor);
    println!("    Capital: {}", rule.required_resources.capital);
    
    user_state.start_task(task_name.to_string(), &rule)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    
    println!("\n[OK] Task started!");
    println!("  Resources remaining:");
    println!("    Land: {}", user_state.resources.land);
    println!("    Labor: {}", user_state.resources.labor);
    println!("    Capital: {}", user_state.resources.capital);
    
    // Save state
    fs::write(
        Path::new(data_dir).join("user_state.json"),
        serde_json::to_string_pretty(&user_state)?,
    )?;
    
    println!("\nWait 60 seconds, then run:");
    println!("  user harvest-task {} --data-dir {}", task_name, data_dir);
    
    Ok(())
}

fn harvest_task(data_dir: &str, task_name: &str) -> Result<()> {
    println!("Harvesting task: {}\n", task_name);
    
    // Load user state
    let state_str = fs::read_to_string(Path::new(data_dir).join("user_state.json"))?;
    let mut user_state: UserStateManager = serde_json::from_str(&state_str)?;
    
    // Load government log to get multiplier
    let object_store = ObjectStore::new("./objects")?;
    let objects = object_store.list_all()?;
    
    let mut multiplier = 0;
    for hash in objects {
        if let Some(bytes) = object_store.get_object(&hash)? {
            if let Ok(RealmObject::GovernmentRuleLog(log)) = serde_json::from_slice(&bytes) {
                for rule in log.state_rules {
                    if rule.task_name == task_name {
                        multiplier = rule.coin_multiplier;
                        break;
                    }
                }
            }
        }
    }
    
    let coins = user_state.harvest_task(task_name, multiplier)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    
    println!("[OK] Task harvested!");
    println!("  Coins earned: {}", coins);
    println!("  New balance: {}", user_state.coins);
    
    // Save state
    fs::write(
        Path::new(data_dir).join("user_state.json"),
        serde_json::to_string_pretty(&user_state)?,
    )?;
    
    println!("\nNow create an obligation:");
    println!("  user submit-obligation {} --data-dir {}", coins, data_dir);
    
    Ok(())
}

fn submit_obligation(data_dir: &str, amount: u64) -> Result<()> {
    println!("Creating tax obligation for {} coins...\n", amount);
    
    // Load keys
    let signing_key_bytes = fs::read(Path::new(data_dir).join("signing_key.bin"))?;
    let signing_key = SigningKey::from_bytes(&signing_key_bytes.try_into()
        .map_err(|_| anyhow::anyhow!("Invalid signing key"))?);
    
    let realm_id_hex = fs::read_to_string(Path::new(data_dir).join("realm_id.txt"))?;
    let realm_id_bytes = hex::decode(realm_id_hex.trim())?;
    let mut realm_id = [0u8; 32];
    realm_id.copy_from_slice(&realm_id_bytes);
    
    // Get current government log from object store
    let object_store = ObjectStore::new("./objects")?;
    let objects = object_store.list_all()?;
    
    let mut gov_log_hash: Option<HASH> = None;
    for hash in objects {
        if let Some(bytes) = object_store.get_object(&hash)? {
            if let Ok(RealmObject::GovernmentRuleLog(_)) = serde_json::from_slice(&bytes) {
                gov_log_hash = Some(hash);
                break;
            }
        }
    }
    
    let log_hash = gov_log_hash.ok_or_else(|| anyhow::anyhow!("No government log found. Connect to government first."))?;
    
    // Create obligation
    let gov_log_ref = GovernmentLogRef {
        realm_id,
        log_hash,
    };
    
    let mut obligation = TaxObligation {
        user_id: signing_key.verifying_key(),
        government_log: gov_log_ref,
        coin_amount: amount,
        obligation_hash: [0; 32],
        user_signature: [0; 64],
    };
    
    // Compute hash
    let mut unsigned = obligation.clone();
    unsigned.user_signature = [0; 64];
    unsigned.obligation_hash = [0; 32];
    let hash_bytes = serde_json::to_vec(&unsigned)?;
    obligation.obligation_hash = *blake3::hash(&hash_bytes).as_bytes();
    
    // Sign
    let sig = signing_key.sign(&obligation.obligation_hash);
    obligation.user_signature = sig.to_bytes();
    
    // Create envelope
    let envelope_bytes = serde_json::to_vec(&obligation)?;
    let envelope_sig = signing_key.sign(&envelope_bytes);
    
    let envelope = TaxObligationEnvelope {
        obligation,
        signature: envelope_sig.to_bytes(),
    };
    
    // Store as object
    let object = RealmObject::TaxObligationEnvelope(envelope);
    let object_bytes = serde_json::to_vec(&object)?;
    let object_hash = blake3::hash(&object_bytes);
    
    object_store.store_object(object_hash.as_bytes(), &object_bytes)?;
    
    println!("[OK] Obligation created!");
    println!("  Hash: {}", hex::encode(object_hash.as_bytes()));
    println!("  Amount: {} coins", amount);
    
    // NEW: Queue for broadcast when node is running
    fs::create_dir_all("./pending_push")?;
    let pending_file = format!("./pending_push/{}.json", hex::encode(object_hash.as_bytes()));
    fs::write(&pending_file, &object_bytes)?;
    
    println!("\n[QUEUED] Obligation queued for network broadcast.");
    println!("Start your node to propagate it:");
    println!("  user run --data-dir {}", data_dir);
    
    Ok(())
}

fn send_coins(data_dir: &str, to_hex: &str, amount: i64) -> Result<()> {
    println!("Sending {} coins to {}...\n", amount, to_hex);
    
    // Load user key
    let signing_key_bytes = fs::read(Path::new(data_dir).join("signing_key.bin"))?;
    let signing_key = SigningKey::from_bytes(&signing_key_bytes.try_into()
        .map_err(|_| anyhow::anyhow!("Invalid signing key"))?);
    
    // Parse recipient key
    let to_bytes = hex::decode(to_hex)?;
    let to_key = VerifyingKey::from_bytes(to_bytes.as_slice().try_into()?)?;
    
    // Create P2P transfer receipt
    let mut receipt = P2PTransferReceipt {
        from: signing_key.verifying_key(),
        to: to_key,
        amount: -amount,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        signature: [0; 64],
    };
    
    // Sign
    let receipt_bytes = serde_json::to_vec(&receipt)?;
    let sig = signing_key.sign(&receipt_bytes);
    receipt.signature = sig.to_bytes();
    
    // Create envelope
    let envelope_bytes = serde_json::to_vec(&receipt)?;
    let envelope_sig = signing_key.sign(&envelope_bytes);
    
    let envelope = ReceiptEnvelope {
        receipt,
        signature: envelope_sig.to_bytes(),
    };
    
    // Store as object
    let object = RealmObject::P2PReceiptEnvelope(envelope);
    let object_bytes = serde_json::to_vec(&object)?;
    let object_hash = blake3::hash(&object_bytes);
    
    let object_store = ObjectStore::new("./objects")?;
    object_store.store_object(object_hash.as_bytes(), &object_bytes)?;
    
    println!("[OK] P2P transfer created!");
    println!("  Hash: {}", hex::encode(object_hash.as_bytes()));
    println!("  Amount: {} coins", amount);
    
    println!("\nTransfer stored locally.");
    println!("Connect to recipient to deliver payment.");
    
    Ok(())
}

fn show_status(data_dir: &str) -> Result<()> {
    println!("=== USER NODE STATUS ===\n");
    
    // Load config
    let realm_id = fs::read_to_string(Path::new(data_dir).join("realm_id.txt"))?;
    let gov_key = fs::read_to_string(Path::new(data_dir).join("gov_key.txt"))?;
    
    let signing_key_bytes = fs::read(Path::new(data_dir).join("signing_key.bin"))?;
    let signing_key = SigningKey::from_bytes(&signing_key_bytes.try_into()
        .map_err(|_| anyhow::anyhow!("Invalid signing key"))?);
    
    println!("Realm ID:       {}", realm_id.trim());
    println!("Gov Key:        {}", gov_key.trim());
    println!("User Key:       {}", hex::encode(signing_key.verifying_key().to_bytes()));
    
    // Load user state
    if let Ok(state_str) = fs::read_to_string(Path::new(data_dir).join("user_state.json")) {
        if let Ok(user_state) = serde_json::from_str::<UserStateManager>(&state_str) {
            println!("\nBalance:        {} coins", user_state.coins);
            println!("Active Tasks:   {}", user_state.active_tasks.len());
            println!("\nResources:");
            println!("  Land:         {}", user_state.resources.land);
            println!("  Labor:        {}", user_state.resources.labor);
            println!("  Capital:      {}", user_state.resources.capital);
        }
    }
    
    // Check object store
    if let Ok(store) = ObjectStore::new("./objects") {
        if let Ok(objects) = store.list_all() {
            println!("\nObject Store:   {} objects", objects.len());
            
            let mut gov_logs = 0;
            let mut obligations = 0;
            let mut receipts = 0;
            
            for hash in &objects {
                if let Some(bytes) = store.get_object(hash).ok().flatten() {
                    if let Ok(obj) = serde_json::from_slice::<RealmObject>(&bytes) {
                        match obj {
                            RealmObject::GovernmentRuleLog(_) => gov_logs += 1,
                            RealmObject::TaxObligationEnvelope(_) => obligations += 1,
                            RealmObject::TaxReceiptEnvelope(_) => receipts += 1,
                            RealmObject::P2PReceiptEnvelope(_) => {},
                        }
                    }
                }
            }
            
            println!("  Gov logs:     {}", gov_logs);
            println!("  Obligations:  {}", obligations);
            println!("  Receipts:     {}", receipts);
        }
    }
    
    Ok(())
}