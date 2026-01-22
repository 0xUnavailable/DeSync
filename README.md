# DeSync — Decentralized Economic Simulation Protocol

## Overview

**DeSync** is a decentralized economic simulation protocol where users participate in a persistent, peer-to-peer virtual economy. In DeSync, players manage resources, operate farms, and interact with other players, all under a single authoritative framework provided by the government node.

Unlike traditional simulations, DeSync is **serverless for most operations**, with no centralized authority controlling the world state. Instead, the system relies on cryptographic proofs, verifiable events, and a distributed protocol to ensure **consistency, fairness, and tamper-proof state evolution**.

---

## Core Philosophy

1. **Decentralization with Authority Anchors**

   * Users maintain their own full state locally.
   * The government node exists as the **rule authority and state anchor**, not a server that constantly mediates the economy.
   * Users can interact directly with one another in a peer-to-peer network without involving the government, except for **tax payments and rule updates**.

2. **Cryptographic Guarantees**

   * Every user action, from planting crops to transferring resources, is cryptographically signed.
   * User states are hash-chained, making **forking or tampering with history impossible**.
   * Transactions between users require **mutual signatures**, creating verifiable, atomic transfers.

3. **Protocol-Defined Time**

   * Time is not measured by clocks or hardware.
   * All time-sensitive actions (e.g., crop growth) are validated using **computational proofs** (Verifiable Delay Functions) based on action history.
   * This ensures fairness regardless of a user's system speed or time manipulation.

4. **Resilience & Offline Operation**

   * Users can operate offline, perform actions, and later synchronize with the network.
   * The government node is only required during tax validation.
   * Updates and logs can propagate through peer-to-peer replication if users miss government updates.

---

## Roles

### Government Node

The **government** serves as the single source of truth for rules, deadlines, and economic oversight. Its responsibilities include:

* **Rule Management:** Maintains a signed log of all economic rules (e.g., crop growth times, energy costs, tax rates, and task definitions).
* **Tax Validation:** Confirms the legitimacy of user state submissions and issues receipts to anchor history.
* **Treasury Control:** Tracks tax collection and manages government resources.
* **Event Verification:** Ensures user actions comply with rules and energy constraints.

The government log is authoritative and versioned. Users **must synchronize with the latest log** to perform actions or pay taxes.

---

### User Node

**Users** are full participants in the decentralized economy:

* **Resource Management:** Plant, harvest, and sell resources like crops or wood.
* **Energy Accounting:** All actions consume energy, preventing infinite activity and ensuring economic balance.
* **Proof-of-Action:** Every action produces a cryptographic proof to prevent spam or invalid state manipulation.
* **Peer Interactions:** Trade resources and communicate with other users via a secure, peer-to-peer network.
* **State Maintenance:** Each user stores a local copy of their world state, including snapshots and recent events, ensuring continuity even when offline.

Users anchor their actions with **government receipts**, which prevent forks or invalid state submissions in future interactions.

---

## Economic Mechanics

DeSync implements a simplified but rigorous economic simulation:

1. **Plots and Crops:** Users are allocated plots of land where they can perform actions like planting and harvesting. Each plot maintains its own state and action history.
2. **Energy & Actions:** Every action consumes energy, ensuring users cannot infinitely accelerate their progress.
3. **Tasks & Deadlines:** The government can assign tasks with specific requirements, which appear in the user’s log.
4. **Taxes & Receipts:** Periodically, users submit state summaries to pay taxes. The government validates these submissions and issues signed receipts, which **anchor the user’s history and prevent tampering**.

---

## Time & Progression

Time in DeSync is **mathematically enforced, not clock-based**:

* Actions like crop growth are validated using a **computational proof of work** derived from prior events.
* Each action must be proven to have “matured” according to its required computational depth.
* Users cannot cheat by changing system clocks or performing actions too quickly; **the protocol itself enforces temporal correctness**.

This approach makes all **time-sensitive actions tamper-proof and hardware-neutral**.

---

## Network Architecture

DeSync operates on a **peer-to-peer network**:

* **Partial Mesh with Gossipsub:** Most messages, like receipts and logs, propagate through a topic-based gossip mesh.
* **Direct Streams for Transfers:** High-value or atomic transactions (resource transfers) are sent over direct encrypted streams.
* **Bootstrap + DHT:** Users discover peers via bootstrap nodes and Kademlia-based distributed hash tables.
* **Offline Support:** Users can fetch missing logs from peers if they were offline during updates.

The network ensures **robust, decentralized propagation of state and government logs**, without requiring a constant central server.

---

## Security & Invariants

DeSync enforces multiple layers of security:

1. **Cryptographic Signatures:** All actions and state transitions are signed, ensuring authenticity.
2. **Hash-Chained States:** User state histories are sequentially linked, preventing forks or retroactive tampering.
3. **Mutual Transfer Signatures:** Resource exchanges require acknowledgment from both sender and receiver, preventing double-spending.
4. **Proof-of-Action:** Time-sensitive events use verifiable computational proofs (VDFs) to prevent clock manipulation.
5. **Government Log Versioning:** Users must act according to the latest government rules; outdated logs are automatically rejected.

These invariants guarantee that **no invalid state can exist**, even in a fully decentralized network.

---

## Offline & Recovery

* Users can operate offline and continue their economic activities.
* When reconnecting, users synchronize logs and receipts with peers or the government.
* Periodic snapshots of state prevent loss of progress and allow **replay of events** to rebuild user state.

---

## Key Concepts at a Glance

| Concept            | Description                                           |
| ------------------ | ----------------------------------------------------- |
| **Government Log** | Single authoritative source of rules and deadlines.   |
| **User State**     | Local snapshot + event chain, hash-linked and signed. |
| **VDF Proofs**     | Computational work that enforces action timing.       |
| **Receipts**       | Government-signed proofs anchoring user state.        |
| **P2P Transfers**  | Doubly-signed resource transfers with hash-chains.    |
| **Protocol Time**  | Derived from computational depth, not wall-clock.     |
| **Mesh Network**   | Partial mesh + gossip for reliable log propagation.   |

---

## Why DeSync is Different

* **Trustless Timing:** Users cannot cheat on time-sensitive actions.
* **Offline-First:** Most operations do not require central authority.
* **Tamper-Proof History:** All states are cryptographically anchored.
* **P2P Resource Exchange:** Economy is user-driven, resilient, and scalable.
* **Simplified Government Role:** Central authority provides rules and anchors history without controlling daily operations.

---

## Conclusion

DeSync is a **secure, decentralized, deterministic economic simulation** where players interact in a peer-to-peer economy under an authoritative but lightweight government log. Its combination of **hash-chained states, computational proofs, and cryptographic receipts** ensures fairness, anti-cheating, and resilience.

This README serves as the **entry point for developers and new users**, providing a clear understanding of the protocol, its mechanics, and core principles, without diving into implementation details.

---
