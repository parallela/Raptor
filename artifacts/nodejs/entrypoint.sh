#!/bin/bash

# Raptor Panel - Node.js Container Entrypoint
# This script handles startup for Node.js-based servers

# Default the TZ environment variable to UTC
TZ=${TZ:-UTC}
export TZ

# Set environment variable that holds the Internal Docker IP
INTERNAL_IP=$(ip route get 1 2>/dev/null | awk '{print $(NF-2);exit}' || echo "127.0.0.1")
export INTERNAL_IP

# Switch to the container's working directory
cd /home/container || exit 1

# Print Node.js version
printf "\033[1m\033[36m[Raptor]\033[0m Node.js version:\n"
node --version
printf "\033[1m\033[36m[Raptor]\033[0m NPM version:\n"
npm --version

# Convert all of the "{{VARIABLE}}" parts of the command into the expected shell
# variable format of "${VARIABLE}" before evaluating the string and automatically
# replacing the values.
PARSED=$(echo "${STARTUP}" | sed -e 's/{{/${/g' -e 's/}}/}/g')
PARSED=$(eval echo "$PARSED")

# Display the command we're running in the output, and then execute it with the env
# from the container itself.
printf "\033[1m\033[36m[Raptor]\033[0m Starting server with command:\n"
printf "\033[1m\033[33m$ \033[0m%s\n" "$PARSED"

# Handle SIGTERM gracefully - forward to child process and wait
# This ensures Docker stop works correctly
_term() {
    echo "[Raptor] Received shutdown signal, stopping server..."
    if [ -n "$child" ]; then
        kill -TERM "$child" 2>/dev/null
        wait "$child"
    fi
    exit 0
}
trap _term SIGTERM SIGINT

# Run the startup command in background, then wait for it
# This allows us to handle signals properly
eval $PARSED &
child=$!
wait "$child"
