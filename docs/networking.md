# Networking

[← Back to README](../README.md)

## Connection Topology

DeSync supports multiple network topologies.

### Star Topology (Government-Centric)

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

### Mesh Topology (Peer-to-Peer)

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

### Hybrid Topology

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

## Log Synchronization

Government rule logs propagate through the network.

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

## P2P Transactions

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

**See also:** [Security Model](security.md) · [Configuration](configuration.md) · [Common Workflows](workflows.md)
