# Advanced Topics

[← Back to README](../README.md)

## Custom Task Economies

You can design complex economic systems with creative task definitions.

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

## Multi-Machine Deployment

**Important Note**: Multi-PC workflows require manual TOML configuration for each node, as the automatic multiaddress detection only works for single-PC multi-node operations. Multi-PC deployments have not been tested (because I only have one PC, and yes, that's actually the reason).

Running government and users on different machines:

**Machine 1 (Government Server)**:

```bash
# 1. Initialize government
./run.sh init-gov

# 2. Start on public interface
# Edit government/config.toml:
#   [network]
#   listen = "/ip4/0.0.0.0/tcp/9000"  # Listen on all interfaces

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
git clone https://github.com/0xUnavailable/DeSync.git
cd DeSync

# 2. Build project
cargo build --release
chmod +x run.sh

# 3. Initialize with remote government config
./run.sh init-user alice <realm_id> <government_key>

# 4. Edit alice/config.toml
#   [bootstrap]
#   government = "/ip4/<GOVERNMENT_IP>/tcp/9000/p2p/<PEER_ID>"

# 5. Start alice
./run.sh start alice --gov
```

**Machine 3 (User Bob - connects to Alice)**:

```bash
# Similar to Alice, but add both connections in bob/config.toml:
#   [bootstrap]
#   government = "/ip4/<GOVERNMENT_IP>/tcp/9000/p2p/<GOV_PEER_ID>"
#   alice = "/ip4/<ALICE_IP>/tcp/<ALICE_PORT>/p2p/<ALICE_PEER_ID>"

./run.sh start bob --peer alice
```

**Firewall Configuration**:

```bash
# Government server
sudo ufw allow 9000/tcp

# User machines (if accepting peer connections)
sudo ufw allow <user_port>/tcp
```

## Backup and Recovery

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

**See also:** [Configuration](configuration.md) · [Troubleshooting](troubleshooting.md) · [Usage](usage.md)
