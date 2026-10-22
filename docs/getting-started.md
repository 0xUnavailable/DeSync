# Getting Started

[← Back to README](../README.md)

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
   git clone https://github.com/0xUnavailable/DeSync.git
   cd DeSync
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

```toml
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

**Next:** [Architecture](architecture.md) · [Configuration](configuration.md) · [Usage](usage.md)
