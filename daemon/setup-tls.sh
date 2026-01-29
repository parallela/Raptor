#!/bin/bash
#
# Raptor Daemon - TLS Certificate Setup Script
# This script helps set up TLS certificates for HTTPS
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
CERT_DIR="$RAPTOR_HOME/certs"

echo -e "${GREEN}=== Raptor Daemon TLS Certificate Setup ===${NC}"
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}Error: This script must be run as root${NC}"
    echo "Please run: sudo $0 <cert_path> <key_path>"
    exit 1
fi

# Check arguments
if [ "$#" -lt 2 ]; then
    echo "Usage: $0 <certificate_path> <private_key_path>"
    echo ""
    echo "Example:"
    echo "  $0 /etc/letsencrypt/live/example.com/fullchain.pem /etc/letsencrypt/live/example.com/privkey.pem"
    echo ""
    echo "Or for ACME/acme.sh:"
    echo "  $0 /path/to/acme/domain/fullchain.cer /path/to/acme/domain/domain.key"
    exit 1
fi

CERT_SOURCE="$1"
KEY_SOURCE="$2"

# Validate source files
if [ ! -f "$CERT_SOURCE" ]; then
    echo -e "${RED}Error: Certificate file not found: $CERT_SOURCE${NC}"
    exit 1
fi

if [ ! -f "$KEY_SOURCE" ]; then
    echo -e "${RED}Error: Private key file not found: $KEY_SOURCE${NC}"
    exit 1
fi

# Create certs directory if it doesn't exist
mkdir -p "$CERT_DIR"

echo -e "${YELLOW}Copying certificates...${NC}"

# Copy certificates
cp "$CERT_SOURCE" "$CERT_DIR/fullchain.pem"
cp "$KEY_SOURCE" "$CERT_DIR/privkey.pem"

echo "  Copied: $CERT_SOURCE -> $CERT_DIR/fullchain.pem"
echo "  Copied: $KEY_SOURCE -> $CERT_DIR/privkey.pem"

# Set ownership and permissions
chown "$RAPTOR_USER:$RAPTOR_GROUP" "$CERT_DIR"/*
chmod 644 "$CERT_DIR/fullchain.pem"
chmod 600 "$CERT_DIR/privkey.pem"

echo "  Set ownership to $RAPTOR_USER:$RAPTOR_GROUP"
echo "  Set permissions: fullchain.pem (644), privkey.pem (600)"

echo -e "${YELLOW}Updating configuration...${NC}"

# Update .env file
ENV_FILE="$RAPTOR_HOME/.env"

if [ -f "$ENV_FILE" ]; then
    # Check if TLS settings already exist (commented or not)
    if grep -q "TLS_CERT_PATH" "$ENV_FILE"; then
        # Update existing entries
        sed -i "s|^#*\s*TLS_CERT_PATH=.*|TLS_CERT_PATH=$CERT_DIR/fullchain.pem|" "$ENV_FILE"
        sed -i "s|^#*\s*TLS_KEY_PATH=.*|TLS_KEY_PATH=$CERT_DIR/privkey.pem|" "$ENV_FILE"
        echo "  Updated TLS paths in $ENV_FILE"
    else
        # Add new entries
        echo "" >> "$ENV_FILE"
        echo "# TLS Configuration" >> "$ENV_FILE"
        echo "TLS_CERT_PATH=$CERT_DIR/fullchain.pem" >> "$ENV_FILE"
        echo "TLS_KEY_PATH=$CERT_DIR/privkey.pem" >> "$ENV_FILE"
        echo "  Added TLS paths to $ENV_FILE"
    fi
else
    echo -e "${YELLOW}  Warning: $ENV_FILE not found. Please add TLS paths manually.${NC}"
fi

echo ""
echo -e "${GREEN}=== TLS Setup Complete ===${NC}"
echo ""
echo "Certificate: $CERT_DIR/fullchain.pem"
echo "Private Key: $CERT_DIR/privkey.pem"
echo ""
echo "To apply the changes, restart the daemon:"
echo "  sudo systemctl restart raptor-daemon"
echo ""

# Create renewal script
RENEW_SCRIPT="$RAPTOR_HOME/renew-certs.sh"
cat > "$RENEW_SCRIPT" << EOF
#!/bin/bash
# Auto-generated certificate renewal script
# Source: $CERT_SOURCE, $KEY_SOURCE

cp "$CERT_SOURCE" "$CERT_DIR/fullchain.pem"
cp "$KEY_SOURCE" "$CERT_DIR/privkey.pem"
chown $RAPTOR_USER:$RAPTOR_GROUP "$CERT_DIR"/*
chmod 644 "$CERT_DIR/fullchain.pem"
chmod 600 "$CERT_DIR/privkey.pem"

# Reload daemon to pick up new certificates
systemctl reload raptor-daemon 2>/dev/null || systemctl restart raptor-daemon
EOF

chmod 755 "$RENEW_SCRIPT"
echo "Created renewal script: $RENEW_SCRIPT"
echo ""
echo "To set up automatic certificate renewal, add this to root's crontab:"
echo "  sudo crontab -e"
echo "  # Add this line:"
echo "  0 3 * * * $RENEW_SCRIPT"
echo ""
