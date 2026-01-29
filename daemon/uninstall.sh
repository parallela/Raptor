#!/bin/bash
#
# Raptor Daemon Uninstallation Script
# This script removes the raptor user, directories, and systemd service
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration (must match install.sh)
RAPTOR_USER="raptor"
RAPTOR_GROUP="raptor"
RAPTOR_HOME="/opt/raptor"
RAPTOR_DATA="/var/lib/raptor"
RAPTOR_LOG="/var/log/raptor"

echo -e "${RED}=== Raptor Daemon Uninstallation ===${NC}"
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}Error: This script must be run as root${NC}"
    echo "Please run: sudo $0"
    exit 1
fi

echo -e "${YELLOW}Warning: This will remove the raptor daemon and all its data!${NC}"
read -p "Are you sure you want to continue? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 0
fi

read -p "Do you want to keep the data (volumes, container state)? (Y/n) " -n 1 -r
echo
KEEP_DATA=true
if [[ $REPLY =~ ^[Nn]$ ]]; then
    KEEP_DATA=false
fi

echo -e "${YELLOW}Step 1: Stopping and disabling service...${NC}"

if systemctl is-active --quiet raptor-daemon 2>/dev/null; then
    systemctl stop raptor-daemon
    echo "  Stopped raptor-daemon service"
fi

if systemctl is-enabled --quiet raptor-daemon 2>/dev/null; then
    systemctl disable raptor-daemon
    echo "  Disabled raptor-daemon service"
fi

echo -e "${YELLOW}Step 2: Removing systemd service...${NC}"

if [ -f /etc/systemd/system/raptor-daemon.service ]; then
    rm /etc/systemd/system/raptor-daemon.service
    echo "  Removed: /etc/systemd/system/raptor-daemon.service"
fi

systemctl daemon-reload
echo "  Reloaded systemd daemon"

echo -e "${YELLOW}Step 3: Removing logrotate configuration...${NC}"

if [ -f /etc/logrotate.d/raptor-daemon ]; then
    rm /etc/logrotate.d/raptor-daemon
    echo "  Removed: /etc/logrotate.d/raptor-daemon"
fi

echo -e "${YELLOW}Step 4: Removing directories...${NC}"

if [ -d "$RAPTOR_HOME" ]; then
    rm -rf "$RAPTOR_HOME"
    echo "  Removed: $RAPTOR_HOME"
fi

if [ -d "$RAPTOR_LOG" ]; then
    rm -rf "$RAPTOR_LOG"
    echo "  Removed: $RAPTOR_LOG"
fi

if [ "$KEEP_DATA" = false ]; then
    if [ -d "$RAPTOR_DATA" ]; then
        rm -rf "$RAPTOR_DATA"
        echo "  Removed: $RAPTOR_DATA"
    fi
else
    echo -e "  ${GREEN}Kept data directory: $RAPTOR_DATA${NC}"
fi

echo -e "${YELLOW}Step 5: Removing user and group...${NC}"

if id "$RAPTOR_USER" > /dev/null 2>&1; then
    userdel "$RAPTOR_USER"
    echo "  Removed user: $RAPTOR_USER"
fi

if getent group "$RAPTOR_GROUP" > /dev/null 2>&1; then
    groupdel "$RAPTOR_GROUP"
    echo "  Removed group: $RAPTOR_GROUP"
fi

echo ""
echo -e "${GREEN}=== Uninstallation Complete ===${NC}"
echo ""

if [ "$KEEP_DATA" = true ]; then
    echo "Data directory was kept at: $RAPTOR_DATA"
    echo "To remove it manually: sudo rm -rf $RAPTOR_DATA"
fi
echo ""
