#!/bin/bash
# run.sh - Simplified task runner for realm nodes

set -e

NODES_DIR="nodes"
PROJECT_ROOT="$(cd "$(dirname "$0")" && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

function print_usage() {
    echo "Realm Node Management"
    echo ""
    echo "Usage: ./run.sh <command> [options]"
    echo ""
    echo "Setup Commands:"
    echo "  init-gov                                Initialize government node"
    echo "  init-user <name> [realm_id] [gov_key]   Initialize user node"
    echo "  update-gov-addr                         Update government multiaddress from logs"
    echo ""
    echo "Node Management:"
    echo "  start <node> [--gov|--peer <name>]      Start a node with optional bootstrap peer"
    echo "  stop <node>                             Stop a running node"
    echo "  restart <node>                          Restart a node"
    echo "  status <node>                           Show node status"
    echo "  logs <node> [lines]                     Show recent logs (default: 20 lines)"
    echo "  list                                    List all nodes and their status"
    echo ""
    echo "Government Operations:"
    echo "  gov-info                                Show government information and rules"
    echo "  gov-set-tax <amount>                    Set new tax amount"
    echo "  gov-add-task <name> <multiplier> <land> <labor> <capital>   Add new task rule"
    echo "  gov-modify-task <name> [--multiplier N] [--land N] [--labor N] [--capital N]"
    echo "  gov-remove-task <name>                  Remove a task rule"
    echo "  gov-show-rules                          Show current economic rules"
    echo ""
    echo "User Operations:"
    echo "  task-start <task>                       Start a task (farming, mining, etc.)"
    echo "  task-harvest <task>                     Harvest a completed task"
    echo "  pay-tax                                 Submit tax obligation (uses current rate)"
    echo "  send <sender> <recipient> <amount>      Send coins from sender to recipient"
    echo ""
    echo "Maintenance:"
    echo "  clean [node]                            Clean specific node or all nodes"
    echo "  backup [node]                           Backup specific node or all nodes"
    echo "  restore <backup-file>                   Restore from backup (backups are in ./nodes/)"
    echo "  kill-port <port>                        Kill all processes on a specific port"
    echo "  kill-all                                Kill all running nodes and clean PID files"
    echo ""
    echo "Examples:"
    echo "  # Same machine setup:"
    echo "  ./run.sh init-gov"
    echo "  ./run.sh start government"
    echo "  ./run.sh update-gov-addr                   # Capture multiaddress"
    echo "  ./run.sh init-user alice"
    echo "  ./run.sh start alice --gov"
    echo ""
    echo "  # Government operations:"
    echo "  ./run.sh gov-set-tax 15                    # Set tax to 15 coins"
    echo "  ./run.sh gov-add-task trading 50 10 5 2    # multiplier=50"
    echo "  ./run.sh gov-modify-task farming --multiplier 40 # Update farming to 40 coins"
    echo "  ./run.sh gov-show-rules                    # View all rules"
    echo ""
    echo "  # User operations (run from user's perspective):"
    echo "  ./run.sh task-start farming                # Start farming"
    echo "  ./run.sh pay-tax                           # Pay tax (auto-calculates)"
    echo "  ./run.sh send alice bob 5                  # Send 5 coins from alice to bob"
    echo ""
    echo "  # Troubleshooting:"
    echo "  ./run.sh kill-port 9000                    # Clear port 9000"
    echo "  ./run.sh kill-all                          # Kill all nodes"
}

function ensure_project_root() {
    cd "$PROJECT_ROOT"
}

function check_port_available() {
    local port=$1
    local process_name=$2
    
    echo -e "${BLUE}Checking if port $port is available...${NC}"
    
    if command -v lsof >/dev/null 2>&1; then
        if lsof -i ":$port" >/dev/null 2>&1; then
            echo -e "${YELLOW}Port $port is already in use!${NC}"
            echo "Processes using port $port:"
            lsof -i ":$port"
            return 1
        fi
    elif command -v netstat >/dev/null 2>&1; then
        if netstat -tuln | grep ":$port " >/dev/null 2>&1; then
            echo -e "${YELLOW}Port $port is already in use!${NC}"
            echo "Processes using port $port:"
            netstat -tuln -p 2>/dev/null | grep ":$port " || true
            return 1
        fi
    elif command -v ss >/dev/null 2>&1; then
        if ss -tuln | grep ":$port " >/dev/null 2>&1; then
            echo -e "${YELLOW}Port $port is already in use!${NC}"
            echo "Processes using port $port:"
            ss -tuln -p 2>/dev/null | grep ":$port " || true
            return 1
        fi
    fi
    
    echo -e "${GREEN}Port $port is available${NC}"
    return 0
}

function load_node_config() {
    local node=$1
    local node_dir="$PROJECT_ROOT/$NODES_DIR/$node"
    
    if [ ! -d "$node_dir" ]; then
        echo -e "${RED}Error: Node '$node' not found in $NODES_DIR${NC}"
        echo "Available nodes:"
        ls -1 "$PROJECT_ROOT/$NODES_DIR" 2>/dev/null || echo "  (none)"
        exit 1
    fi
    
    echo "$node_dir"
}

function get_node_type() {
    local node_dir=$1
    if [ -f "$node_dir/config.toml" ]; then
        grep 'type = ' "$node_dir/config.toml" | cut -d'"' -f2
    elif [ -f "$node_dir/gov_keys/state.json" ]; then
        echo "government"
    elif [ -f "$node_dir/user_state.json" ]; then
        echo "user"
    else
        echo "unknown"
    fi
}

function get_node_multiaddr() {
    local node=$1
    local node_dir="$PROJECT_ROOT/$NODES_DIR/$node"
    
    if [ -f "$node_dir/config.toml" ]; then
        grep 'multiaddress = ' "$node_dir/config.toml" 2>/dev/null | cut -d'"' -f2
    fi
}
function get_current_user_node() {
    # Try to determine current user node from context
    # Only consider RUNNING nodes
    local running_user_nodes=()
    
    # Debug: Show what we find
    # echo "DEBUG: Checking for running user nodes..."
    
    for node in "$PROJECT_ROOT/$NODES_DIR"/*; do
        if [ -d "$node" ]; then
            local node_type=$(get_node_type "$node")
            local node_name=$(basename "$node")
            
            if [ "$node_type" = "user" ]; then
                # Check if this user node is actually running
                local pid=""
                
                # Method 1: Check PID file first
                if [ -f "$node/$node_name.pid" ]; then
                    local file_pid=$(cat "$node/$node_name.pid" 2>/dev/null)
                    if [ -n "$file_pid" ] && kill -0 "$file_pid" 2>/dev/null; then
                        pid="$file_pid"
                        # echo "DEBUG: Found $node_name via PID file: $pid"
                    fi
                fi
                
                # Method 2: Search for process if PID file check failed
                if [ -z "$pid" ]; then
                    # More reliable process detection - look for cargo user run with this data-dir
                    # Use pgrep for more reliable process matching
                    pid=$(pgrep -f "user run.*--data-dir.*$node_name" || true)
                    
                    # Fallback: grep process list
                    if [ -z "$pid" ]; then
                        pid=$(ps aux | grep "cargo run --bin user" | grep "$node" | grep -v grep | awk '{print $2}' | head -1)
                    fi
                    
                    # if [ -n "$pid" ]; then
                    #     echo "DEBUG: Found $node_name via process search: $pid"
                    # fi
                fi
                
                if [ -n "$pid" ]; then
                    running_user_nodes+=("$node_name")
                fi
            fi
        fi
    done
    
    if [ ${#running_user_nodes[@]} -eq 1 ]; then
        echo "${running_user_nodes[0]}"
    elif [ ${#running_user_nodes[@]} -gt 1 ]; then
        # Multiple nodes running - return empty so we can show error
        echo ""
    else
        echo ""
    fi
}

function init_government() {
    ensure_project_root
    echo -e "${GREEN}Initializing government node...${NC}"
    
    mkdir -p "$NODES_DIR/government"
    cd "$NODES_DIR/government"
    
    # Run government init and capture output
    local init_output=$(cargo run --bin government -- init --keys-dir gov_keys 2>&1)
    echo "$init_output"
    
    # Parse realm_id and gov_key from output
    local realm_id=$(echo "$init_output" | grep "Realm ID:" | head -1 | awk '{print $NF}')
    local gov_key=$(echo "$init_output" | grep "Gov Key:" | head -1 | awk '{print $NF}')
    
    if [ -z "$realm_id" ] || [ -z "$gov_key" ]; then
        echo -e "${RED}Error: Failed to parse realm_id or gov_key from init output${NC}"
        exit 1
    fi
    
    # Create config file
    cat > config.toml << EOF
[node]
type = "government"
name = "government"
data_dir = "."

[network]
listen = "/ip4/0.0.0.0/tcp/9000"

[realm]
realm_id = "$realm_id"
government_key = "$gov_key"

[network_info]
multiaddress = ""
EOF
    
    echo ""
    echo -e "${GREEN}Government node initialized successfully!${NC}"
    echo ""
    echo -e "${BLUE}Configuration: $NODES_DIR/government/config.toml${NC}"
    echo -e "${BLUE}Realm ID:      $realm_id${NC}"
    echo -e "${BLUE}Gov Key:       $gov_key${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Start government:  ./run.sh start government"
    echo "  2. Get multiaddress:  ./run.sh update-gov-addr"
    echo "  3. Init users:        ./run.sh init-user alice"
    echo ""
    echo "For distributed setup, manually set multiaddress in config.toml"
}

function init_user() {
    ensure_project_root
    local name=$1
    local realm_id=$2
    local gov_key=$3
    
    if [ -z "$name" ]; then
        echo -e "${RED}Error: User name required${NC}"
        echo "Usage: ./run.sh init-user <name> [realm_id] [gov_key]"
        echo ""
        echo "Examples:"
        echo "  ./run.sh init-user alice <realm_id> <gov_key>      # Manual config"
        echo "  ./run.sh init-user alice                            # Read from government node"
        exit 1
    fi
    
    # If realm_id and gov_key not provided, try to read from government
    if [ -z "$realm_id" ] || [ -z "$gov_key" ]; then
        if [ -f "$NODES_DIR/government/config.toml" ]; then
            echo -e "${BLUE}Reading configuration from government node...${NC}"
            realm_id=$(grep 'realm_id = ' "$NODES_DIR/government/config.toml" | cut -d'"' -f2)
            gov_key=$(grep 'government_key = ' "$NODES_DIR/government/config.toml" | cut -d'"' -f2)
            
            if [ -z "$realm_id" ] || [ -z "$gov_key" ]; then
                echo -e "${RED}Error: Could not read government configuration${NC}"
                echo "Please provide realm_id and gov_key manually:"
                echo "  ./run.sh init-user $name <realm_id> <gov_key>"
                exit 1
            fi
        else
            echo -e "${RED}Error: Government node not found and realm_id/gov_key not provided${NC}"
            echo ""
            echo "Either:"
            echo "  1. Initialize government first: ./run.sh init-gov"
            echo "  2. Or provide parameters: ./run.sh init-user $name <realm_id> <gov_key>"
            exit 1
        fi
    fi
    
    echo -e "${GREEN}Initializing user node: $name${NC}"
    echo -e "${BLUE}Realm ID: $realm_id${NC}"
    echo -e "${BLUE}Gov Key:  $gov_key${NC}"
    
    # Try to get government multiaddr from config if available (optional)
    local gov_multiaddr=""
    if [ -f "$NODES_DIR/government/config.toml" ]; then
        gov_multiaddr=$(grep 'multiaddress = ' "$NODES_DIR/government/config.toml" 2>/dev/null | cut -d'"' -f2)
    fi
    
    if [ -z "$gov_multiaddr" ]; then
        gov_multiaddr="/ip4/127.0.0.1/tcp/9000"
        echo -e "${YELLOW}Note: Using default government address: $gov_multiaddr${NC}"
        echo -e "${YELLOW}Edit config.toml to set the correct multiaddress for your setup${NC}"
    fi
    
    mkdir -p "$NODES_DIR/$name"
    cd "$NODES_DIR/$name"
    
    cargo run --bin user -- init \
        --realm-id "$realm_id" \
        --gov-key "$gov_key" \
        --data-dir "."
    
    # Get the generated user key
    local user_key=$(cargo run --bin user -- status --data-dir "." 2>/dev/null | grep "User Key" | awk '{print $3}' || echo "")
    
    # Create config file
    cat > config.toml << EOF
[node]
type = "user"
name = "$name"
data_dir = "."

[realm]
realm_id = "$realm_id"
government_key = "$gov_key"

[network]
listen = "/ip4/0.0.0.0/tcp/0"

[network_info]
multiaddress = ""

[bootstrap]
government = "$gov_multiaddr"

[identity]
user_key = "$user_key"
EOF
    
    echo -e "${GREEN}User node '$name' initialized successfully!${NC}"
    echo ""
    echo -e "${BLUE}Configuration: $NODES_DIR/$name/config.toml${NC}"
    echo -e "${BLUE}User Key:      $user_key${NC}"
    echo ""
    echo "To start the node:"
    echo "  ./run.sh start $name --gov        # Connect to government"
    echo ""
    echo "For distributed setup:"
    echo "  1. Edit $NODES_DIR/$name/config.toml"
    echo "  2. Update [bootstrap] section with correct peer addresses"
}

function update_user_multiaddr() {
    local node=$1
    local node_dir="$PROJECT_ROOT/$NODES_DIR/$node"
    
    # Wait a bit for the node to start and print its multiaddress
    sleep 3
    
    if [ -f "$node_dir/$node.log" ]; then
        # Extract COMPLETE multiaddress including /p2p/<peer_id> - supports IPv4 and IPv6
        local multiaddr=$(grep -oE '/ip[46]/[0-9.:a-fA-F]+/tcp/[0-9]+/p2p/[a-zA-Z0-9]{52,}' "$node_dir/$node.log" | tail -1)
        
        if [ -n "$multiaddr" ]; then
            if grep -q "^\\[network_info\\]" "$node_dir/config.toml"; then
                if grep -q "^multiaddress = " "$node_dir/config.toml"; then
                    sed -i "s|^multiaddress = .*$|multiaddress = \"$multiaddr\"|" "$node_dir/config.toml"
                else
                    sed -i "/^\\[network_info\\]/a multiaddress = \"$multiaddr\"" "$node_dir/config.toml"
                fi
            fi
            
            echo -e "${GREEN}Multiaddress captured: $multiaddr${NC}"
            
            # Update other user configs' bootstrap sections
            for other_node in "$PROJECT_ROOT/$NODES_DIR"/*; do
                if [ -d "$other_node" ] && [ "$other_node" != "$node_dir" ]; then
                    local other_name=$(basename "$other_node")
                    local other_type=$(get_node_type "$other_node")
                    
                    if [ "$other_type" = "user" ] && [ -f "$other_node/config.toml" ]; then
                        # Remove any existing entry for this node to avoid duplicates
                        sed -i "/^$node = /d" "$other_node/config.toml"
                        # Add the new multiaddress
                        sed -i "/^\\[bootstrap\\]/a $node = \"$multiaddr\"" "$other_node/config.toml"
                        echo -e "${BLUE}Added $node to $other_name's bootstrap peers${NC}"
                    fi
                fi
            done
        fi
    fi
}

function start_node() {
    ensure_project_root
    local node=$1
    shift
    
    local node_dir=$(load_node_config "$node")
    local node_type=$(get_node_type "$node_dir")
    
    # First check if node is already running (better check)
    if [ "$node_type" = "government" ]; then
        # Check for actual government process
        local existing_pid=$(ps aux | grep "cargo run --bin government" | grep -v grep | awk '{print $2}' | head -1)
        if [ -n "$existing_pid" ]; then
            echo -e "${YELLOW}Government node is already running (PID: $existing_pid)${NC}"
            
            # Check if we have a PID file
            if [ ! -f "$node_dir/$node.pid" ] || [ "$(cat "$node_dir/$node.pid" 2>/dev/null)" != "$existing_pid" ]; then
                echo -e "${YELLOW}Updating PID file...${NC}"
                echo "$existing_pid" > "$node_dir/$node.pid"
            fi
            
            return 0
        fi
    elif [ "$node_type" = "user" ]; then
        # Check for actual user process
        local existing_pid=$(ps aux | grep "cargo run --bin user" | grep "$node_dir" | grep -v grep | awk '{print $2}' | head -1)
        if [ -n "$existing_pid" ]; then
            echo -e "${YELLOW}User node '$node' is already running (PID: $existing_pid)${NC}"
            
            if [ ! -f "$node_dir/$node.pid" ] || [ "$(cat "$node_dir/$node.pid" 2>/dev/null)" != "$existing_pid" ]; then
                echo -e "${YELLOW}Updating PID file...${NC}"
                echo "$existing_pid" > "$node_dir/$node.pid"
            fi
            
            return 0
        fi
    fi
    
    # Clean up stale PID file if exists
    if [ -f "$node_dir/$node.pid" ]; then
        local pid=$(cat "$node_dir/$node.pid")
        if ! kill -0 "$pid" 2>/dev/null; then
            echo -e "${YELLOW}Removing stale PID file...${NC}"
            rm "$node_dir/$node.pid"
        fi
    fi
    
    local dial_addr=""
    if [ "$node_type" = "user" ]; then
        while [[ $# -gt 0 ]]; do
            case $1 in
                --gov)
                    dial_addr=$(grep '^government = ' "$node_dir/config.toml" | cut -d'"' -f2)
                    if [ -z "$dial_addr" ]; then
                        echo -e "${YELLOW}Warning: Government address not found in config${NC}"
                    else
                        echo -e "${BLUE}Connecting to government: $dial_addr${NC}"
                    fi
                    shift
                    ;;
                --peer)
                    local peer_name=$2
                    if [ -z "$peer_name" ]; then
                        echo -e "${RED}Error: --peer requires a peer name${NC}"
                        exit 1
                    fi
                    dial_addr=$(grep "^$peer_name = " "$node_dir/config.toml" | cut -d'"' -f2)
                    if [ -z "$dial_addr" ]; then
                        echo -e "${RED}Error: Peer '$peer_name' not found in bootstrap config${NC}"
                        echo "Available peers:"
                        grep ' = "' "$node_dir/config.toml" | grep -A 10 '^\\[bootstrap\\]' | grep -v '^\\[' || echo "  (none)"
                        exit 1
                    else
                        echo -e "${BLUE}Connecting to peer $peer_name: $dial_addr${NC}"
                    fi
                    shift 2
                    ;;
                *)
                    echo -e "${YELLOW}Warning: Unknown option '$1' ignored${NC}"
                    shift
                    ;;
            esac
        done
    fi
    
    echo -e "${GREEN}Starting $node_type node: $node${NC}"
    cd "$node_dir"
    
    if [ "$node_type" = "government" ]; then
        # Check if port 9000 is available
        if ! check_port_available 9000 "government"; then
            echo -e "${YELLOW}Government may already be running on port 9000${NC}"
            echo -e "${YELLOW}Try: ./run.sh status government${NC}"
            echo -e "${YELLOW}Or: ./run.sh stop government${NC}"
            exit 1
        fi
        
        nohup cargo run --bin government -- run --keys-dir gov_keys --listen "/ip4/0.0.0.0/tcp/9000" > "$node.log" 2>&1 &
    else
        if [ -n "$dial_addr" ]; then
            nohup cargo run --bin user -- run --data-dir "." --dial "$dial_addr" > "$node.log" 2>&1 &
        else
            nohup cargo run --bin user -- run --data-dir "." > "$node.log" 2>&1 &
        fi
    fi
    
    local pid=$!
    echo $pid > "$node.pid"
    
    echo -e "${GREEN}Node started!${NC}"
    echo "  PID: $pid"
    echo "  Logs: $node_dir/$node.log"
    
    if [ "$node_type" = "user" ]; then
        (update_user_multiaddr "$node") &
    fi
    
    echo ""
    echo "View logs: ./run.sh logs $node"
    echo "Stop: ./run.sh stop $node"
}

function stop_node() {
    ensure_project_root
    local node=$1
    local node_dir=$(load_node_config "$node")
    local node_type=$(get_node_type "$node_dir")
    
    # First try to find the actual process if PID file is missing or stale
    local pid=""
    if [ -f "$node_dir/$node.pid" ]; then
        pid=$(cat "$node_dir/$node.pid")
        if ! kill -0 "$pid" 2>/dev/null; then
            echo -e "${YELLOW}Stale PID file found, looking for actual process...${NC}"
            pid=""
        fi
    fi
    
    if [ -z "$pid" ]; then
        if [ "$node_type" = "government" ]; then
            pid=$(ps aux | grep "cargo run --bin government" | grep -v grep | awk '{print $2}' | head -1)
        elif [ "$node_type" = "user" ]; then
            pid=$(ps aux | grep "cargo run --bin user" | grep "$node_dir" | grep -v grep | awk '{print $2}' | head -1)
        fi
    fi
    
    if [ -n "$pid" ]; then
        echo -e "${YELLOW}Stopping node: $node (PID: $pid)${NC}"
        kill "$pid"
        
        local count=0
        while kill -0 "$pid" 2>/dev/null && [ $count -lt 5 ]; do
            sleep 1
            count=$((count + 1))
        done
        
        if kill -0 "$pid" 2>/dev/null; then
            echo -e "${YELLOW}Force stopping...${NC}"
            kill -9 "$pid" 2>/dev/null || true
        fi
        
        echo -e "${GREEN}Node stopped${NC}"
    else
        echo -e "${YELLOW}Node '$node' is not running${NC}"
    fi
    
    # Remove PID file if exists
    rm -f "$node_dir/$node.pid"
}

function restart_node() {
    local node_name="$1"
    shift
    
    echo -e "${GREEN}Restarting node: $node_name${NC}"
    stop_node "$node_name"
    sleep 2
    start_node "$node_name" "$@"
}

function show_status() {
    ensure_project_root
    local node=$1
    local node_dir=$(load_node_config "$node")
    local node_type=$(get_node_type "$node_dir")
    
    local is_running="STOPPED"
    local pid=""
    
    # Check PID file first
    if [ -f "$node_dir/$node.pid" ]; then
        pid=$(cat "$node_dir/$node.pid")
        if kill -0 "$pid" 2>/dev/null; then
            is_running="${GREEN}RUNNING${NC} (PID: $pid)"
        else
            is_running="STOPPED (stale pid file)"
            pid=""
        fi
    fi
    
    # If no valid PID from file, check for process
    if [ -z "$pid" ] && [ "$node_type" = "government" ]; then
        pid=$(ps aux | grep "cargo run --bin government" | grep -v grep | awk '{print $2}' | head -1)
        if [ -n "$pid" ]; then
            is_running="${GREEN}RUNNING${NC} (PID: $pid, no PID file)"
        fi
    elif [ -z "$pid" ] && [ "$node_type" = "user" ]; then
        pid=$(ps aux | grep "cargo run --bin user" | grep "$node_dir" | grep -v grep | awk '{print $2}' | head -1)
        if [ -n "$pid" ]; then
            is_running="${GREEN}RUNNING${NC} (PID: $pid, no PID file)"
        fi
    fi
    
    echo -e "${BLUE}=== Node: $node ($node_type) ===${NC}"
    echo -e "Status: $is_running"
    
    local multiaddr=$(get_node_multiaddr "$node")
    if [ -n "$multiaddr" ]; then
        echo "Multiaddress: $multiaddr"
    fi
    
    echo ""
    
    cd "$node_dir"
    
    if [ "$node_type" = "government" ]; then
        cargo run --bin government -- info --keys-dir gov_keys 2>/dev/null || echo "Could not load government info"
    else
        cargo run --bin user -- status --data-dir "." 2>/dev/null || echo "Could not load user status"
    fi
}

function show_logs() {
    ensure_project_root
    local node=$1
    local lines=${2:-20}
    local node_dir=$(load_node_config "$node")
    
    if [ ! -f "$node_dir/$node.log" ]; then
        echo -e "${YELLOW}No log file found for node '$node'${NC}"
        return 1
    fi
    
    echo -e "${BLUE}=== Last $lines lines from $node.log ===${NC}"
    tail -n "$lines" "$node_dir/$node.log"
}

# Government operations
function gov_info() {
    ensure_project_root
    
    if [ ! -d "$NODES_DIR/government" ]; then
        echo -e "${RED}Error: Government node not initialized${NC}"
        exit 1
    fi
    
    cd "$NODES_DIR/government"
    cargo run --bin government -- info --keys-dir gov_keys
}

function gov_set_tax() {
    ensure_project_root
    local amount=$1
    
    if [ -z "$amount" ]; then
        echo -e "${RED}Error: Tax amount required${NC}"
        echo "Usage: ./run.sh gov-set-tax <amount>"
        exit 1
    fi
    
    if [ ! -d "$NODES_DIR/government" ]; then
        echo -e "${RED}Error: Government node not initialized${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}Setting tax amount to $amount coins...${NC}"
    cd "$NODES_DIR/government"
    cargo run --bin government -- set-tax --amount "$amount" --keys-dir gov_keys
}

function gov_add_task() {
    ensure_project_root
    local name=$1
    local mult=$2
    local land=$3
    local labor=$4
    local capital=$5
    
    if [ -z "$name" ] || [ -z "$mult" ] || [ -z "$land" ] || [ -z "$labor" ] || [ -z "$capital" ]; then
        echo -e "${RED}Error: All parameters required${NC}"
        echo "Usage: ./run.sh gov-add-task <name> <multiplier> <land> <labor> <capital>"
        echo "Example: ./run.sh gov-add-task trading 3 10 5 2"
        exit 1
    fi
    
    if [ ! -d "$NODES_DIR/government" ]; then
        echo -e "${RED}Error: Government node not initialized${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}Adding task '$name'...${NC}"
    cd "$NODES_DIR/government"
    cargo run --bin government -- add-task \
        --name "$name" \
        --multiplier "$mult" \
        --land "$land" \
        --labor "$labor" \
        --capital "$capital" \
        --keys-dir gov_keys
}

function gov_modify_task() {
    ensure_project_root
    local name=$1
    shift
    
    if [ -z "$name" ]; then
        echo -e "${RED}Error: Task name required${NC}"
        echo "Usage: ./run.sh gov-modify-task <name> [--multiplier N] [--land N] [--labor N] [--capital N]"
        echo "Example: ./run.sh gov-modify-task farming --multiplier 4 --land 15"
        exit 1
    fi
    
    if [ ! -d "$NODES_DIR/government" ]; then
        echo -e "${RED}Error: Government node not initialized${NC}"
        exit 1
    fi
    
    # Build command with optional parameters
    local cmd="cargo run --bin government -- modify-task --name \"$name\" --keys-dir gov_keys"
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --multiplier)
                cmd="$cmd --multiplier $2"
                shift 2
                ;;
            --land)
                cmd="$cmd --land $2"
                shift 2
                ;;
            --labor)
                cmd="$cmd --labor $2"
                shift 2
                ;;
            --capital)
                cmd="$cmd --capital $2"
                shift 2
                ;;
            *)
                echo -e "${YELLOW}Warning: Unknown option '$1' ignored${NC}"
                shift
                ;;
        esac
    done
    
    echo -e "${GREEN}Modifying task '$name'...${NC}"
    cd "$NODES_DIR/government"
    eval "$cmd"
}

function gov_remove_task() {
    ensure_project_root
    local name=$1
    
    if [ -z "$name" ]; then
        echo -e "${RED}Error: Task name required${NC}"
        echo "Usage: ./run.sh gov-remove-task <name>"
        exit 1
    fi
    
    if [ ! -d "$NODES_DIR/government" ]; then
        echo -e "${RED}Error: Government node not initialized${NC}"
        exit 1
    fi
    
    echo -e "${YELLOW}Removing task '$name'...${NC}"
    cd "$NODES_DIR/government"
    cargo run --bin government -- remove-task --name "$name" --keys-dir gov_keys
}

function gov_show_rules() {
    ensure_project_root
    
    if [ ! -d "$NODES_DIR/government" ]; then
        echo -e "${RED}Error: Government node not initialized${NC}"
        exit 1
    fi
    
    cd "$NODES_DIR/government"
    cargo run --bin government -- show-rules --keys-dir gov_keys
}

# User operations (context-aware)
function start_task() {
    ensure_project_root
    local task=$1
    
    if [ -z "$task" ]; then
        echo -e "${RED}Error: Task name required${NC}"
        echo "Usage: ./run.sh task-start <task>"
        echo "Available tasks: farming, mining, etc."
        exit 1
    fi
    
    # Determine which user node to use
    local current_user=$(get_current_user_node)
    if [ -z "$current_user" ]; then
        echo -e "${RED}Error: Could not determine which user node to use${NC}"
        echo ""
        echo "Running user nodes:"
        local found_running=0
        for node in "$NODES_DIR"/*; do
            if [ -d "$node" ]; then
                local node_type=$(get_node_type "$node")
                local node_name=$(basename "$node")
                if [ "$node_type" = "user" ]; then
                    local pid=""
                    # Check if running
                    if [ -f "$node/$node_name.pid" ]; then
                        local file_pid=$(cat "$node/$node_name.pid" 2>/dev/null)
                        if [ -n "$file_pid" ] && kill -0 "$file_pid" 2>/dev/null; then
                            pid="$file_pid"
                        fi
                    fi
                    if [ -z "$pid" ]; then
                        pid=$(pgrep -f "user run.*$node_name" || ps aux | grep "user run" | grep "$node_name" | grep -v grep | awk '{print $2}' | head -1)
                    fi
                    
                    if [ -n "$pid" ]; then
                        echo "  $node_name (PID: $pid)"
                        found_running=1
                    fi
                fi
            fi
        done
        
        echo ""
        
        if [ $found_running -eq 0 ]; then
            echo "Available user nodes (stopped):"
            for node in "$NODES_DIR"/*; do
                if [ -d "$node" ]; then
                    local node_type=$(get_node_type "$node")
                    if [ "$node_type" = "user" ]; then
                        echo "  $(basename "$node")"
                    fi
                fi
            done
            echo ""
            echo "Please start a user node first: ./run.sh start <user> --gov"
        else
            echo "Multiple user nodes are running. Please specify which user:"
            echo "  ./run.sh <user> task-start <task>"
        fi
        exit 1
    fi
    
    local node_dir="$PROJECT_ROOT/$NODES_DIR/$current_user"
    echo -e "${GREEN}Starting task '$task' for user: $current_user${NC}"
    
    cd "$node_dir"
    cargo run --bin user -- start-task "$task" --data-dir "."
}

function harvest_task() {
    ensure_project_root
    local task=$1
    
    if [ -z "$task" ]; then
        echo -e "${RED}Error: Task name required${NC}"
        echo "Usage: ./run.sh task-harvest <task>"
        exit 1
    fi
    
    # Determine which user node to use
    local current_user=$(get_current_user_node)
    if [ -z "$current_user" ]; then
        echo -e "${RED}Error: No running user node found${NC}"
        echo "Please start a user node first: ./run.sh start <user> --gov"
        exit 1
    fi
    
    local node_dir="$PROJECT_ROOT/$NODES_DIR/$current_user"
    echo -e "${GREEN}Harvesting task '$task' for user: $current_user${NC}"
    
    cd "$node_dir"
    cargo run --bin user -- harvest-task "$task" --data-dir "."
}

function pay_tax() {
    ensure_project_root
    
    # Determine which user node to use
    local current_user=$(get_current_user_node)
    if [ -z "$current_user" ]; then
        echo -e "${RED}Error: No running user node found${NC}"
        echo "Please start a user node first: ./run.sh start <user> --gov"
        exit 1
    fi
    
    local node_dir="$PROJECT_ROOT/$NODES_DIR/$current_user"
    echo -e "${GREEN}Submitting tax obligation for user: $current_user${NC}"
    echo -e "${BLUE}(Tax amount will be automatically determined by government rules)${NC}"
    
    cd "$node_dir"
    cargo run --bin user -- submit-obligation --data-dir "."
}

function send_coins() {
    ensure_project_root
    local sender=$1
    local recipient=$2
    local amount=$3
    
    if [ -z "$sender" ] || [ -z "$recipient" ] || [ -z "$amount" ]; then
        echo -e "${RED}Error: Missing parameters${NC}"
        echo "Usage: ./run.sh send <sender> <recipient> <amount>"
        echo "Example: ./run.sh send alice bob 5"
        exit 1
    fi
    
    # Check if sender node exists
    local sender_dir="$PROJECT_ROOT/$NODES_DIR/$sender"
    if [ ! -d "$sender_dir" ]; then
        echo -e "${RED}Error: Sender node '$sender' not found${NC}"
        echo "Available user nodes:"
        for node in "$NODES_DIR"/*; do
            if [ -d "$node" ]; then
                local node_type=$(get_node_type "$node")
                if [ "$node_type" = "user" ]; then
                    echo "  $(basename "$node")"
                fi
            fi
        done
        exit 1
    fi
    
    # Check if sender node is running
    local sender_pid=""
    if [ -f "$sender_dir/$sender.pid" ]; then
        local file_pid=$(cat "$sender_dir/$sender.pid" 2>/dev/null)
        if [ -n "$file_pid" ] && kill -0 "$file_pid" 2>/dev/null; then
            sender_pid="$file_pid"
        fi
    fi
    if [ -z "$sender_pid" ]; then
        sender_pid=$(pgrep -f "user run.*$sender" || ps aux | grep "user run" | grep "$sender" | grep -v grep | awk '{print $2}' | head -1)
    fi
    
    if [ -z "$sender_pid" ]; then
        echo -e "${RED}Error: Sender node '$sender' is not running${NC}"
        echo "Please start it first: ./run.sh start $sender --gov"
        exit 1
    fi
    
    # Check recipient node
    local recipient_dir="$PROJECT_ROOT/$NODES_DIR/$recipient"
    if [ ! -d "$recipient_dir" ]; then
        echo -e "${RED}Error: Recipient node '$recipient' not found${NC}"
        echo "Available user nodes:"
        for node in "$NODES_DIR"/*; do
            if [ -d "$node" ]; then
                local node_type=$(get_node_type "$node")
                if [ "$node_type" = "user" ]; then
                    echo "  $(basename "$node")"
                fi
            fi
        done
        exit 1
    fi
    
    # Get recipient key
    local recipient_key=""
    if [ -f "$recipient_dir/config.toml" ]; then
        recipient_key=$(grep 'user_key = ' "$recipient_dir/config.toml" | cut -d'"' -f2)
    fi
    
    if [ -z "$recipient_key" ]; then
        cd "$recipient_dir"
        recipient_key=$(cargo run --bin user -- status --data-dir "." 2>/dev/null | grep "User Key" | awk '{print $3}')
    fi
    
    if [ -z "$recipient_key" ]; then
        echo -e "${RED}Error: Could not determine recipient public key${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}Sending $amount coins from $sender to $recipient${NC}"
    echo "Recipient key: $recipient_key"
    
    cd "$sender_dir"
    cargo run --bin user -- send-coins --to "$recipient_key" "$amount" --data-dir "."
}

function update_gov_multiaddr() {
    ensure_project_root
    
    if [ ! -d "$NODES_DIR/government" ]; then
        echo -e "${RED}Error: Government node not initialized${NC}"
        exit 1
    fi
    
    # Check if government is running by looking at the process
    local gov_pid=""
    
    # First check PID file
    if [ -f "$NODES_DIR/government/government.pid" ]; then
        gov_pid=$(cat "$NODES_DIR/government/government.pid")
        if ! kill -0 "$gov_pid" 2>/dev/null; then
            echo -e "${YELLOW}Warning: PID file exists but process is not running${NC}"
            gov_pid=""
        fi
    fi
    
    # If not running via PID, try to find the process
    if [ -z "$gov_pid" ]; then
        echo -e "${YELLOW}Checking for running government process...${NC}"
        gov_pid=$(ps aux | grep "cargo run --bin government" | grep -v grep | awk '{print $2}' | head -1)
        
        if [ -z "$gov_pid" ]; then
            echo -e "${RED}Error: Government node is not running${NC}"
            echo "Start it first with: ./run.sh start government"
            exit 1
        fi
        
        echo -e "${GREEN}Found running government process (PID: $gov_pid)${NC}"
        # Update PID file
        echo "$gov_pid" > "$NODES_DIR/government/government.pid"
    fi
    
    if [ ! -f "$NODES_DIR/government/government.log" ]; then
        echo -e "${YELLOW}Waiting for log file to be created...${NC}"
        local wait_count=0
        while [ ! -f "$NODES_DIR/government/government.log" ] && [ $wait_count -lt 10 ]; do
            sleep 1
            wait_count=$((wait_count + 1))
        done
        
        if [ ! -f "$NODES_DIR/government/government.log" ]; then
            echo -e "${RED}Error: Government log file not found after waiting${NC}"
            echo "The node may not have started correctly"
            exit 1
        fi
    fi
    
    echo -e "${BLUE}Waiting for node to publish its multiaddress...${NC}"
    sleep 3
    
    # Extract COMPLETE multiaddress including /p2p/<peer_id> - must be the FULL peer ID
    # Use multiple patterns to catch different log formats
    local multiaddr=""
    local raw_match=""
    
    # Pattern 1: Try to get complete multiaddress with full peer ID (52+ chars)
    raw_match=$(grep -oE '/ip[46]/[0-9.:a-fA-F]+/tcp/[0-9]+/p2p/[a-zA-Z0-9]+' "$NODES_DIR/government/government.log" | tail -1)
    
    if [ -n "$raw_match" ]; then
        # Extract peer ID to check its length
        local peer_id=$(echo "$raw_match" | grep -oE 'p2p/[a-zA-Z0-9]+' | cut -d'/' -f2)
        if [ ${#peer_id} -ge 52 ]; then
            multiaddr="$raw_match"
            echo -e "${GREEN}Found multiaddress with full peer ID (${#peer_id} chars)${NC}"
        else
            echo -e "${YELLOW}Found multiaddress but peer ID is too short (${#peer_id} chars): $peer_id${NC}"
            echo -e "${YELLOW}This might be truncated in logs. Trying other patterns...${NC}"
        fi
    fi
    
    # Pattern 2: Try lines with "Listening" keyword
    if [ -z "$multiaddr" ]; then
        raw_match=$(grep -i 'listening' "$NODES_DIR/government/government.log" | grep -oE '/ip[46]/[0-9.:a-fA-F]+/tcp/[0-9]+/p2p/[a-zA-Z0-9]+' | tail -1)
        if [ -n "$raw_match" ]; then
            local peer_id=$(echo "$raw_match" | grep -oE 'p2p/[a-zA-Z0-9]+' | cut -d'/' -f2)
            if [ ${#peer_id} -ge 52 ]; then
                multiaddr="$raw_match"
                echo -e "${GREEN}Found multiaddress from 'Listening' line (${#peer_id} chars)${NC}"
            fi
        fi
    fi
    
    # Pattern 3: Try to find peer ID separately and reconstruct
    if [ -z "$multiaddr" ]; then
        echo -e "${YELLOW}Trying to extract peer ID from separate log lines...${NC}"
        local peer_id=$(grep -oE '(Peer ID|PeerId|Local peer id):? ?[a-zA-Z0-9]{52,}' "$NODES_DIR/government/government.log" | grep -oE '[a-zA-Z0-9]{52,}' | head -1)
        local listen_addr=$(grep -oE '/ip[46]/[0-9.:a-fA-F]+/tcp/[0-9]+' "$NODES_DIR/government/government.log" | grep '0.0.0.0\|127.0.0.1' | tail -1)
        
        if [ -n "$peer_id" ] && [ -n "$listen_addr" ]; then
            multiaddr="${listen_addr}/p2p/${peer_id}"
            echo -e "${GREEN}Reconstructed multiaddress from separate components${NC}"
        fi
    fi
    
    if [ -z "$multiaddr" ]; then
        echo -e "${RED}Error: Could not find valid multiaddress in government logs${NC}"
        echo -e "${YELLOW}Showing log contents to help debug:${NC}"
        echo "========================="
        tail -50 "$NODES_DIR/government/government.log"
        echo "========================="
        
        # Check if there's an existing multiaddress in config
        if grep -q 'multiaddress = ' "$NODES_DIR/government/config.toml"; then
            multiaddr=$(grep 'multiaddress = ' "$NODES_DIR/government/config.toml" | cut -d'"' -f2)
            if [ -n "$multiaddr" ] && [ "$multiaddr" != '""' ] && [[ "$multiaddr" =~ /p2p/[a-zA-Z0-9]{52,} ]]; then
                echo -e "${YELLOW}Using existing valid multiaddress from config: $multiaddr${NC}"
            else
                echo -e "${RED}No valid multiaddress found${NC}"
                exit 1
            fi
        else
            echo -e "${RED}Could not extract multiaddress from logs${NC}"
            echo ""
            echo "Possible solutions:"
            echo "  1. Wait a few more seconds and try again"
            echo "  2. Check if the node started correctly: ./run.sh logs government"
            echo "  3. Manually set the multiaddress in config.toml"
            exit 1
        fi
    fi
    
    # Validate multiaddress format
    if [[ ! "$multiaddr" =~ ^/ip[46]/[0-9.:a-fA-F]+/tcp/[0-9]+/p2p/[a-zA-Z0-9]{52,}$ ]]; then
        echo -e "${RED}Error: Invalid multiaddress format: $multiaddr${NC}"
        echo "Expected format: /ip4/<IP>/tcp/<PORT>/p2p/<PEER_ID>"
        echo "Peer ID should be 52+ characters"
        
        # Show what we got
        local peer_id=$(echo "$multiaddr" | grep -oE 'p2p/[a-zA-Z0-9]*' | cut -d'/' -f2)
        echo "Peer ID found: '$peer_id' (${#peer_id} characters)"
        exit 1
    fi
    
    # Replace 0.0.0.0 with 127.0.0.1 for localhost setups
    if [[ "$multiaddr" =~ /ip4/0\.0\.0\.0/ ]]; then
        echo -e "${YELLOW}Replacing 0.0.0.0 with 127.0.0.1 for local connections${NC}"
        multiaddr=$(echo "$multiaddr" | sed 's|/ip4/0\.0\.0\.0/|/ip4/127.0.0.1/|')
    fi
    
    echo -e "${GREEN}Found valid multiaddress: $multiaddr${NC}"
    
    # Update government config - REPLACE entire line
    if grep -q "^\\[network_info\\]" "$NODES_DIR/government/config.toml"; then
        # Remove old multiaddress line completely
        sed -i '/^multiaddress = /d' "$NODES_DIR/government/config.toml"
        # Add new multiaddress after [network_info]
        sed -i "/^\\[network_info\\]/a multiaddress = \"$multiaddr\"" "$NODES_DIR/government/config.toml"
    else
        echo "" >> "$NODES_DIR/government/config.toml"
        echo "[network_info]" >> "$NODES_DIR/government/config.toml"
        echo "multiaddress = \"$multiaddr\"" >> "$NODES_DIR/government/config.toml"
    fi
    
    echo -e "${GREEN}Updated government config with multiaddress${NC}"
    
    # Update all user configs - REPLACE old lines completely
    for user_node in "$NODES_DIR"/*; do
        if [ -d "$user_node" ] && [ "$user_node" != "$NODES_DIR/government" ]; then
            local user_name=$(basename "$user_node")
            local user_type=$(get_node_type "$user_node")
            
            if [ "$user_type" = "user" ] && [ -f "$user_node/config.toml" ]; then
                # Remove ALL old government lines (handles concatenated/multi-line addresses)
                # This removes lines starting with 'government = '
                sed -i '/^government = /d' "$user_node/config.toml"
                
                # Also clean up any orphaned continuation lines from corrupted multi-line values
                # Remove lines that look like standalone addresses (not key = value format)
                sed -i '/^\/ip[46]\//d' "$user_node/config.toml"
                
                # Remove any duplicate/empty lines in bootstrap section
                awk 'BEGIN {in_bootstrap=0} 
                     /^\[bootstrap\]/ {in_bootstrap=1; print; next} 
                     /^\[/ {in_bootstrap=0} 
                     in_bootstrap && /^$/ {next} 
                     {print}' "$user_node/config.toml" > "$user_node/config.toml.tmp"
                mv "$user_node/config.toml.tmp" "$user_node/config.toml"
                
                # Add new government address in the [bootstrap] section
                if grep -q "^\\[bootstrap\\]" "$user_node/config.toml"; then
                    sed -i "/^\\[bootstrap\\]/a government = \"$multiaddr\"" "$user_node/config.toml"
                    echo -e "${BLUE}Updated $user_name's government bootstrap address${NC}"
                else
                    # Add [bootstrap] section if it doesn't exist
                    echo "" >> "$user_node/config.toml"
                    echo "[bootstrap]" >> "$user_node/config.toml"
                    echo "government = \"$multiaddr\"" >> "$user_node/config.toml"
                    echo -e "${BLUE}Added bootstrap section to $user_name${NC}"
                fi
            fi
        fi
    done
    
    echo ""
    echo -e "${GREEN}All done! Government multiaddress updated everywhere.${NC}"
}

function list_nodes() {
    ensure_project_root
    echo -e "${BLUE}=== Available Nodes ===${NC}"
    
    if [ ! -d "$NODES_DIR" ] || [ -z "$(ls -A $NODES_DIR 2>/dev/null)" ]; then
        echo "No nodes found"
        return
    fi
    
    for node_path in "$NODES_DIR"/*; do
        if [ -d "$node_path" ]; then
            local name=$(basename "$node_path")
            local node_dir="$PROJECT_ROOT/$NODES_DIR/$name"
            local type=$(get_node_type "$node_dir")
            local status="${RED}STOPPED${NC}"
            local pid=""
            
            # 1. Try to get PID from the .pid file first
            if [ -f "$node_dir/$name.pid" ]; then
                pid=$(cat "$node_dir/$name.pid")
                # Check if the process with that PID is actually alive
                if ! kill -0 "$pid" 2>/dev/null; then
                    pid=""
                fi
            fi
            
            # 2. If no valid PID file, fallback to a more flexible process search
            if [ -z "$pid" ]; then
                if [ "$type" = "government" ]; then
                    # Matches the binary name regardless of cargo overhead
                    pid=$(pgrep -f "target/debug/government" || ps aux | grep "government run" | grep -v grep | awk '{print $2}' | head -1)
                elif [ "$type" = "user" ]; then
                    # Matches the binary and the specific data directory to distinguish between alice/bob
                    pid=$(pgrep -f "user run.*$name" || ps aux | grep "user run" | grep "$name" | grep -v grep | awk '{print $2}' | head -1)
                fi
            fi

            # 3. If a PID was found by either method, mark as RUNNING
            if [ -n "$pid" ]; then
                status="${GREEN}RUNNING${NC}"
                # Sync the PID file if it was missing
                echo "$pid" > "$node_dir/$name.pid" 2>/dev/null || true
            fi
            
            echo -e "  $name ($type) - $status"
            if [ -n "$pid" ]; then
                echo "    PID: $pid"
            fi
            
            # Only get multiaddr if it's set in config or logs
            local multiaddr=$(get_node_multiaddr "$name" 2>/dev/null || echo "")
            if [ -n "$multiaddr" ]; then
                echo "    Multiaddr: $multiaddr"
            fi
        fi
    done
}

function clean_nodes() {
    ensure_project_root
    local node=$1
    
    if [ -z "$node" ]; then
        echo -e "${YELLOW}WARNING: This will delete ALL node data!${NC}"
        echo "Nodes to be removed:"
        ls -1 "$NODES_DIR" 2>/dev/null || echo "  (none)"
        echo ""
        read -p "Are you sure? (yes/no): " confirm
        
        if [ "$confirm" = "yes" ]; then
            for n in "$NODES_DIR"/*; do
                if [ -d "$n" ]; then
                    local name=$(basename "$n")
                    stop_node "$name" 2>/dev/null || true
                fi
            done
            
            rm -rf "$NODES_DIR"
            echo -e "${GREEN}All node data cleaned${NC}"
        else
            echo "Cancelled"
        fi
    else
        local node_dir="$PROJECT_ROOT/$NODES_DIR/$node"
        if [ ! -d "$node_dir" ]; then
            echo -e "${RED}Error: Node '$node' not found${NC}"
            exit 1
        fi
        
        echo -e "${YELLOW}WARNING: This will delete node '$node' data!${NC}"
        read -p "Are you sure? (yes/no): " confirm
        
        if [ "$confirm" = "yes" ]; then
            stop_node "$node" 2>/dev/null || true
            rm -rf "$node_dir"
            echo -e "${GREEN}Node '$node' data cleaned${NC}"
        else
            echo "Cancelled"
        fi
    fi
}

function backup_nodes() {
    ensure_project_root
    local node=$1
    local timestamp=$(date +%Y%m%d_%H%M%S)
    
    if [ -z "$node" ]; then
        local backup_file="$NODES_DIR/backup_all_${timestamp}.tar.gz"
        echo -e "${GREEN}Creating backup of all nodes...${NC}"
        # Create backup with relative paths
        tar -czf "$backup_file" -C "$PROJECT_ROOT" "nodes"
        echo -e "${GREEN}Backup created: $backup_file${NC}"
        echo "Contents: nodes/..."
    else
        local node_dir="$PROJECT_ROOT/$NODES_DIR/$node"
        if [ ! -d "$node_dir" ]; then
            echo -e "${RED}Error: Node '$node' not found${NC}"
            exit 1
        fi
        
        local backup_file="$NODES_DIR/backup_${node}_${timestamp}.tar.gz"
        echo -e "${GREEN}Creating backup of node '$node'...${NC}"
        # Create backup with path: nodes/<node>/
        tar -czf "$backup_file" -C "$PROJECT_ROOT" "nodes/$node"
        echo -e "${GREEN}Backup created: $backup_file${NC}"
        echo "Contents: nodes/$node/..."
    fi
}
function restore_backup() {
    ensure_project_root
    local backup_file=$1
    
    if [ -z "$backup_file" ]; then
        echo -e "${RED}Error: Backup file required${NC}"
        echo "Usage: ./run.sh restore <backup-file>"
        echo "Note: Backup files are stored in ./nodes/ directory"
        echo "Example: ./run.sh restore nodes/backup_alice_20250206_123456.tar.gz"
        exit 1
    fi
    
    # Check if file exists with or without nodes/ prefix
    local backup_path="$backup_file"
    if [ ! -f "$backup_file" ] && [ -f "$NODES_DIR/$backup_file" ]; then
        backup_path="$NODES_DIR/$backup_file"
    fi
    
    if [ ! -f "$backup_path" ]; then
        echo -e "${RED}Error: Backup file not found: $backup_file${NC}"
        echo "Looking in: $backup_file"
        echo "Also tried: $NODES_DIR/$backup_file"
        echo ""
        echo "Available backups in $NODES_DIR/:"
        ls -1 "$NODES_DIR/"backup_*.tar.gz 2>/dev/null || echo "  (none)"
        exit 1
    fi
    
    echo -e "${YELLOW}WARNING: This will overwrite existing node data!${NC}"
    read -p "Continue? (yes/no): " confirm
    
    if [ "$confirm" = "yes" ]; then
        echo -e "${GREEN}Restoring from backup: $backup_path${NC}"
        
        # First, list what's in the tarball
        echo "Backup contents:"
        tar -tzf "$backup_path" | head -10
        
        # Extract to current directory (project root)
        # This will create nodes/ directory if it doesn't exist
        tar -xzf "$backup_path"
        
        echo -e "${GREEN}Restore complete${NC}"
        echo ""
        echo "Restored nodes:"
        ls -1 "$NODES_DIR" 2>/dev/null | grep -v backup_ || echo "  (none)"
    else
        echo "Cancelled"
    fi
}

function kill_port() {
    local port=$1
    if [ -z "$port" ]; then
        echo -e "${RED}Error: Port number required${NC}"
        echo "Usage: ./run.sh kill-port <port>"
        echo "Example: ./run.sh kill-port 9000"
        exit 1
    fi
    
    echo -e "${YELLOW}Looking for processes on port $port...${NC}"
    
    local pids=""
    if command -v lsof >/dev/null 2>&1; then
        pids=$(lsof -ti ":$port" 2>/dev/null || true)
    elif command -v fuser >/dev/null 2>&1; then
        pids=$(fuser "$port/tcp" 2>/dev/null | tr -d ' ' || true)
    fi
    
    if [ -z "$pids" ]; then
        echo -e "${GREEN}No processes found on port $port${NC}"
        return 0
    fi
    
    echo -e "${YELLOW}Found processes on port $port: $pids${NC}"
    echo -e "${YELLOW}Killing processes...${NC}"
    
    for pid in $pids; do
        if kill -0 "$pid" 2>/dev/null; then
            kill -9 "$pid" 2>/dev/null && echo -e "${GREEN}Killed PID $pid${NC}" || echo -e "${RED}Failed to kill PID $pid${NC}"
        fi
    done
    
    echo -e "${GREEN}Port $port cleared${NC}"
}
function kill_all_nodes() {
    ensure_project_root
    echo -e "${YELLOW}Killing all running realm nodes...${NC}"
    
    # First, stop nodes properly if possible
    if [ -d "$NODES_DIR" ]; then
        echo -e "${YELLOW}Gracefully stopping nodes...${NC}"
        for node_path in "$NODES_DIR"/*; do
            if [ -d "$node_path" ]; then
                local node_name=$(basename "$node_path")
                # Try to stop node gracefully (without killing)
                stop_node "$node_name" 2>/dev/null || true
            fi
        done
        sleep 2
    fi
    
    # Now kill any remaining processes that are clearly our nodes
    echo -e "${YELLOW}Force killing remaining node processes...${NC}"
    
    # Find processes that are running from our nodes directory
    local node_processes=$(ps aux | grep -E "(cargo run --bin (government|user)|target/debug/(government|user))" | grep -v grep | awk '{print $2}')
    
    # Also find processes by data-dir parameter
    if [ -d "$NODES_DIR" ]; then
        for node_dir in "$NODES_DIR"/*; do
            if [ -d "$node_dir" ]; then
                local node_name=$(basename "$node_dir")
                # Look for processes with this specific data directory
                local specific_pids=$(ps aux | grep "$node_dir" | grep -v grep | awk '{print $2}')
                if [ -n "$specific_pids" ]; then
                    node_processes="$node_processes $specific_pids"
                fi
            fi
        done
    fi
    
    # Kill all found processes
    local killed_count=0
    for pid in $node_processes; do
        # Remove duplicates and empty, check if process exists
        if [ -n "$pid" ] && [ "$pid" != "$$" ] && kill -0 "$pid" 2>/dev/null; then
            echo -e "${YELLOW}Killing PID $pid${NC}"
            kill -9 "$pid" 2>/dev/null && echo -e "${GREEN}Killed PID $pid${NC}" || true
            killed_count=$((killed_count + 1))
        fi
    done
    
    # Remove all PID files
    if [ -d "$NODES_DIR" ]; then
        echo -e "${YELLOW}Cleaning up PID files...${NC}"
        find "$NODES_DIR" -name "*.pid" -delete 2>/dev/null || true
    fi
    
    # Only check ports that our nodes use
    echo -e "${YELLOW}Clearing used ports...${NC}"
    # Check which ports are actually in use by our configs
    for port in 9000 9001 9002 9003 9004 9005; do
        local port_pids=$(lsof -ti:$port 2>/dev/null || true)
        if [ -n "$port_pids" ]; then
            # Only kill if the process is one of our nodes (by checking command line)
            for pid in $port_pids; do
                local cmdline=$(ps -p $pid -o command= 2>/dev/null || echo "")
                if [[ "$cmdline" =~ (government|user|target/debug) ]]; then
                    kill -9 "$pid" 2>/dev/null && echo -e "${GREEN}Cleared port $port (PID: $pid)${NC}" || true
                fi
            done
        fi
    done
    
    echo -e "${GREEN}Killed $killed_count node processes${NC}"
    
    # Verify cleanup
    echo -e "${YELLOW}Verifying...${NC}"
    local remaining=$(ps aux | grep -E "(cargo run --bin (government|user)|target/debug/(government|user))" | grep -v grep | wc -l)
    
    if [ "$remaining" -eq 0 ]; then
        echo -e "${GREEN}✓ All realm nodes stopped${NC}"
    else
        echo -e "${YELLOW}⚠ $remaining processes might still be running${NC}"
        echo "Run 'ps aux | grep -E \"(government|user)\" | grep -v grep' to see them"
    fi
}

# Main command dispatch
case "$1" in
    init-gov)
        init_government
        ;;
    init-user)
        init_user "$2" "$3" "$4"
        ;;
    update-gov-addr)
        update_gov_multiaddr
        ;;
    start)
        node_name="$2"
        shift 2
        start_node "$node_name" "$@"
        ;;
    stop)
        stop_node "$2"
        ;;
    restart)
        node_name="$2"
        shift 2
        restart_node "$node_name" "$@"
        ;;
    status)
        show_status "$2"
        ;;
    logs)
        show_logs "$2" "$3"
        ;;
    
    # Government operations
    gov-info)
        gov_info
        ;;
    gov-set-tax)
        gov_set_tax "$2"
        ;;
    gov-add-task)
        gov_add_task "$2" "$3" "$4" "$5" "$6"
        ;;
    gov-modify-task)
        shift
        gov_modify_task "$@"
        ;;
    gov-remove-task)
        gov_remove_task "$2"
        ;;
    gov-show-rules)
        gov_show_rules
        ;;
    
    # User operations
    task-start)
        start_task "$2"
        ;;
    task-harvest)
        harvest_task "$2"
        ;;
    pay-tax)
        pay_tax
        ;;
    send)
        send_coins "$2" "$3" "$4"
        ;;
    
    # Maintenance
    list)
        list_nodes
        ;;
    clean)
        clean_nodes "$2"
        ;;
    backup)
        backup_nodes "$2"
        ;;
    restore)
        restore_backup "$2"
        ;;
    kill-port)
        kill_port "$2"
        ;;
    kill-all)
        kill_all_nodes
        ;;
    help|--help|-h|"")
        print_usage
        ;;
    *)
        echo -e "${RED}Error: Unknown command '$1'${NC}"
        echo ""
        print_usage
        exit 1
        ;;
esac