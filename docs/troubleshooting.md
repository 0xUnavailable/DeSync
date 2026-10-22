# Troubleshooting

[← Back to README](../README.md)

## Common Issues

### Issue: "Port already in use"

**Symptoms**:

```
Error: Address already in use (os error 98)
Failed to start node on port 9000
```

**Solutions**:

```bash
# Option 1: Kill processes on the port
./run.sh kill-port 9000

# Option 2: Kill all DeSync nodes
./run.sh kill-all

# Option 3: Manual cleanup
lsof -ti :9000 | xargs kill -9
```

### Issue: "Multiple user nodes running"

**Symptoms**:

```
Error: Multiple user nodes are currently running
Cannot perform task operation
```

**Explanation**: Task operations (start, harvest, pay-tax) require exactly one user node.

**Solutions**:

```bash
# Stop other user nodes
./run.sh stop bob
./run.sh stop charlie

# Verify only one user running
./run.sh list
```

### Issue: "Realm configuration mismatch"

**Symptoms**:

```
Error: Peer rejected connection
Warning: Invalid government signature
```

**Explanation**: Nodes have different realm_id or government_key values.

**Solutions**:

```bash
# 1. Verify configuration matches
grep -A2 "\[realm\]" government/config.toml
grep -A2 "\[realm\]" alice/config.toml

# 2. If mismatched, re-initialize user
./run.sh clean alice
./run.sh init-user alice

# 3. Restart nodes
./run.sh restart alice
```

### Issue: "Task not found"

**Symptoms**:

```
Error: Task 'manufacturing' not found in government rules
```

**Explanation**: Task doesn't exist in government rules, or user's rule logs are out of sync with government.

**Solutions**:

```bash
# 1. Check government rules
./run.sh gov-show-rules

# 2. Restart user with government connection
./run.sh restart alice --gov

# 3. Try task operation again
./run.sh task-start farming
```

### Issue: "Insufficient resources"

**Symptoms**:

```
Error: Insufficient labor to start task (need 10, have 5)
```

**Explanation**: All resources are locked in active tasks.

**Solutions**:

```bash
# 1. Check active tasks
./run.sh logs alice | grep -i "active tasks"

# 2. Harvest completed tasks to free resources
./run.sh task-harvest farming

# 3. Wait for tasks to complete before starting new ones
```

## Debug Commands

**Check Network Connectivity**:

```bash
# View node logs for connection status
./run.sh logs alice

# Check node's multiaddress in TOML file
cat alice/config.toml
```

**Inspect Object Store**:

```bash
# List stored objects
ls -lah alice/objects/

# View object content (requires hash)
cat alice/objects/e9055adcca53ecaaceb68591bf533ab056ff76869388cc54aef8fe72134a26cc
```

**Monitor Real-Time Activity**:

```bash
# Tail logs for all nodes
tail -f government/node.log alice/node.log bob/node.log
```

## Configuration Problems

### Missing Government Address

**Problem**: Users can't connect to government.

**Solution**:

```bash
# 1. Ensure government is running
./run.sh status government

# 2. Update government address
./run.sh update-gov-addr

# 3. Restart user nodes
./run.sh restart alice
```

### Corrupt State Files

**Problem**: Node fails to start with state loading errors.

**Solution**:

```bash
# 1. Backup current state
./run.sh backup alice

# 2. Clean and re-initialize
./run.sh clean alice
./run.sh init-user alice

# 3. Restore will re-sync from network
./run.sh start alice --gov
```

### Orphaned PID Files

**Problem**: Script thinks node is running but it's not.

**Solution**:

```bash
# 1. Clean all PID files
./run.sh kill-all

# 2. Manually remove if needed
rm government/*.pid alice/*.pid bob/*.pid

# 3. Restart nodes
./run.sh start government
```

---

**See also:** [Configuration](configuration.md) · [Usage](usage.md) · [Backup and Recovery](advanced.md#backup-and-recovery)
