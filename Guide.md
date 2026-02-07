# Quick Reference Guide - run.sh Commands

## Setup & Node Management

| Command | Description | Example |
|---------|-------------|---------|
| `./run.sh init-gov` | Initialize government node | `./run.sh init-gov` |
| `./run.sh init-user <name>` | Initialize user node | `./run.sh init-user alice` |
| `./run.sh update-gov-addr` | Update government multiaddress | `./run.sh update-gov-addr` |
| `./run.sh start <node>` | Start a node | `./run.sh start government` |
| `./run.sh start <user> --gov` | Start user connected to government | `./run.sh start alice --gov` |
| `./run.sh start <user> --peer <name>` | Start user connected to peer | `./run.sh start bob --peer alice` |
| `./run.sh stop <node>` | Stop a node | `./run.sh stop alice` |
| `./run.sh restart <node>` | Restart a node | `./run.sh restart alice` |
| `./run.sh status <node>` | Show node status | `./run.sh status alice` |
| `./run.sh logs <node> [lines]` | View node logs | `./run.sh logs government 50` |
| `./run.sh list` | List all nodes | `./run.sh list` |

---

## GOVERNMENT COMMANDS

### Realm Identity & Static Information
| Command | Description | Purpose | Example |
|---------|-------------|---------|---------|
| `./run.sh gov-info` | Show realm identity and genesis rules | **Static info only** - Realm ID, Public Key, original genesis rules | `./run.sh gov-info` |

### Dynamic Economic State
| Command | Description | Purpose | Example |
|---------|-------------|---------|---------|
| `./run.sh gov-show-rules` | Display current economic rules | **Dynamic state** - Latest version rules with version tracking | `./run.sh gov-show-rules` |
| `./run.sh gov-set-tax <amount>` | Set tax amount (in coins) | Update economic policy | `./run.sh gov-set-tax 15` |

### Task Management
| Command | Description | Example |
|---------|-------------|---------|
| `./run.sh gov-add-task <name> <multiplier> <land> <labor> <capital>` | Add new task | `./run.sh gov-add-task trading 50 10 5 2` |
| `./run.sh gov-modify-task <name> [options]` | Modify task | `./run.sh gov-modify-task farming --multiplier 40` |
| `./run.sh gov-remove-task <name>` | Remove task | `./run.sh gov-remove-task obsolete` |

#### Task Modification Options:
- `--multiplier N` - Set coin multiplier (determines earnings)
- `--land N` - Set land requirement
- `--labor N` - Set labor requirement
- `--capital N` - Set capital requirement

---

## USER COMMANDS

### Important: User Activity Requirements
User activities (task-start, task-harvest, pay-tax) require:
1. **Only one running user node** at a time
2. **Latest government logs** (can be from peer or government)
3. **Government connection only required for**:
   - Submitting tax obligation
   - Initial log synchronization

| Command | Description | Example | Requires Government | Requires P2P Connection |
|---------|-------------|---------|---------------------|-------------------------|
| `./run.sh task-start <task>` | Start a task | `./run.sh task-start farming` | No (needs latest logs) | No |
| `./run.sh task-harvest <task>` | Harvest task | `./run.sh task-harvest farming` | No (needs latest logs) | No |
| `./run.sh pay-tax` | Submit tax obligation | `./run.sh pay-tax` | Yes (must connect) | No |
| `./run.sh send <sender> <recipient> <amount>` | Send coins P2P | `./run.sh send alice bob 25` | **No** (government not required) | **Yes** (between users) |

### P2P Send Command Details:
- **Format**: `./run.sh send <sender> <recipient> <amount>`
- **Example**: `./run.sh send alice bob 25`
  - **Sender**: alice (must be running)
  - **Recipient**: bob (must exist in network)
  - **Amount**: 25 coins

---

## CONFIGURATION REQUIREMENTS

### Universal Configuration Requirements
**CRITICAL**: All nodes (government and users) must have identical realm configuration:

```toml
# Example alice config.toml
[node]
type = "user"
name = "alice"
data_dir = "."

[realm]
realm_id = "e9055adcca53ecaaceb68591bf533ab056ff76869388cc54aef8fe72134a26cc"
government_key = "99e04793744d78d1b832a3f074377909561463e24b29fe3c40726afe76b3de97"

[network]
listen = "/ip4/0.0.0.0/tcp/0"

[network_info]
multiaddress = "/ip4/172.17.0.1/tcp/42593/p2p/12D3KooWQn7Ry3Uuh2K7P9h8cwf764WQgi4nWutz1EgnVfYwbAMA"

[bootstrap]
bob = "/ip4/172.17.0.1/tcp/35227/p2p/12D3KooWEpnnKz3hYUc169oZi2rHpNSPZP6a2xid2eWC5bz1xBzg"
government = "/ip4/172.17.0.1/tcp/9000/p2p/12D3KooWLB2uBo8PQVBmVqbFvbH2Z2NoTbVpyPCSCoZJr9eRwQyU"

[identity]
user_key = "de4b7a7e15ac87c0672baae7f7dd028a8b1fb796218a559eb36ec3b7b1242e65"
```

### Mandatory Shared Values:
1. **`realm_id`**: Must match exactly across all nodes
2. **`government_key`**: Must match exactly across all nodes
3. **Government multiaddress**: Must be correct in all user `[bootstrap]` sections

### Unique Per-Node Values:
1. **`name`**: Node identifier (alice, bob, government)
2. **`multiaddress`**: Node's own network address (auto-populated)
3. **`user_key`**: User's unique identity key
4. **Peer addresses** in `[bootstrap]`: Other nodes' addresses

---

## MAINTENANCE COMMANDS

| Command | Description | Example |
|---------|-------------|---------|
| `./run.sh clean [node]` | Clean node data | `./run.sh clean alice` |
| `./run.sh backup [node]` | Backup nodes to ./nodes/ | `./run.sh backup government` |
| `./run.sh restore <file>` | Restore from backup | `./run.sh restore nodes/backup.tar.gz` |
| `./run.sh kill-port <port>` | Kill processes on port | `./run.sh kill-port 9000` |
| `./run.sh kill-all` | Kill all realm nodes | `./run.sh kill-all` |

---

## COMMON WORKFLOWS

### Initial Setup
```bash
# 1. Initialize government (creates realm_id and government_key)
./run.sh init-gov

# 2. Start government to get its multiaddress
./run.sh start government
./run.sh update-gov-addr

# 3. Initialize users (automatically copies realm configuration)
./run.sh init-user alice
./run.sh init-user bob

# All nodes now have same realm_id and government_key
```

### P2P Network Setup
```bash
# 1. Start alice with government connection
./run.sh start alice --gov

# 2. Start bob connected to alice (first time: edit TOML)
# Edit bob/config.toml [bootstrap] section to add alice's address
./run.sh start bob --peer alice

# 3. Verify configuration matches
# alice and bob must have identical [realm] section values
```

### Configuration Verification
Check that all nodes share these identical values:
```bash
# In alice/config.toml:
realm_id = "e9055adcca53ecaaceb68591bf533ab056ff76869388cc54aef8fe72134a26cc"
government_key = "99e04793744d78d1b832a3f074377909561463e24b29fe3c40726afe76b3de97"

# In bob/config.toml (MUST MATCH):
realm_id = "e9055adcca53ecaaceb68591bf533ab056ff76869388cc54aef8fe72134a26cc"
government_key = "99e04793744d78d1b832a3f074377909561463e24b29fe3c40726afe76b3de97"

# In government/config.toml (MUST MATCH):
realm_id = "e9055adcca53ecaaceb68591bf533ab056ff76869388cc54aef8fe72134a26cc"
government_key = "99e04793744d78d1b832a3f074377909561463e24b29fe3c40726afe76b3de97"
```

### P2P Transactions
```bash
# Requirements:
# 1. All nodes have identical realm_id and government_key
# 2. At least one --peer connection between users
# 3. Government node NOT required for P2P

# Setup with correct configuration
./run.sh start alice
./run.sh start bob --peer alice

# Send coins (government can be offline)
./run.sh send alice bob 25
```

---

## INFORMATION ARCHITECTURE

### Static vs Dynamic Information
| Command | Information Type | Purpose | Version Awareness |
|---------|-----------------|---------|-------------------|
| `gov-info` | **Static Identity** | Realm ID, Public Key, Genesis rules | None - fixed at initialization |
| `gov-show-rules` | **Dynamic State** | Current economic rules | Yes - shows latest version |
| User commands | **Dynamic State** | Task execution, tax payment | Yes - uses latest logs |

---

## ERROR MESSAGES

| Error | Solution |
|-------------|----------|
| "Multiple user nodes running" | Stop all but one user node |
| "Peer not found" | First time: edit TOML, then use --peer flag |
| "Task not found in government rules" | Check latest rules with `./run.sh gov-show-rules` |
| "Realm configuration mismatch" | Ensure all nodes have identical realm_id and government_key |

---

## IMPORTANT RULES

### Configuration Rules:
1. **Identical Realm Configuration**: All nodes must have exactly the same `realm_id` and `government_key`
2. **Unique Node Identity**: Each node has unique `name`, `user_key`, and `multiaddress`
3. **Shared Government Address**: All users must have correct government multiaddress in `[bootstrap]`

### Network Rules:
1. **Single Running User**: Only one user node running for task-start, task-harvest, pay-tax
2. **Log Source Flexibility**: Latest logs can come from peer OR government
3. **Government Connection Required For**:
   - Tax obligation submission
   - Initial log sync (can then propagate via peers)
4. **Government Connection NOT Required For**:
   - P2P transactions (send command)
   - Task execution once logs are synced

### P2P Transaction Rules:
1. **Format**: `./run.sh send <sender> <recipient> <amount>`
2. **Government**: NOT required - can be offline
3. **Connection**: Users must be connected via --peer or bootstrap
4. **Configuration**: All nodes must share same realm_id and government_key

### Configuration Synchronization Flow:
```
Government (init) → Generates realm_id & government_key
        ↓
User Init → Copies realm configuration
        ↓
All Nodes → Identical realm_id & government_key
        ↓
Network Works → Valid transactions
```

**Critical**: If `realm_id` or `government_key` differs between nodes, the network will fail. Always initialize users from the same government node.