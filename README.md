# DeSync

**A Decentralized Economic Realm with Blockchain-Based Governance and Peer-to-Peer Transactions**

DeSync is a decentralized economic simulation system built with Rust, featuring blockchain-based governance, peer-to-peer transactions, and a task-based economic model. The system creates isolated "realms" where a government node defines economic rules, and user nodes participate in economic activities, pay taxes, and transact directly with each other.

At its core, DeSync demonstrates:

- **Decentralized Governance**: A government node establishes and updates economic policies through cryptographically signed rule logs
- **Peer-to-Peer Economy**: Users can engage in economic activities and transfer value without constant government oversight
- **Blockchain-Inspired Design**: Immutable logs with hash chains and cryptographic signatures ensure data integrity
- **Resource-Based Economics**: Task completion requires resource allocation (land, labor, capital) and generates rewards
- **Flexible Network Topology**: Support for direct government connections or peer-to-peer relay networks

## What Makes DeSync Unique?

DeSync implements a **hybrid consensus model** where:

- Government nodes have **policy authority** (they can change rules)
- User nodes have **economic autonomy** (they control their activities)
- The network operates with **eventual consistency** (changes propagate through the network)
- Transactions are **cryptographically verified** but don't require mining or proof-of-work

This creates an interesting middle ground between centralized control and full decentralization, suitable for simulations, experimental economics, local community currencies, or educational demonstrations of blockchain concepts.

## Key Features

- **Decentralized governance**: realms with unique identities, dynamic policy updates, versioned and signed rule logs
- **Task-based economics**: custom tasks with land/labor/capital requirements, configurable multipliers, VDF-based harvesting
- **Cryptographic security**: Ed25519 signatures, Blake3 hashing, verifiable receipts, content-addressed storage
- **Flexible networking**: LibP2P peer-to-peer communication, direct or relay topologies, automatic log sync
- **State management**: deterministic state computed from logs, resource and task tracking
- **Developer-friendly**: a CLI wrapper (`run.sh`), built-in backup and restore, extensive logging

## Quick Start

> The workflows in these docs assume multiple nodes running on the same PC. Multi-PC deployments have not been tested.

```bash
# Build (requires Rust 1.70+)
git clone https://github.com/0xUnavailable/DeSync.git
cd DeSync
cargo build --release
chmod +x run.sh

# Start a government node and a user
./run.sh init-gov
./run.sh start government
./run.sh update-gov-addr
./run.sh init-user alice
./run.sh start alice --gov

# Earn and pay
./run.sh task-start farming
./run.sh task-harvest farming
./run.sh pay-tax
```

For the full walkthrough, including peer-to-peer setup, see [Getting Started](docs/getting-started.md).

## Documentation

| Document | What's in it |
|---|---|
| [Getting Started](docs/getting-started.md) | Prerequisites, building from source, and a full quick-start walkthrough |
| [Architecture](docs/architecture.md) | System layers, node types, data structures, and project layout |
| [Core Concepts](docs/concepts.md) | Realms, the economic model, and cryptographic guarantees |
| [Configuration](docs/configuration.md) | Realm and network configuration, and keeping nodes in sync |
| [Usage](docs/usage.md) | Government, user, and maintenance operations |
| [Command Reference](docs/commands.md) | Every `run.sh` command in table form, plus a cheatsheet |
| [Networking](docs/networking.md) | Topologies, log synchronization, and P2P transactions |
| [Security Model](docs/security.md) | Cryptographic foundations, verification, and integrity limits |
| [Common Workflows](docs/workflows.md) | Multi-user setup, economic activities, and policy updates |
| [Troubleshooting](docs/troubleshooting.md) | Common issues, debug commands, and configuration problems |
| [Advanced Topics](docs/advanced.md) | Custom economies, multi-machine deployment, and backup/recovery |
| [Guide.md](Guide.md) | Detailed command reference and usage instructions |

## Development

```bash
cargo build --release
```

This creates the production binaries (`government`, `user`, `load_genesis`) in `target/release/`.

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
