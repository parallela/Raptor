#!/bin/bash

# Raptor Panel - Java Server Entrypoint
# This script prepares and launches Java-based game servers

set -e

# Set timezone (default to UTC)
TZ="${TZ:-UTC}"
export TZ

# Get the container's internal IP address
INTERNAL_IP=$(ip route get 1 2>/dev/null | awk '{print $(NF-2);exit}' || echo "127.0.0.1")
export INTERNAL_IP

# Change to the server directory
cd /home/container || exit 1

# Display Java version for debugging
echo -e "\033[1m\033[36m[Raptor]\033[0m Java version:"
java -version
echo ""

# Parse the startup command
# Converts {{VARIABLE}} format to ${VARIABLE} and evaluates
parse_startup() {
    local cmd="$1"
    # Replace {{VAR}} with ${VAR}
    cmd=$(echo "$cmd" | sed -e 's/{{/${/g' -e 's/}}/}/g')
    # Evaluate the variables
    eval echo "$cmd"
}

# Get the parsed startup command
PARSED=$(parse_startup "${STARTUP}")

# Display what we're about to run
echo -e "\033[1m\033[36m[Raptor]\033[0m Starting server with command:"
echo -e "\033[1m\033[33m$\033[0m $PARSED"
echo ""

# Execute the startup command
# Using exec replaces this shell with the server process
# This ensures signals (like SIGTERM) are sent directly to the server
exec $PARSED
