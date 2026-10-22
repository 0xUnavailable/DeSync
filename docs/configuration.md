# Configuration

[← Back to README](../README.md)

## Realm Configuration

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

## Network Configuration

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

## Configuration Synchronization

**Automatic Synchronization (Recommended)**: When you run `./run.sh init-user alice`, the script automatically:

1. Reads government's `realm_id` and `government_key`
2. Copies them to alice's configuration
3. Generates alice's unique user key
4. Ensures configuration consistency

**Manual Synchronization (Advanced)**: If setting up on different machines:

```bash
# On government machine
./run.sh gov-info  # Note the realm_id and government_key

# On user machine
./run.sh init-user alice <realm_id> <government_key>
```

---

**See also:** [Getting Started](getting-started.md) · [Troubleshooting](troubleshooting.md) · [Multi-Machine Deployment](advanced.md#multi-machine-deployment)
