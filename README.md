# DeSync

**A Decentralized Economic Realm with Blockchain-Based Governance and Peer-to-Peer Transactions**

---

## Table of Contents

- [Overview](#overview)
- [Key Features](#key-features)
- [Architecture](#architecture)
  - [System Components](#system-components)
  - [Node Types](#node-types)
  - [Data Structures](#data-structures)
- [Core Concepts](#core-concepts)
  - [Realms](#realms)
  - [Economic Model](#economic-model)
  - [Cryptographic Guarantees](#cryptographic-guarantees)
- [Installation](#installation)
  - [Prerequisites](#prerequisites)
  - [Building from Source](#building-from-source)
- [Quick Start Guide](#quick-start-guide)
  - [Initial Setup](#initial-setup)
  - [Running a Government Node](#running-a-government-node)
  - [Running User Nodes](#running-user-nodes)
  - [Peer-to-Peer Network](#peer-to-peer-network)
- [Configuration](#configuration)
  - [Realm Configuration](#realm-configuration)
  - [Network Configuration](#network-configuration)
  - [Configuration Synchronization](#configuration-synchronization)
- [Usage](#usage)
  - [Government Operations](#government-operations)
  - [User Operations](#user-operations)
  - [Maintenance Operations](#maintenance-operations)
- [Command Reference](#command-reference)
  - [Setup Commands](#setup-commands)
  - [Node Management](#node-management)
  - [Government Commands](#government-commands)
  - [User Commands](#user-commands)
  - [Maintenance Commands](#maintenance-commands)
- [Network Architecture](#network-architecture)
  - [Connection Topology](#connection-topology)
  - [Log Synchronization](#log-synchronization)
  - [P2P Transactions](#p2p-transactions)
- [Security Model](#security-model)
  - [Cryptographic Foundations](#cryptographic-foundations)
  - [Signature Verification](#signature-verification)
  - [State Integrity](#state-integrity)
- [Common Workflows](#common-workflows)
  - [Setting Up a Multi-User Network](#setting-up-a-multi-user-network)
  - [Economic Activities](#economic-activities)
  - [Policy Updates](#policy-updates)
- [Troubleshooting](#troubleshooting)
  - [Common Issues](#common-issues)
  - [Debug Commands](#debug-commands)
  - [Configuration Problems](#configuration-problems)
- [Advanced Topics](#advanced-topics)
  - [Custom Task Economies](#custom-task-economies)
  - [Multi-Machine Deployment](#multi-machine-deployment)
  - [Backup and Recovery](#backup-and-recovery)
- [Project Structure](#project-structure)
- [Development](#development)
- [Roadmap](#roadmap)

---

## Overview

**DeSync** is a decentralized economic simulation system built with Rust, featuring blockchain-based governance, peer-to-peer transactions, and a task-based economic model. The system creates isolated "realms" where a government node defines economic rules, and user nodes participate in economic activities, pay taxes, and transact directly with each other.

At its core, DeSync demonstrates:

- **Decentralized Governance**: A government node establishes and updates economic policies through cryptographically signed rule logs
- **Peer-to-Peer Economy**: Users can engage in economic activities and transfer value without constant government oversight
- **Blockchain-Inspired Design**: Immutable logs with hash chains and cryptographic signatures ensure data integrity
- **Resource-Based Economics**: Task completion requires resource allocation (land, labor, capital) and generates rewards
- **Flexible Network Topology**: Support for direct government connections or peer-to-peer relay networks

### What Makes DeSync Unique?

DeSync implements a **hybrid consensus model** where:
- Government nodes have **policy authority** (they can change rules)
- User nodes have **economic autonomy** (they control their activities)
- The network operates with **eventual consistency** (changes propagate through the network)
- Transactions are **cryptographically verified** but don't require mining or proof-of-work

This creates an interesting middle ground between centralized control and full decentralization, suitable for simulations, experimental economics, local community currencies, or educational demonstrations of blockchain concepts.

---

## Key Features

### Decentralized Governance
- Government nodes establish realms with unique identities
- Dynamic economic policy updates (tax rates, task definitions)
- Versioned rule logs with complete audit trails
- Cryptographically signed policy changes

### Task-Based Economics
- Define custom tasks with resource requirements (land, labor, capital)
- Configurable coin multipliers for task rewards
- Time-locked task execution with harvest mechanics
- Resource allocation tracking

### Cryptographic Security
- Ed25519 signature verification for all transactions
- Blake3 hashing for data integrity
- Verifiable receipts for tax payments and P2P transfers
- Content-addressed storage with immutable logs

### Flexible Networking
- LibP2P-based peer-to-peer communication
- Support for direct connections or relay networks
- Automatic log synchronization across nodes
- Bootstrap peer configuration

### State Management
- Deterministic state computation from logs
- Resource tracking (coins, land, labor, capital)
- Task lifecycle management
- Tax obligation tracking

### Developer-Friendly
- Comprehensive CLI wrapper (`run.sh`)
- Simple initialization and configuration
- Built-in backup and restore
- Extensive logging and debugging tools

---

## Architecture

### System Components

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

### Node Types

#### **Government Node**
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

#### **User Node**
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

### Data Structures

#### **Core Types**

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

## Core Concepts

### Realms

A **realm** is an isolated economic system with its own government, rules, and participants. Each realm is identified by a unique `realm_id` (a 32-byte hash) that is generated when the government node is initialized.

**Key Properties of Realms:**

1. **Unique Identity**: Each realm has a cryptographically unique identifier
2. **Single Government**: One government node per realm (identified by public key)
3. **Multiple Users**: Unlimited number of user participants
4. **Shared Configuration**: All nodes must agree on `realm_id` and `government_key`
5. **Isolated Economics**: Realms are completely independent of each other

**Why Realms?**
Realms allow multiple independent economic experiments to coexist. You might run:
- A test realm with experimental policies
- A production realm with stable rules
- Multiple competitive realms with different economic models

### Economic Model

DeSync implements a **resource-based task economy**:

#### **Resources**
Users have three types of resources:
- **Land**: Required for agricultural and extraction tasks
- **Labor**: Required for all tasks (universal resource)
- **Capital**: Required for manufacturing and trading tasks

Resources are:
- **Allocated** when starting a task (locked until harvest)
- **Released** when harvesting a task
- **Conserved** (total resources don't change, just move between allocated/available)

#### **Tasks**
Economic activities are defined as tasks:

1. **Task Definition** (by government):
   ```toml
   [task.farming]
   multiplier = 30        # Earn 30 coins on completion
   land = 10             # Requires 10 land
   labor = 5             # Requires 5 labor
   capital = 0           # Requires 0 capital
   ```

2. **Task Lifecycle** (by user):
   ```
   Start Task → Lock Resources → Wait Duration → Harvest → Earn Coins + Release Resources
   ```

3. **Duration**:
   - Tasks are completed by executing a Verifiable Delay Function (VDF) during harvest
   - VDF is currently set at 500,000 iterations (approximately 10-20 seconds on a standard PC)
   - Performance varies based on PC specifications
   - VDF ensures tasks require actual computation time, simulating real work
   - The VDF computation occurs when the user harvests the task, not when starting it

#### **Taxation**
The government sets a tax rate (in coins):
- Users self-report their tax obligation
- Government issues signed receipts upon receipt
- Tax receipts can be used to prove compliance
- No enforcement mechanism (participation is voluntary)

#### **Coin Flow**
```
Tasks → User Earns Coins → User Pays Tax → Coins to Government
                        ↓
                  User Sends to Peer → Peer Receives Coins
```

Coins are created through task completion and flow through the economy via taxation and P2P transfers.

### Cryptographic Guarantees

DeSync uses cryptography to ensure data integrity without centralized validation:

#### **Hash Chains**
Government rule logs form a hash chain:
```
Genesis Log (v0) → Log v1 → Log v2 → Log v3 → ...
     ↓                ↓         ↓         ↓
  hash_0          hash_1    hash_2    hash_3
                     ↑         ↑         ↑
          previous_hash  previous_hash  previous_hash
```

This ensures:
- Logs cannot be retroactively modified
- Version history is immutable
- Any tampering is immediately detectable

#### **Digital Signatures**
All critical data is signed:
- **Government Rule Logs**: Signed by government's private key
- **Tax Obligations**: Signed by user's private key
- **Tax Receipts**: Signed by government's private key
- **P2P Transfers**: Signed by sender's private key

Verification:
```rust
// Anyone can verify a signature using the public key
pub fn verify_signature(&self, gov_key: &VerifyingKey) -> bool {
    // Uses Ed25519 to verify signature matches data
}
```

#### **Content Addressing**
All objects are stored by their Blake3 hash:
```
Object → Blake3 Hash → Storage Key
```

This guarantees:
- Identical data always has the same hash
- Different data always has different hashes
- Content cannot be changed without changing the hash
- References are tamper-evident

---

## Installation

### Prerequisites

**Required:**
- **Rust** (1.70 or later): [Install Rust](https://rustup.rs/)
- **Cargo**: Comes with Rust installation
- **Git**: For cloning the repository

**Operating System:**
- Linux (recommended)
- macOS
- Windows (with WSL2)

### Building from Source

1. **Clone the repository**:
   ```bash
   git clone https://github.com/yourusername/desync.git
   cd desync
   ```

2. **Build the project**:
   ```bash
   cargo build --release
   ```

   This will create three binaries in `target/release/`:
   - `government` - Government node binary
   - `user` - User node binary
   - `load_genesis` - Genesis configuration utility

3. **Make run.sh executable**:
   ```bash
   chmod +x run.sh
   ```

4. **Test the installation**:
   ```bash
   ./run.sh init-gov
   ./run.sh start government
   ./run.sh gov-info
   ./run.sh stop government
   ```

---

## Quick Start Guide

This section will get you running a complete DeSync network in under 5 minutes.

**Note:** The workflows and examples in this guide assume multiple nodes running on the same PC. The system was only tested on a single PC embodying multiple nodes. Multi-PC deployments have not been tested (because I only have one PC).

### Initial Setup

**Step 1: Initialize the Government Node**

```bash
./run.sh init-gov
```

This creates:
- A government node directory at `government/`
- A unique realm identity (realm_id)
- A government keypair (public/private keys)
- Genesis rule logs with default tasks

**Step 2: Start the Government Node**

```bash
./run.sh start government
```

The government node will:
- Start listening on port 9000
- Display its multiaddress (needed for user connections)
- Begin accepting connections

**Step 3: Capture Government Address**

```bash
./run.sh update-gov-addr
```

This reads the government's multiaddress from logs and updates the configuration. This step is crucial for users to connect.

### Running a Government Node

**View Government Information**:
```bash
./run.sh gov-info
```

Output:
```
Realm ID: e9055adcca53ecaaceb68591bf533ab056ff76869388cc54aef8fe72134a26cc
Government Public Key: 99e04793744d78d1b832a3f074377909561463e24b29fe3c40726afe76b3de97

Genesis Rules:
  Task: farming (multiplier: 30, land: 10, labor: 5, capital: 0)
  Task: mining (multiplier: 40, land: 5, labor: 10, capital: 5)
  Task: trading (multiplier: 50, land: 0, labor: 5, capital: 15)
```

**Modify Economic Rules**:
```bash
# Set tax rate to 15 coins
./run.sh gov-set-tax 15

# Add a new task
./run.sh gov-add-task manufacturing 60 5 8 12

# Modify an existing task
./run.sh gov-modify-task farming --multiplier 35

# View current rules
./run.sh gov-show-rules
```

### Running User Nodes

**Step 1: Initialize User Node**

```bash
./run.sh init-user alice
```

This creates:
- A user directory at `alice/`
- Copies the realm configuration from government
- Generates Alice's unique keypair
- Initializes empty state

**Step 2: Start User Connected to Government**

```bash
./run.sh start alice --gov
```

This starts Alice's node and connects it directly to the government node for log synchronization.

**Step 3: Perform Economic Activities**

```bash
# Start a farming task
./run.sh task-start farming

# Harvest the task to earn coins (VDF computation happens here)
# VDF: 500,000 iterations, ~10-20 seconds on standard PC
./run.sh task-harvest farming

# Pay taxes
./run.sh pay-tax
```

### Peer-to-Peer Network

**Step 1: Add a Second User**

```bash
./run.sh init-user bob
./run.sh start bob --gov
```

**Step 2: Enable P2P Connection**

After both nodes are running, they need each other's addresses in their configuration:

```bash
# Edit alice/config.toml
[bootstrap]
bob = "/ip4/172.17.0.1/tcp/35227/p2p/12D3KooW..."  # Add Bob's address

# Edit bob/config.toml
[bootstrap]
alice = "/ip4/172.17.0.1/tcp/42593/p2p/12D3KooW..."  # Add Alice's address
```

**Step 3: Restart for P2P**

```bash
./run.sh restart alice
./run.sh start bob --peer alice
```

**Step 4: Send Coins P2P**

```bash
# Alice sends 25 coins to Bob
./run.sh send alice bob 25
```

**Important**: P2P transactions do NOT require the government node to be running! Only the sender and recipient need to be connected.

---

## Configuration

### Realm Configuration

Every node in a realm MUST have identical realm configuration:

```toml
[realm]
realm_id = "e9055adcca53ecaaceb68591bf533ab056ff76869388cc54aef8fe72134a26cc"
government_key = "99e04793744d78d1b832a3f074377909561463e24b29fe3c40726afe76b3de97"
```

**Critical Rules:**
1. **realm_id**: Must be exactly the same on all nodes
2. **government_key**: Must be exactly the same on all nodes
3. **Never modify these manually** - always copy from government initialization

**Why This Matters:**
- Different realm_ids mean nodes are in different realms (can't communicate)
- Different government_keys mean nodes won't accept each other's signatures
- Configuration mismatches cause silent failures (transactions rejected)

### Network Configuration

**Government Node (`government/config.toml`)**:
```toml
[node]
type = "government"
name = "government"
data_dir = "."

[realm]
realm_id = "e9055adcca53ecaaceb68591bf533ab056ff76869388cc54aef8fe72134a26cc"
government_key = "99e04793744d78d1b832a3f074377909561463e24b29fe3c40726afe76b3de97"

[network]
listen = "/ip4/0.0.0.0/tcp/9000"  # Fixed port for government

[network_info]
multiaddress = "/ip4/172.17.0.1/tcp/9000/p2p/12D3KooW..."  # Auto-populated

[identity]
government_key = "public_key_here"  # Government's public key (for signing)
```

**User Node (`alice/config.toml`)**:
```toml
[node]
type = "user"
name = "alice"
data_dir = "."

[realm]
realm_id = "e9055adcca53ecaaceb68591bf533ab056ff76869388cc54aef8fe72134a26cc"  # MUST MATCH
government_key = "99e04793744d78d1b832a3f074377909561463e24b29fe3c40726afe76b3de97"  # MUST MATCH

[network]
listen = "/ip4/0.0.0.0/tcp/0"  # Random port (0 = auto-assign)

[network_info]
multiaddress = "/ip4/172.17.0.1/tcp/42593/p2p/12D3KooW..."  # Auto-populated

[bootstrap]
government = "/ip4/172.17.0.1/tcp/9000/p2p/12D3KooW..."  # Government's address
bob = "/ip4/172.17.0.1/tcp/35227/p2p/12D3KooW..."        # Other peers

[identity]
user_key = "private_key_here"  # Alice's private key (UNIQUE)
```

### Configuration Synchronization

**Automatic Synchronization (Recommended)**:
When you run `./run.sh init-user alice`, the script automatically:
1. Reads government's `realm_id` and `government_key`
2. Copies them to alice's configuration
3. Generates alice's unique user key
4. Ensures configuration consistency

**Manual Synchronization (Advanced)**:
If setting up on different machines:
```bash
# On government machine
./run.sh gov-info  # Note the realm_id and government_key

# On user machine
./run.sh init-user alice <realm_id> <government_key>
```

---

## Usage

### Government Operations

#### Setting Tax Rates

```bash
./run.sh gov-set-tax <amount>
```

Example:
```bash
# Set tax to 20 coins
./run.sh gov-set-tax 20

# This creates a new government rule log with an incremented version number
# Users will sync this log and pay the new tax rate
```

#### Managing Tasks

**Note:** Any modification to economic rules (adding, modifying, or removing tasks, or changing tax rates) creates a new Government Rule Log with an incremented version number. This ensures a complete audit trail of all policy changes.

**Add a New Task**:
```bash
./run.sh gov-add-task <name> <multiplier> <land> <labor> <capital>
```

Example:
```bash
# Add a new "research" task
./run.sh gov-add-task research 80 2 15 20
```

**Modify Existing Task**:
```bash
./run.sh gov-modify-task <name> [options]
```

Options:
- `--multiplier N` - Change coin reward
- `--land N` - Change land requirement
- `--labor N` - Change labor requirement
- `--capital N` - Change capital requirement

Example:
```bash
# Make farming more profitable
./run.sh gov-modify-task farming --multiplier 45

# Change multiple parameters
./run.sh gov-modify-task mining --multiplier 50 --labor 12
```

**Remove a Task**:
```bash
./run.sh gov-remove-task <name>
```

Example:
```bash
# Remove obsolete task
./run.sh gov-remove-task oldtask
```

#### Viewing Current Rules

```bash
./run.sh gov-show-rules
```

Output:
```
=== Current Government Rules (Version 3) ===
Tax Amount: 20 coins

Active Tasks:
  • farming: 45 coins (land: 10, labor: 5, capital: 0)
  • mining: 50 coins (land: 5, labor: 12, capital: 5)
  • trading: 50 coins (land: 0, labor: 5, capital: 15)
  • research: 80 coins (land: 2, labor: 15, capital: 20)
```

### User Operations

#### Task Management

**Starting a Task**:
```bash
./run.sh task-start <task_name>
```

Example:
```bash
./run.sh task-start farming
```

This will:
1. Check if you have sufficient resources
2. Lock the required resources
3. Record the start time
4. Create a task entry in your state

**Harvesting a Task**:
```bash
./run.sh task-harvest <task_name>
```

Example:
```bash
./run.sh task-harvest farming
```

This will:
1. Execute the VDF (500,000 iterations, ~10-20 seconds on standard PC)
2. Verify the VDF computation completes successfully
3. Add coins to your balance (based on multiplier)
4. Release the locked resources
5. Mark the task as completed

#### Tax Payment

```bash
./run.sh pay-tax
```

This command:
1. Syncs with government to get latest tax rate
2. Creates a tax obligation (signed by your private key)
3. Submits to government (requires direct government connection)
4. Receives a cryptographically signed receipt
5. Deducts coins from your balance

**Requirements**:
- **Must have direct government connection** (tax obligations can only be submitted directly to government, not via peers)
- Government connection is necessary for tax obligation submission only
- Log syncing can be done through peers if they have up-to-date logs
- Must have sufficient coins to pay tax
- Only one user node can be running

#### Peer-to-Peer Transfers

```bash
./run.sh send <sender> <recipient> <amount>
```

Example:
```bash
# Alice sends 30 coins to Bob
./run.sh send alice bob 30
```

This performs:
1. Sender creates and signs a transfer receipt
2. Receipt is sent to recipient via P2P network
3. Both parties update their local state
4. No government involvement required

**Requirements**:
- Sender node must be running
- Recipient node must be reachable (peer connection)
- Sender must have sufficient coins
- Government does NOT need to be online

**Important**: The sender's private key signs the transaction, so only they can initiate transfers from their account.

### Maintenance Operations

#### Node Management

**Start a Node**:
```bash
./run.sh start <node> [--gov|--peer <name>]
```

Options:
- No flag: Start standalone (no bootstrap connections)
- `--gov`: Connect to government node
- `--peer <name>`: Connect to a specific peer

**Stop a Node**:
```bash
./run.sh stop <node>
```

**Restart a Node**:
```bash
./run.sh restart <node>
```

**Check Node Status**:
```bash
./run.sh status <node>
```

**View Logs**:
```bash
./run.sh logs <node> [lines]
```

Example:
```bash
# View last 50 lines
./run.sh logs alice 50

# View last 20 lines (default)
./run.sh logs government
```

**List All Nodes**:
```bash
./run.sh list
```

#### Data Management

**Backup Nodes**:
```bash
./run.sh backup [node]
```

Examples:
```bash
# Backup specific node
./run.sh backup alice

# Backup all nodes
./run.sh backup
```

Backups are stored in `./nodes/backup_<timestamp>.tar.gz`

**Restore from Backup**:
```bash
./run.sh restore <backup-file>
```

Example:
```bash
./run.sh restore nodes/backup_20260206_143022.tar.gz
```

**Clean Node Data**:
```bash
./run.sh clean [node]
```

Examples:
```bash
# Clean specific node
./run.sh clean alice

# Clean all nodes
./run.sh clean
```

**Warning**: This deletes all node data! Always backup first.

#### Troubleshooting Tools

**Kill Processes on Port**:
```bash
./run.sh kill-port <port>
```

Example:
```bash
./run.sh kill-port 9000
```

**Kill All Nodes**:
```bash
./run.sh kill-all
```

This forcefully terminates all running DeSync nodes and cleans up PID files.

---

## Command Reference

### Setup Commands

| Command | Description | Example |
|---------|-------------|---------|
| `init-gov` | Initialize government node | `./run.sh init-gov` |
| `init-user <name>` | Initialize user node | `./run.sh init-user alice` |
| `update-gov-addr` | Update government multiaddress | `./run.sh update-gov-addr` |

### Node Management

| Command | Description | Example |
|---------|-------------|---------|
| `start <node>` | Start a node | `./run.sh start government` |
| `start <node> --gov` | Start user connected to government | `./run.sh start alice --gov` |
| `start <node> --peer <name>` | Start user connected to peer | `./run.sh start bob --peer alice` |
| `stop <node>` | Stop a running node | `./run.sh stop alice` |
| `restart <node>` | Restart a node | `./run.sh restart alice` |
| `status <node>` | Show node status and PID | `./run.sh status alice` |
| `logs <node> [lines]` | View recent logs | `./run.sh logs government 50` |
| `list` | List all nodes and their status | `./run.sh list` |

### Government Commands

| Command | Description | Example |
|---------|-------------|---------|
| `gov-info` | Show realm identity and genesis rules | `./run.sh gov-info` |
| `gov-show-rules` | Display current economic rules with version | `./run.sh gov-show-rules` |
| `gov-set-tax <amount>` | Set tax amount (creates new rule log) | `./run.sh gov-set-tax 15` |
| `gov-add-task <name> <mult> <land> <labor> <cap>` | Add new task definition | `./run.sh gov-add-task trading 50 10 5 2` |
| `gov-modify-task <name> [opts]` | Modify task parameters | `./run.sh gov-modify-task farming --multiplier 40` |
| `gov-remove-task <name>` | Remove a task from rules | `./run.sh gov-remove-task obsolete` |

**Task Modification Options**:
- `--multiplier N` - Set coin reward multiplier
- `--land N` - Set land resource requirement
- `--labor N` - Set labor resource requirement
- `--capital N` - Set capital resource requirement

### User Commands

| Command | Description | Requires Gov? | Requires Peer? |
|---------|-------------|---------------|----------------|
| `task-start <task>` | Start a task (locks resources) | No* | No |
| `task-harvest <task>` | Harvest completed task (earn coins) | No* | No |
| `pay-tax` | Submit tax obligation to government | **Yes (Direct)** | No |
| `send <sender> <recipient> <amount>` | P2P coin transfer | **No** | **Yes** |

*Requires latest government logs (can be obtained from peer or government for log syncing)

### Maintenance Commands

| Command | Description | Example |
|---------|-------------|---------|
| `clean [node]` | Clean node data directory | `./run.sh clean alice` |
| `backup [node]` | Backup node(s) to ./nodes/ | `./run.sh backup government` |
| `restore <file>` | Restore from backup archive | `./run.sh restore nodes/backup.tar.gz` |
| `kill-port <port>` | Kill all processes on port | `./run.sh kill-port 9000` |
| `kill-all` | Terminate all realm nodes | `./run.sh kill-all` |

---

## Network Architecture

### Connection Topology

DeSync supports multiple network topologies:

#### **Star Topology** (Government-Centric)
```
        Government
       /    |    \
      /     |     \
   Alice   Bob   Charlie
```

All users connect directly to government. Best for:
- Small networks
- High trust in government availability
- Simple setup

#### **Mesh Topology** (Peer-to-Peer)
```
        Government
             |
           Alice
          /     \
        Bob --- Charlie
```

Users connect to each other. Best for:
- Distributed networks
- Resilience to government downtime
- P2P-heavy workflows

#### **Hybrid Topology**
```
        Government
        /        \
     Alice      David
    /    \        |
  Bob    Charlie  Eve
```

Mix of direct and peer connections. Best for:
- Large networks
- Geographic distribution
- Load balancing

### Log Synchronization

Government rule logs propagate through the network:

**Direct Sync (User ↔ Government)**:
```
1. User connects to government
2. User requests logs newer than its current version
3. Government sends all newer logs
4. User verifies signatures and stores logs
```

**Peer Relay (User ↔ User)**:
```
1. Alice syncs with government (gets latest logs)
2. Bob connects to Alice as peer
3. Bob requests logs from Alice
4. Alice relays government logs to Bob
5. Bob verifies signatures (uses government_key)
```

**Key Points**:
- Logs are immutable (hash-chained)
- Signatures prevent forgery (even peers can't fake logs)
- Latest version always wins (monotonic version numbers)
- Peers can relay logs they received from government

### P2P Transactions

Peer-to-peer transfers happen without government:

```
Alice (Sender)                           Bob (Recipient)
      |                                        |
      | 1. Create transfer receipt             |
      | 2. Sign with Alice's private key       |
      |                                        |
      |-------- Send receipt via P2P -------->|
      |                                        |
      | 3. Deduct coins from Alice's state     |
      |                                   4. Verify signature
      |                                   5. Add coins to Bob's state
```

**Critical Features**:
- **No government required** (can be offline)
- **Cryptographic verification** (Bob verifies Alice's signature)
- **Eventual consistency** (both update their local state)

**Note**: The system uses a trust-based model where users self-report their balances. Government doesn't validate P2P transactions, making the system suitable for cooperative and experimental settings.

---

## Security Model

### Cryptographic Foundations

DeSync uses industry-standard cryptography:

#### **Ed25519 Digital Signatures**
- **Algorithm**: EdDSA (Edwards-curve Digital Signature Algorithm)
- **Key Size**: 256 bits (32 bytes)
- **Signature Size**: 512 bits (64 bytes)
- **Security**: Quantum-resistant design, fast verification

**Used For**:
- Government rule log signatures
- User tax obligation signatures
- Government tax receipt signatures
- P2P transfer signatures

#### **Blake3 Hashing**
- **Algorithm**: Blake3 (latest BLAKE family)
- **Output**: 256 bits (32 bytes)
- **Performance**: Extremely fast (multiple GB/s)
- **Security**: Collision-resistant, preimage-resistant

**Used For**:
- Content addressing (object storage)
- Log hash chains
- Data integrity verification
- Object identifiers

### Signature Verification

All critical objects must be verified:

```rust
// Example: Verifying a government rule log
fn verify_government_log(log: &GovernmentRuleLog, gov_key: &VerifyingKey) -> bool {
    // 1. Extract the signature from the log
    let signature = Signature::from_bytes(&log.government_signature)?;
    
    // 2. Verify signature matches the log hash
    gov_key.verify(&log.log_hash, &signature).is_ok()
}
```

**Verification Flow**:
```
Object Received → Extract Signature → Verify Against Public Key → Accept/Reject
```

**What Gets Verified**:
- Government rule logs (signed by government)
- Tax obligations (signed by user)
- Tax receipts (signed by government)
- P2P transfers (signed by sender)

**What Doesn't Get Verified**:
- User resource claims (self-reported)
- User coin balances (derived locally)
- Task completion times (trust-based)

### State Integrity

User state is computed deterministically:

```
Initial State + Ordered Log of Events = Current State
```

**State Derivation**:
```rust
// Pseudocode for state computation
fn compute_state(logs: Vec<RealmObject>) -> UserState {
    let mut state = UserState::default();
    
    for log in logs {
        match log {
            TaxReceipt => state.coins -= tax_amount,
            TaskHarvest => state.coins += reward,
            P2PTransfer => state.coins += amount,
            // ... more event types
        }
    }
    
    return state;
}
```

**Key Properties**:
- **Deterministic**: Same logs always produce same state
- **Reproducible**: Anyone with logs can verify state
- **Append-Only**: Old logs never change
- **Self-Certifying**: Users maintain their own state
- **Checksum-Protected**: User state files are checksum-protected to prevent manual tampering and detect unauthorized modifications

**Integrity Guarantees**:
- Government can't forge user transactions (lacks user's private key)
- Users can't forge government rules (lacks government's private key)
- Peers can't forge logs (all signed by originator)
- Hash chains prevent retroactive modification

**Integrity Limitations**:
- Users can lie about resources (not cryptographically verified)
- Government has ultimate authority (can change any rule)

---

## Common Workflows

### Setting Up a Multi-User Network

**Scenario**: Set up a government with three users (Alice, Bob, Charlie) where users can transact with each other.

```bash
# 1. Initialize and start government
./run.sh init-gov
./run.sh start government

# 2. Update government multiaddress in all node configurations
./run.sh update-gov-addr

# 3. Initialize all users
./run.sh init-user alice
./run.sh init-user bob
./run.sh init-user charlie

# 4. Start Alice connected to government
./run.sh start alice --gov

# 5. Get Alice's multiaddress from her TOML file
# Check alice/config.toml under [network_info] for multiaddress

# 6. Edit Bob's config to add Alice as peer
# Add to bob/config.toml under [bootstrap]:
# alice = "/ip4/172.17.0.1/tcp/42593/p2p/12D3KooW..."

# 7. Start Bob connected to Alice
./run.sh start bob --peer alice

# 8. Get Bob's multiaddress from his TOML file
# Check bob/config.toml under [network_info] for multiaddress

# 9. Edit Charlie's config to add both peers
# Add to charlie/config.toml under [bootstrap]:
# alice = "/ip4/..."
# bob = "/ip4/..."

# 10. Start Charlie
./run.sh start charlie --peer alice

# 11. Verify network
./run.sh list
```

**Network Topology**:
```
     Government
         |
       Alice
      /     \
    Bob --- Charlie
```

### Economic Activities

**Scenario**: Alice earns coins through tasks, pays taxes, and sends coins to Bob.

```bash
# 1. Alice starts a task
./run.sh task-start farming
# Output: "Task 'farming' started."

# 2. Alice harvests the task (VDF computation happens here: ~10-20 seconds)
./run.sh task-harvest farming
# Output: "Task 'farming' harvested! Earned 30 coins."

# 3. Alice pays taxes
./run.sh pay-tax
# Output: "Tax obligation submitted. Paid 15 coins."

# 4. Alice sends coins to Bob
./run.sh send alice bob 10
# Output: "Transfer successful. Sent 10 coins to bob."
```

**Flow**:
```
Step 1: Alice starts farming task (locks resources)
Step 2: Alice harvests farming (VDF executes ~10-20 seconds, earns 30 coins)
Step 3: Alice pays tax (balance: 30 - 15 = 15 coins)
Step 4: Alice sends to Bob (Alice: 5 coins, Bob: 10 coins)
```

### Policy Updates

**Scenario**: Government updates economic policy mid-operation.

```bash
# Initial state: Users are farming at 30 coins per task

# 1. Government increases farming rewards
./run.sh gov-set-tax 10
./run.sh gov-modify-task farming --multiplier 50

# 2. Government broadcasts new rules
# (Automatic - next user operation will sync)

# 3. Alice starts a new farming task
./run.sh task-start farming
# This uses the NEW rules (50 coin multiplier)

# 4. Alice harvests
sleep 60
./run.sh task-harvest farming
# Output: "Earned 50 coins" (new rate applied)

# 5. View rule history
./run.sh gov-show-rules
# Shows: Version 2, farming: 50 coins
```

**Key Points**:
- Users automatically sync new rules before task operations
- Old tasks (started before update) use old rules
- New tasks use current rules
- Complete version history is preserved

---

## Troubleshooting

### Common Issues

#### **Issue: "Port already in use"**

**Symptoms**:
```
Error: Address already in use (os error 98)
Failed to start node on port 9000
```

**Solutions**:
```bash
# Option 1: Kill processes on the port
./run.sh kill-port 9000

# Option 2: Kill all DeSync nodes
./run.sh kill-all

# Option 3: Manual cleanup
lsof -ti :9000 | xargs kill -9
```

#### **Issue: "Multiple user nodes running"**

**Symptoms**:
```
Error: Multiple user nodes are currently running
Cannot perform task operation
```

**Explanation**: Task operations (start, harvest, pay-tax) require exactly one user node.

**Solutions**:
```bash
# Stop other user nodes
./run.sh stop bob
./run.sh stop charlie

# Verify only one user running
./run.sh list
```

#### **Issue: "Realm configuration mismatch"**

**Symptoms**:
```
Error: Peer rejected connection
Warning: Invalid government signature
```

**Explanation**: Nodes have different realm_id or government_key values.

**Solutions**:
```bash
# 1. Verify configuration matches
grep -A2 "\[realm\]" government/config.toml
grep -A2 "\[realm\]" alice/config.toml

# 2. If mismatched, re-initialize user
./run.sh clean alice
./run.sh init-user alice

# 3. Restart nodes
./run.sh restart alice
```

#### **Issue: "Task not found"**

**Symptoms**:
```
Error: Task 'manufacturing' not found in government rules
```

**Explanation**: Task doesn't exist in government rules, or user's rule logs are out of sync with government.

**Solutions**:
```bash
# 1. Check government rules
./run.sh gov-show-rules

# 2. Restart user with government connection
./run.sh restart alice --gov

# 3. Try task operation again
./run.sh task-start farming
```

#### **Issue: "Insufficient resources"**

**Symptoms**:
```
Error: Insufficient labor to start task (need 10, have 5)
```

**Explanation**: All resources are locked in active tasks.

**Solutions**:
```bash
# 1. Check active tasks
./run.sh logs alice | grep -i "active tasks"

# 2. Harvest completed tasks to free resources
./run.sh task-harvest farming

# 3. Wait for tasks to complete before starting new ones
```

### Debug Commands

**Check Network Connectivity**:
```bash
# View node logs for connection status
./run.sh logs alice

# Check node's multiaddress in TOML file
cat alice/config.toml
```

**Inspect Object Store**:
```bash
# List stored objects
ls -lah alice/objects/

# View object content (requires hash)
cat alice/objects/e9055adcca53ecaaceb68591bf533ab056ff76869388cc54aef8fe72134a26cc
```

**Monitor Real-Time Activity**:
```bash
# Tail logs for all nodes
tail -f government/node.log alice/node.log bob/node.log
```

### Configuration Problems

#### **Missing Government Address**

**Problem**: Users can't connect to government.

**Solution**:
```bash
# 1. Ensure government is running
./run.sh status government

# 2. Update government address
./run.sh update-gov-addr

# 3. Restart user nodes
./run.sh restart alice
```

#### **Corrupt State Files**

**Problem**: Node fails to start with state loading errors.

**Solution**:
```bash
# 1. Backup current state
./run.sh backup alice

# 2. Clean and re-initialize
./run.sh clean alice
./run.sh init-user alice

# 3. Restore will re-sync from network
./run.sh start alice --gov
```

#### **Orphaned PID Files**

**Problem**: Script thinks node is running but it's not.

**Solution**:
```bash
# 1. Clean all PID files
./run.sh kill-all

# 2. Manually remove if needed
rm government/*.pid alice/*.pid bob/*.pid

# 3. Restart nodes
./run.sh start government
```

---

## Advanced Topics

### Custom Task Economies

You can design complex economic systems with creative task definitions:

**Example: Progression Economy**
```bash
# Tier 1: Basic tasks (low reward, low resources)
./run.sh gov-add-task gathering 10 2 2 0

# Tier 2: Intermediate tasks
./run.sh gov-add-task farming 30 10 5 0
./run.sh gov-add-task mining 40 5 10 5

# Tier 3: Advanced tasks (high reward, high resources)
./run.sh gov-add-task research 100 5 15 25
./run.sh gov-add-task manufacturing 120 10 20 30
```

**Example: Specialization Economy**
```bash
# Land-heavy tasks (agriculture)
./run.sh gov-add-task farming 30 20 5 0
./run.sh gov-add-task ranching 40 25 8 0

# Labor-heavy tasks (services)
./run.sh gov-add-task teaching 35 0 20 0
./run.sh gov-add-task healthcare 45 0 25 5

# Capital-heavy tasks (industry)
./run.sh gov-add-task trading 50 0 5 30
./run.sh gov-add-task banking 60 0 8 40
```

**Example: Seasonal Economy**
```bash
# Phase 1: Abundance (high rewards)
./run.sh gov-modify-task farming --multiplier 50
./run.sh gov-set-tax 5

# Phase 2: Scarcity (low rewards, high tax)
./run.sh gov-modify-task farming --multiplier 20
./run.sh gov-set-tax 25

# Phase 3: Recovery
./run.sh gov-modify-task farming --multiplier 35
./run.sh gov-set-tax 15
```

### Multi-Machine Deployment

**Important Note**: Multi-PC workflows require manual TOML configuration for each node, as the automatic multiaddress detection only works for single-PC multi-node operations. Multi-PC deployments have not been tested (because I only have one PC, and yes, that's actually the reason).

Running government and users on different machines:

**Machine 1 (Government Server)**:
```bash
# 1. Initialize government
./run.sh init-gov

# 2. Start on public interface
# Edit government/config.toml:
[network]
listen = "/ip4/0.0.0.0/tcp/9000"  # Listen on all interfaces

# 3. Start government
./run.sh start government

# 4. Get external IP and multiaddress
ip addr show | grep inet
./run.sh logs government | grep "Local node is listening"
# Note: /ip4/<EXTERNAL_IP>/tcp/9000/p2p/<PEER_ID>

# 5. Share realm configuration
./run.sh gov-info
# Share: realm_id and government_key
```

**Machine 2 (User Alice)**:
```bash
# 1. Clone repository
git clone https://github.com/MelzLabs/DeSync.git
cd DeSync

# 2. Build project
cargo build --release
chmod +x run.sh

# 3. Initialize with remote government config
./run.sh init-user alice <realm_id> <government_key>

# 4. Edit alice/config.toml
[bootstrap]
government = "/ip4/<GOVERNMENT_IP>/tcp/9000/p2p/<PEER_ID>"

# 5. Start alice
./run.sh start alice --gov
```

**Machine 3 (User Bob - connects to Alice)**:
```bash
# Similar to Alice, but add both connections:
[bootstrap]
government = "/ip4/<GOVERNMENT_IP>/tcp/9000/p2p/<GOV_PEER_ID>"
alice = "/ip4/<ALICE_IP>/tcp/<ALICE_PORT>/p2p/<ALICE_PEER_ID>"

./run.sh start bob --peer alice
```

**Firewall Configuration**:
```bash
# Government server
sudo ufw allow 9000/tcp

# User machines (if accepting peer connections)
sudo ufw allow <user_port>/tcp
```

### Backup and Recovery

**Comprehensive Backup Strategy**:

```bash
# Manual backup before major operations
./run.sh backup
# Creates: nodes/backup_20260206_143022.tar.gz

# Backup specific nodes
./run.sh backup government
./run.sh backup alice
```

**Disaster Recovery**:

```bash
# Scenario: Government node corrupted

# 1. Stop all nodes
./run.sh kill-all

# 2. Restore government from backup
./run.sh restore nodes/backup_government_20260206.tar.gz

# 3. Restart government
./run.sh start government

# 4. Users will auto-sync on next connection
./run.sh start alice --gov
./run.sh start bob --gov
```

**Partial Recovery (User Only)**:

```bash
# If user state is corrupted but government is fine

# 1. Stop user
./run.sh stop alice

# 2. Clean user data
./run.sh clean alice

# 3. Re-initialize (keeps realm config)
./run.sh init-user alice

# 4. Reconnect to sync
./run.sh start alice --gov
# User will re-download all government logs
```

**State Verification After Recovery**:

```bash
# Check government logs
./run.sh logs government

# Check user sync status
./run.sh logs alice

# Verify configuration consistency
cat government/config.toml alice/config.toml
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
├── README.md                     # This file
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

- **`src/bin/`**: Executable binaries for government and user nodes
- **`src/gov/`**: Government-specific logic (rule management, receipts)
- **`src/user/`**: User-specific logic (state management, tasks)
- **`src/network/`**: P2P networking using LibP2P
- **`src/storage/`**: Content-addressed storage and indexing
- **`src/types/`**: Shared data structures (logs, receipts, tasks)

**Runtime Directories** (created during operation):

- **`nodes/government/`**: Government node state and logs
- **`nodes/alice/`**, **`nodes/bob/`**: User node states
- **`nodes/backups/`**: Backup archives

---

## Development

### Building

**Release Build** (optimized):
```bash
cargo build --release
```

This creates the production binaries in `target/release/`.

---

## Roadmap

### Current Features (v0.1)
- Government node with rule management
- User nodes with state management
- Task-based economics with VDF
- Taxation system
- P2P transactions
- LibP2P networking
- Cryptographic verification
- CLI management wrapper

### Planned Features
- Proof of interaction for tasks
- Messaging capability
- Decentralized government nodes
- Government council (for task suggestions by users)
- Factions for users within realms

---

## Command Reference

For a complete command reference and detailed usage instructions, see the [Guide.md](Guide.md) file.

## Quick Command Cheatsheet

**Essential Commands**:
```bash
# Setup
./run.sh init-gov && ./run.sh start government
./run.sh update-gov-addr
./run.sh init-user alice && ./run.sh start alice --gov

# Government
./run.sh gov-set-tax 15
./run.sh gov-add-task farming 30 10 5 0
./run.sh gov-show-rules

# User
./run.sh task-start farming
./run.sh task-harvest farming
./run.sh pay-tax
./run.sh send alice bob 10

# Maintenance
./run.sh status <node>
./run.sh logs <node> 50
./run.sh backup && ./run.sh clean
./run.sh kill-all
```