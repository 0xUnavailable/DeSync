# Core Concepts

[← Back to README](../README.md)

## Realms

A **realm** is an isolated economic system with its own government, rules, and participants. Each realm is identified by a unique `realm_id` (a 32-byte hash) that is generated when the government node is initialized.

**Key Properties of Realms:**

1. **Unique Identity**: Each realm has a cryptographically unique identifier
2. **Single Government**: One government node per realm (identified by public key)
3. **Multiple Users**: Unlimited number of user participants
4. **Shared Configuration**: All nodes must agree on `realm_id` and `government_key`
5. **Isolated Economics**: Realms are completely independent of each other

**Why Realms?** Realms allow multiple independent economic experiments to coexist. You might run:

- A test realm with experimental policies
- A production realm with stable rules
- Multiple competitive realms with different economic models

## Economic Model

DeSync implements a **resource-based task economy**.

### Resources

Users have three types of resources:

- **Land**: Required for agricultural and extraction tasks
- **Labor**: Required for all tasks (universal resource)
- **Capital**: Required for manufacturing and trading tasks

Resources are:

- **Allocated** when starting a task (locked until harvest)
- **Released** when harvesting a task
- **Conserved** (total resources don't change, just move between allocated/available)

### Tasks

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

### Taxation

The government sets a tax rate (in coins):

- Users self-report their tax obligation
- Government issues signed receipts upon receipt
- Tax receipts can be used to prove compliance
- No enforcement mechanism (participation is voluntary)

### Coin Flow

```
Tasks → User Earns Coins → User Pays Tax → Coins to Government
                        ↓
                  User Sends to Peer → Peer Receives Coins
```

Coins are created through task completion and flow through the economy via taxation and P2P transfers.

## Cryptographic Guarantees

DeSync uses cryptography to ensure data integrity without centralized validation.

### Hash Chains

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

### Digital Signatures

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

### Content Addressing

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

**See also:** [Security Model](security.md) · [Architecture](architecture.md) · [Configuration](configuration.md)
