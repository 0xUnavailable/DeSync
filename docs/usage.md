# Usage

[← Back to README](../README.md)

For a lookup table of every command, see the [Command Reference](commands.md).

## Government Operations

### Setting Tax Rates

```bash
./run.sh gov-set-tax <amount>
```

Example:

```bash
# Set tax to 20 coins
./run.sh gov-set-tax 20

# This creates a new government rule log with an incremented version number
# Users will sync this log and pay the new tax rate
```

### Managing Tasks

**Note:** Any modification to economic rules (adding, modifying, or removing tasks, or changing tax rates) creates a new Government Rule Log with an incremented version number. This ensures a complete audit trail of all policy changes.

**Add a New Task**:

```bash
./run.sh gov-add-task <name> <multiplier> <land> <labor> <capital>
```

Example:

```bash
# Add a new "research" task
./run.sh gov-add-task research 80 2 15 20
```

**Modify Existing Task**:

```bash
./run.sh gov-modify-task <name> [options]
```

Options:

- `--multiplier N` - Change coin reward
- `--land N` - Change land requirement
- `--labor N` - Change labor requirement
- `--capital N` - Change capital requirement

Example:

```bash
# Make farming more profitable
./run.sh gov-modify-task farming --multiplier 45

# Change multiple parameters
./run.sh gov-modify-task mining --multiplier 50 --labor 12
```

**Remove a Task**:

```bash
./run.sh gov-remove-task <name>
```

Example:

```bash
# Remove obsolete task
./run.sh gov-remove-task oldtask
```

### Viewing Current Rules

```bash
./run.sh gov-show-rules
```

Output:

```
=== Current Government Rules (Version 3) ===
Tax Amount: 20 coins

Active Tasks:
  • farming: 45 coins (land: 10, labor: 5, capital: 0)
  • mining: 50 coins (land: 5, labor: 12, capital: 5)
  • trading: 50 coins (land: 0, labor: 5, capital: 15)
  • research: 80 coins (land: 2, labor: 15, capital: 20)
```

---

## User Operations

### Task Management

**Starting a Task**:

```bash
./run.sh task-start <task_name>
```

Example:

```bash
./run.sh task-start farming
```

This will:

1. Check if you have sufficient resources
2. Lock the required resources
3. Record the start time
4. Create a task entry in your state

**Harvesting a Task**:

```bash
./run.sh task-harvest <task_name>
```

Example:

```bash
./run.sh task-harvest farming
```

This will:

1. Execute the VDF (500,000 iterations, ~10-20 seconds on standard PC)
2. Verify the VDF computation completes successfully
3. Add coins to your balance (based on multiplier)
4. Release the locked resources
5. Mark the task as completed

### Tax Payment

```bash
./run.sh pay-tax
```

This command:

1. Syncs with government to get latest tax rate
2. Creates a tax obligation (signed by your private key)
3. Submits to government (requires direct government connection)
4. Receives a cryptographically signed receipt
5. Deducts coins from your balance

**Requirements**:

- **Must have direct government connection** (tax obligations can only be submitted directly to government, not via peers)
- Government connection is necessary for tax obligation submission only
- Log syncing can be done through peers if they have up-to-date logs
- Must have sufficient coins to pay tax
- Only one user node can be running

### Peer-to-Peer Transfers

```bash
./run.sh send <sender> <recipient> <amount>
```

Example:

```bash
# Alice sends 30 coins to Bob
./run.sh send alice bob 30
```

This performs:

1. Sender creates and signs a transfer receipt
2. Receipt is sent to recipient via P2P network
3. Both parties update their local state
4. No government involvement required

**Requirements**:

- Sender node must be running
- Recipient node must be reachable (peer connection)
- Sender must have sufficient coins
- Government does NOT need to be online

**Important**: The sender's private key signs the transaction, so only they can initiate transfers from their account.

---

## Maintenance Operations

### Node Management

**Start a Node**:

```bash
./run.sh start <node> [--gov|--peer <name>]
```

Options:

- No flag: Start standalone (no bootstrap connections)
- `--gov`: Connect to government node
- `--peer <name>`: Connect to a specific peer

**Stop a Node**:

```bash
./run.sh stop <node>
```

**Restart a Node**:

```bash
./run.sh restart <node>
```

**Check Node Status**:

```bash
./run.sh status <node>
```

**View Logs**:

```bash
./run.sh logs <node> [lines]
```

Example:

```bash
# View last 50 lines
./run.sh logs alice 50

# View last 20 lines (default)
./run.sh logs government
```

**List All Nodes**:

```bash
./run.sh list
```

### Data Management

**Backup Nodes**:

```bash
./run.sh backup [node]
```

Examples:

```bash
# Backup specific node
./run.sh backup alice

# Backup all nodes
./run.sh backup
```

Backups are stored in `./nodes/backup_<timestamp>.tar.gz`

**Restore from Backup**:

```bash
./run.sh restore <backup-file>
```

Example:

```bash
./run.sh restore nodes/backup_20260206_143022.tar.gz
```

**Clean Node Data**:

```bash
./run.sh clean [node]
```

Examples:

```bash
# Clean specific node
./run.sh clean alice

# Clean all nodes
./run.sh clean
```

**Warning**: This deletes all node data! Always backup first.

### Troubleshooting Tools

**Kill Processes on Port**:

```bash
./run.sh kill-port <port>
```

Example:

```bash
./run.sh kill-port 9000
```

**Kill All Nodes**:

```bash
./run.sh kill-all
```

This forcefully terminates all running DeSync nodes and cleans up PID files.

---

**See also:** [Command Reference](commands.md) · [Troubleshooting](troubleshooting.md) · [Backup and Recovery](advanced.md#backup-and-recovery)
