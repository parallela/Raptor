#!/bin/bash
#
# Raptor Daemon Installation Script
# This script sets up the raptor user, directories, and systemd service
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
RAPTOR_USER="raptor"
RAPTOR_GROUP="raptor"
RAPTOR_HOME="/opt/raptor"
RAPTOR_DATA="/var/lib/raptor"
RAPTOR_LOG="/var/log/raptor"
DAEMON_PORT="6969"
FTP_PORT="2121"

echo -e "${GREEN}=== Raptor Daemon Installation ===${NC}"
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}Error: This script must be run as root${NC}"
    echo "Please run: sudo $0"
    exit 1
fi

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo -e "${RED}Error: Docker is not installed${NC}"
    echo "Please install Docker first: https://docs.docker.com/engine/install/"
    exit 1
fi

echo -e "${YELLOW}Step 1: Creating raptor user and group...${NC}"

# Create raptor group if it doesn't exist
if ! getent group "$RAPTOR_GROUP" > /dev/null 2>&1; then
    groupadd --system "$RAPTOR_GROUP"
    echo "  Created group: $RAPTOR_GROUP"
else
    echo "  Group $RAPTOR_GROUP already exists"
fi

# Create raptor user if it doesn't exist
if ! id "$RAPTOR_USER" > /dev/null 2>&1; then
    useradd --system \
        --gid "$RAPTOR_GROUP" \
        --home-dir "$RAPTOR_HOME" \
        --shell /usr/sbin/nologin \
        --comment "Raptor Daemon Service" \
        "$RAPTOR_USER"
    echo "  Created user: $RAPTOR_USER"
else
    echo "  User $RAPTOR_USER already exists"
fi

# Add raptor user to docker group so it can manage containers
if getent group docker > /dev/null 2>&1; then
    usermod -aG docker "$RAPTOR_USER"
    echo "  Added $RAPTOR_USER to docker group"
else
    echo -e "${YELLOW}  Warning: docker group not found. Make sure Docker is properly installed.${NC}"
fi

echo -e "${YELLOW}Step 2: Creating directories...${NC}"

# Create directories
mkdir -p "$RAPTOR_HOME"
mkdir -p "$RAPTOR_HOME/certs"
mkdir -p "$RAPTOR_DATA"
mkdir -p "$RAPTOR_DATA/volumes"
mkdir -p "$RAPTOR_DATA/sys"
mkdir -p "$RAPTOR_LOG"

echo "  Created: $RAPTOR_HOME"
echo "  Created: $RAPTOR_DATA"
echo "  Created: $RAPTOR_DATA/volumes"
echo "  Created: $RAPTOR_LOG"

# Set ownership
chown -R "$RAPTOR_USER:$RAPTOR_GROUP" "$RAPTOR_HOME"
chown -R "$RAPTOR_USER:$RAPTOR_GROUP" "$RAPTOR_DATA"
chown -R "$RAPTOR_USER:$RAPTOR_GROUP" "$RAPTOR_LOG"

# Set permissions for volumes directory
# This allows container user (UID 1000) to write to volumes
chmod 777 "$RAPTOR_DATA/volumes"

echo "  Set ownership to $RAPTOR_USER:$RAPTOR_GROUP"

echo -e "${YELLOW}Step 3: Creating configuration file...${NC}"

# Create default .env file if it doesn't exist
if [ ! -f "$RAPTOR_HOME/.env" ]; then
    # Generate a random API key
    API_KEY=$(openssl rand -hex 32)

    cat > "$RAPTOR_HOME/.env" << EOF
# Raptor Daemon Configuration
# Generated on $(date)

# API Key for authentication (change this!)
DAEMON_API_KEY=$API_KEY

# Daemon address and port
DAEMON_ADDR=0.0.0.0:$DAEMON_PORT

# Docker socket
DOCKER_HOST=unix:///var/run/docker.sock

# Data directories
DAEMON_DATA_DIR=$RAPTOR_DATA/sys
FTP_BASE_PATH=$RAPTOR_DATA

# FTP Server
FTP_HOST=0.0.0.0
FTP_PORT=$FTP_PORT

# TLS Configuration (optional - uncomment and set paths for HTTPS)
# TLS_CERT_PATH=$RAPTOR_HOME/certs/fullchain.pem
# TLS_KEY_PATH=$RAPTOR_HOME/certs/privkey.pem

# Logging
RUST_LOG=info
EOF

    chown "$RAPTOR_USER:$RAPTOR_GROUP" "$RAPTOR_HOME/.env"
    chmod 600 "$RAPTOR_HOME/.env"

    echo "  Created: $RAPTOR_HOME/.env"
    echo -e "  ${GREEN}Generated API Key: $API_KEY${NC}"
    echo -e "  ${YELLOW}IMPORTANT: Save this API key! It won't be shown again.${NC}"
else
    echo "  Configuration file already exists at $RAPTOR_HOME/.env"
fi

echo -e "${YELLOW}Step 4: Installing daemon binary...${NC}"

# Check if binary exists in current directory
if [ -f "./raptor-daemon" ]; then
    cp ./raptor-daemon "$RAPTOR_HOME/raptor-daemon"
    chown "$RAPTOR_USER:$RAPTOR_GROUP" "$RAPTOR_HOME/raptor-daemon"
    chmod 755 "$RAPTOR_HOME/raptor-daemon"
    echo "  Installed: $RAPTOR_HOME/raptor-daemon"
elif [ -f "./target/release/raptor-daemon" ]; then
    cp ./target/release/raptor-daemon "$RAPTOR_HOME/raptor-daemon"
    chown "$RAPTOR_USER:$RAPTOR_GROUP" "$RAPTOR_HOME/raptor-daemon"
    chmod 755 "$RAPTOR_HOME/raptor-daemon"
    echo "  Installed: $RAPTOR_HOME/raptor-daemon"
else
    echo -e "${YELLOW}  Warning: raptor-daemon binary not found in current directory${NC}"
    echo "  Please copy the binary to $RAPTOR_HOME/raptor-daemon manually"
fi

echo -e "${YELLOW}Step 5: Creating systemd service...${NC}"

cat > /etc/systemd/system/raptor-daemon.service << EOF
[Unit]
Description=Raptor Daemon - Game Server Management
Documentation=https://github.com/your-repo/raptor
After=network.target docker.service
Requires=docker.service

[Service]
Type=simple
User=$RAPTOR_USER
Group=$RAPTOR_GROUP
WorkingDirectory=$RAPTOR_HOME
EnvironmentFile=$RAPTOR_HOME/.env
ExecStart=$RAPTOR_HOME/raptor-daemon
ExecReload=/bin/kill -HUP \$MAINPID
Restart=always
RestartSec=5

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=$RAPTOR_DATA $RAPTOR_LOG $RAPTOR_HOME
PrivateTmp=true

# Logging
StandardOutput=append:$RAPTOR_LOG/daemon.log
StandardError=append:$RAPTOR_LOG/error.log

# Limits
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

echo "  Created: /etc/systemd/system/raptor-daemon.service"

# Reload systemd
systemctl daemon-reload
echo "  Reloaded systemd daemon"

echo -e "${YELLOW}Step 6: Setting up log rotation...${NC}"

cat > /etc/logrotate.d/raptor-daemon << EOF
$RAPTOR_LOG/*.log {
    daily
    missingok
    rotate 14
    compress
    delaycompress
    notifempty
    create 0640 $RAPTOR_USER $RAPTOR_GROUP
    sharedscripts
    postrotate
        systemctl reload raptor-daemon > /dev/null 2>&1 || true
    endscript
}
EOF

echo "  Created: /etc/logrotate.d/raptor-daemon"

echo ""
echo -e "${GREEN}=== Installation Complete ===${NC}"
echo ""
echo "Configuration:"
echo "  - User: $RAPTOR_USER"
echo "  - Home: $RAPTOR_HOME"
echo "  - Data: $RAPTOR_DATA"
echo "  - Logs: $RAPTOR_LOG"
echo "  - Config: $RAPTOR_HOME/.env"
echo ""
echo "Next steps:"
echo "  1. Edit the configuration file:"
echo "     sudo nano $RAPTOR_HOME/.env"
echo ""
echo "  2. (Optional) Set up TLS certificates:"
echo "     sudo cp /path/to/fullchain.pem $RAPTOR_HOME/certs/"
echo "     sudo cp /path/to/privkey.pem $RAPTOR_HOME/certs/"
echo "     sudo chown $RAPTOR_USER:$RAPTOR_GROUP $RAPTOR_HOME/certs/*"
echo "     Then uncomment TLS_CERT_PATH and TLS_KEY_PATH in .env"
echo ""
echo "  3. Start the daemon:"
echo "     sudo systemctl start raptor-daemon"
echo ""
echo "  4. Enable auto-start on boot:"
echo "     sudo systemctl enable raptor-daemon"
echo ""
echo "  5. Check status:"
echo "     sudo systemctl status raptor-daemon"
echo ""
echo "  6. View logs:"
echo "     sudo journalctl -u raptor-daemon -f"
echo "     # or"
echo "     sudo tail -f $RAPTOR_LOG/daemon.log"
echo ""
echo -e "${YELLOW}Note: The API key was shown above. Make sure to save it!${NC}"
echo ""
