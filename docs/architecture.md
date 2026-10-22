# Architecture

[← Back to README](../README.md)

## System Components

DeSync consists of three main architectural layers:

```
┌─────────────────────────────────────────────────────────┐
│                   Application Layer                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  Government  │  │     User     │  │   CLI Tools  │  │
│  │     Node     │  │     Node     │  │   (run.sh)   │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│                    Network Layer                        │
│  ┌──────────────────────────────────────────────────┐  │
│  │         LibP2P Networking (P2PNode)              │  │
│  │  • Peer Discovery  • Message Routing             │  │
│  │  • Connection Management  • Protocol Handlers    │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│                    Storage Layer                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Object     │  │  Organized   │  │     Log      │  │
│  │    Store     │  │    Store     │  │    Index     │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
│  • Content-Addressed Storage  • Versioned Logs         │
│  • Blake3 Hashing  • File-Based Persistence            │
└─────────────────────────────────────────────────────────┘
```

## Node Types

### Government Node

The authoritative source of economic policy in a realm.

**Responsibilities:**

- Generate the initial realm identity (realm_id, government public key)
- Create and sign government rule logs with economic policies
- Process tax obligations and issue cryptographically signed receipts
- Serve as a source of truth for the latest economic rules
- Maintain the canonical chain of policy updates

**Key Operations:**

```rust
// Government Rule Log Structure
pub struct GovernmentRuleLog {
    pub version_number: u64,              // Monotonically increasing version
    pub realm_id: HASH,                   // Unique realm identifier
    pub previous_log_hash: HASH,          // Chain of logs
    pub state_rules: Vec<ActionRule>,     // Economic task definitions
    pub tax_amount: u64,                  // Current tax rate
    pub log_hash: HASH,                   // Self-hash for integrity
    pub government_signature: [u8; 64],   // Ed25519 signature
}
```

**Does NOT:**

- Track individual user balances (users maintain their own state)
- Validate resource allocation (users self-certify)
- Process P2P transactions (these occur directly between users)

### User Node

Participants in the economic realm who perform tasks, pay taxes, and transact.

**Responsibilities:**

- Maintain local economic state (coins, resources, active tasks)
- Synchronize government rule logs from government or peers
- Execute tasks according to the latest government rules
- Generate and submit tax obligations
- Participate in P2P transactions with other users
- Verify all received data using cryptographic signatures

**State Structure:**

```rust
// User maintains their own state derived from logs
pub struct UserState {
    coins: i64,                    // Current coin balance
    land: i64,                     // Available land resources
    labor: i64,                    // Available labor resources
    capital: i64,                  // Available capital resources
    active_tasks: Vec<Task>,       // Currently running tasks
    completed_tasks: Vec<Task>,    // Harvested tasks
    tax_receipts: Vec<TaxReceipt>, // Government-signed receipts
}
```

**Does NOT:**

- Modify government rules (only government can do this)
- Validate other users' states (each user is responsible for their own)
- Require constant government connection (can operate with cached logs)

## Data Structures

### Core Types

**ActionRule** - Defines a task type in the economy

```rust
pub struct ActionRule {
    pub task_name: String,           // e.g., "farming", "mining", "trading"
    pub coin_multiplier: u64,        // Reward multiplier (coins earned)
    pub required_resources: Resources, // Resources needed to start task
}
```

**Resources** - The three resource types in the economy

```rust
pub struct Resources {
    pub land: u64,      // Land resources (for farming, etc.)
    pub labor: u64,     // Labor resources (for all tasks)
    pub capital: u64,   // Capital resources (for trading, manufacturing)
}
```

**Task** - An active or completed task

```rust
pub struct Task {
    pub task_name: String,          // References an ActionRule
    pub start_time: u64,            // Unix timestamp of start
    pub duration: u64,              // How long task takes (seconds)
    pub resources_locked: Resources, // Resources committed to this task
}
```

**TaxObligation** - User's submission of tax payment

```rust
pub struct TaxObligation {
    pub user_id: VerifyingKey,           // User's public key
    pub government_log: GovernmentLogRef, // Which rules this uses
    pub coin_amount: u64,                // Tax amount being paid
    pub obligation_hash: HASH,           // Hash of obligation
    pub user_signature: [u8; 64],        // User's signature
}
```

**TaxReceipt** - Government's acknowledgment of tax payment

```rust
pub struct TaxReceipt {
    pub obligation_hash: HASH,         // References the obligation
    pub previous_receipt_hash: Option<HASH>, // Chain of receipts
    pub payment_slot: u64,             // Timestamp/slot number
    pub coins_transferred: u64,        // Amount received
    pub government_signature: [u8; 64], // Government's signature
}
```

**P2PTransferReceipt** - Direct user-to-user transaction

```rust
pub struct P2PTransferReceipt {
    pub from: VerifyingKey,    // Sender's public key
    pub to: VerifyingKey,      // Recipient's public key
    pub amount: i64,           // Coins transferred (can be negative)
    pub timestamp: u64,        // When transfer occurred
    pub signature: [u8; 64],   // Sender's signature
}
```

---

## Project Structure

```
desync/
├── src/                          # Rust source code
│   ├── bin/                      # Binary entry points
│   │   ├── government.rs         # Government node binary
│   │   ├── user.rs               # User node binary
│   │   └── load_genesis.rs       # Genesis loader utility
│   ├── crypto/                   # Cryptographic utilities
│   │   ├── ed25519_utils.rs      # Ed25519 signature helpers
│   │   ├── vdf.rs                # VDF (Verifiable Delay Function)
│   │   └── mod.rs
│   ├── gov/                      # Government node logic
│   │   ├── log_manager.rs        # Rule log management
│   │   ├── rule_manager.rs       # Economic rule handling
│   │   ├── receipt_signing.rs    # Tax receipt generation
│   │   └── mod.rs
│   ├── user/                     # User node logic
│   │   ├── state_manager.rs      # User state computation
│   │   ├── recovery.rs           # State recovery from logs
│   │   └── mod.rs
│   ├── network/                  # Networking layer
│   │   ├── libp2p.rs             # LibP2P node implementation
│   │   ├── protocol.rs           # Custom protocol definitions
│   │   ├── sync.rs               # Log synchronization
│   │   ├── state_apply.rs        # State application logic
│   │   └── mod.rs
│   ├── storage/                  # Storage layer
│   │   ├── object_store.rs       # Content-addressed storage
│   │   ├── organized_store.rs    # Organized object store
│   │   ├── log_index.rs          # Log indexing
│   │   └── mod.rs
│   ├── types/                    # Data structures
│   │   ├── types.rs              # Core type definitions
│   │   └── mod.rs
│   ├── utils/                    # Utility functions
│   │   ├── serialization.rs      # Serde helpers
│   │   ├── persistence.rs        # File I/O
│   │   ├── file_protection.rs    # File permissions
│   │   └── mod.rs
│   ├── lib.rs                    # Library root
│   └── main.rs                   # Main entry (if applicable)
├── run.sh                        # CLI wrapper script
├── Cargo.toml                    # Rust dependencies
├── Cargo.lock                    # Dependency lock file
├── README.md                     # Project overview
├── docs/                         # Detailed documentation
├── Guide.md                      # Detailed command guide
├── LICENSE                       # License information
├── .gitignore                    # Git ignore rules
└── nodes/                        # Runtime data (not in repo)
    ├── government/               # Government node data
    │   ├── config.toml
    │   ├── gov_keys/
    │   ├── objects/
    │   ├── node.log
    │   └── government.pid
    ├── alice/                    # Alice's node data
    ├── bob/                      # Bob's node data
    └── backups/                  # Backup archives
```

**Key Directories**:

- **`src/bin/`** : Executable binaries for government and user nodes
- **`src/gov/`** : Government-specific logic (rule management, receipts)
- **`src/user/`** : User-specific logic (state management, tasks)
- **`src/network/`** : P2P networking using LibP2P
- **`src/storage/`** : Content-addressed storage and indexing
- **`src/types/`** : Shared data structures (logs, receipts, tasks)

**Runtime Directories** (created during operation):

- **`nodes/government/`** : Government node state and logs
- **`nodes/alice/`** , **`nodes/bob/`** : User node states
- **`nodes/backups/`** : Backup archives

---

**See also:** [Core Concepts](concepts.md) · [Networking](networking.md) · [Security Model](security.md)
