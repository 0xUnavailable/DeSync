# Command Reference

[← Back to README](../README.md)

A quick-lookup table of every `run.sh` command. For explanations of what each command does, see [Usage](usage.md). The repository's [Guide.md](../Guide.md) has more detailed instructions.

## Setup Commands

| Command            | Description                    | Example                    |
|--------------------|--------------------------------|----------------------------|
| `init-gov`         | Initialize government node     | `./run.sh init-gov`        |
| `init-user <name>` | Initialize user node           | `./run.sh init-user alice` |
| `update-gov-addr`  | Update government multiaddress | `./run.sh update-gov-addr` |

## Node Management

| Command                      | Description                        | Example                           |
|------------------------------|------------------------------------|-----------------------------------|
| `start <node>`               | Start a node                       | `./run.sh start government`       |
| `start <node> --gov`         | Start user connected to government | `./run.sh start alice --gov`      |
| `start <node> --peer <name>` | Start user connected to peer       | `./run.sh start bob --peer alice` |
| `stop <node>`                | Stop a running node                | `./run.sh stop alice`             |
| `restart <node>`             | Restart a node                     | `./run.sh restart alice`          |
| `status <node>`              | Show node status and PID           | `./run.sh status alice`           |
| `logs <node> [lines]`        | View recent logs                   | `./run.sh logs government 50`     |
| `list`                       | List all nodes and their status    | `./run.sh list`                   |

## Government Commands

| Command                                           | Description                                 | Example                                            |
|---------------------------------------------------|---------------------------------------------|----------------------------------------------------|
| `gov-info`                                        | Show realm identity and genesis rules       | `./run.sh gov-info`                                |
| `gov-show-rules`                                  | Display current economic rules with version | `./run.sh gov-show-rules`                          |
| `gov-set-tax <amount>`                            | Set tax amount (creates new rule log)       | `./run.sh gov-set-tax 15`                          |
| `gov-add-task <name> <mult> <land> <labor> <cap>` | Add new task definition                     | `./run.sh gov-add-task trading 50 10 5 2`          |
| `gov-modify-task <name> [opts]`                   | Modify task parameters                      | `./run.sh gov-modify-task farming --multiplier 40` |
| `gov-remove-task <name>`                          | Remove a task from rules                    | `./run.sh gov-remove-task obsolete`                |

**Task Modification Options**:

- `--multiplier N` - Set coin reward multiplier
- `--land N` - Set land resource requirement
- `--labor N` - Set labor resource requirement
- `--capital N` - Set capital resource requirement

## User Commands

| Command                              | Description                         | Requires Gov?    | Requires Peer? |
|--------------------------------------|-------------------------------------|------------------|----------------|
| `task-start <task>`                  | Start a task (locks resources)      | No*              | No             |
| `task-harvest <task>`                | Harvest completed task (earn coins) | No*              | No             |
| `pay-tax`                            | Submit tax obligation to government | **Yes (Direct)** | No             |
| `send <sender> <recipient> <amount>` | P2P coin transfer                   | **No**           | **Yes**        |

*Requires latest government logs (can be obtained from peer or government for log syncing)

## Maintenance Commands

| Command            | Description                 | Example                                |
|--------------------|-----------------------------|----------------------------------------|
| `clean [node]`     | Clean node data directory   | `./run.sh clean alice`                 |
| `backup [node]`    | Backup node(s) to ./nodes/  | `./run.sh backup government`           |
| `restore <file>`   | Restore from backup archive | `./run.sh restore nodes/backup.tar.gz` |
| `kill-port <port>` | Kill all processes on port  | `./run.sh kill-port 9000`              |
| `kill-all`         | Terminate all realm nodes   | `./run.sh kill-all`                    |

---

## Quick Command Cheatsheet

**Essential Commands**:

```bash
# Setup
./run.sh init-gov && ./run.sh start government
./run.sh update-gov-addr
./run.sh init-user alice && ./run.sh start alice --gov

# Government
./run.sh gov-set-tax 15
./run.sh gov-add-task farming 30 10 5 0
./run.sh gov-show-rules

# User
./run.sh task-start farming
./run.sh task-harvest farming
./run.sh pay-tax
./run.sh send alice bob 10

# Maintenance
./run.sh status <node>
./run.sh logs <node> 50
./run.sh backup && ./run.sh clean
./run.sh kill-all
```

---

**See also:** [Usage](usage.md) · [Common Workflows](workflows.md)
