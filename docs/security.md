# Security Model

[← Back to README](../README.md)

## Cryptographic Foundations

DeSync uses industry-standard cryptography.

### Ed25519 Digital Signatures

- **Algorithm**: EdDSA (Edwards-curve Digital Signature Algorithm)
- **Key Size**: 256 bits (32 bytes)
- **Signature Size**: 512 bits (64 bytes)
- **Security**: Quantum-resistant design, fast verification

**Used For**:

- Government rule log signatures
- User tax obligation signatures
- Government tax receipt signatures
- P2P transfer signatures

### Blake3 Hashing

- **Algorithm**: Blake3 (latest BLAKE family)
- **Output**: 256 bits (32 bytes)
- **Performance**: Extremely fast (multiple GB/s)
- **Security**: Collision-resistant, preimage-resistant

**Used For**:

- Content addressing (object storage)
- Log hash chains
- Data integrity verification
- Object identifiers

## Signature Verification

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

## State Integrity

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

**See also:** [Core Concepts](concepts.md#cryptographic-guarantees) · [Networking](networking.md) · [Architecture](architecture.md)
