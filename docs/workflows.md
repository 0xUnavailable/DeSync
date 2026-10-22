# Common Workflows

[← Back to README](../README.md)

## Setting Up a Multi-User Network

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

## Economic Activities

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

## Policy Updates

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

**See also:** [Usage](usage.md) · [Command Reference](commands.md) · [Networking](networking.md)
