# Raptor Daemon Installation Guide

This guide covers the installation and configuration of the Raptor Daemon on a Linux server.

## Prerequisites

- **Linux server** (Ubuntu 20.04+, Debian 11+, or similar)
- **Docker** installed and running
- **Root access** (sudo)
- **Open ports**: 
  - `6969` (Daemon API - HTTPS)
  - `2121` (FTP server)

## Quick Installation

### 1. Download the Installation Files

```bash
# Create a temporary directory
mkdir -p /tmp/raptor-install
cd /tmp/raptor-install

# Download the daemon binary and installation scripts
# (Replace with your actual download URL)
wget https://your-server.com/releases/raptor-daemon
wget https://your-server.com/releases/install.sh
wget https://your-server.com/releases/uninstall.sh

# Or copy from your build machine
scp raptor-daemon install.sh uninstall.sh user@server:/tmp/raptor-install/
```

### 2. Run the Installation Script

```bash
cd /tmp/raptor-install
chmod +x install.sh raptor-daemon
sudo ./install.sh
```

The installation script will:
- Create a dedicated `raptor` system user and group
- Add `raptor` user to the `docker` group
- Create required directories:
  - `/opt/raptor` - Application home
  - `/var/lib/raptor` - Data storage (volumes, state)
  - `/var/log/raptor` - Log files
- Generate a default configuration file
- Generate a random API key
- Install the systemd service
- Set up log rotation

**Important:** Save the generated API key displayed during installation!

### 3. Configure the Daemon

Edit the configuration file:

```bash
sudo nano /opt/raptor/.env
```

Configuration options:

```bash
# API Key for authentication (generated during install, or set your own)
DAEMON_API_KEY=your-api-key-here

# Daemon address and port
DAEMON_ADDR=0.0.0.0:6969

# Docker socket
DOCKER_HOST=unix:///var/run/docker.sock

# Data directories
DAEMON_DATA_DIR=/var/lib/raptor/sys
FTP_BASE_PATH=/var/lib/raptor

# FTP Server
FTP_HOST=0.0.0.0
FTP_PORT=2121

# Available IPs for container allocation
AVAILABLE_IPS=0.0.0.0

# TLS Configuration (uncomment after setting up certificates)
# TLS_CERT_PATH=/opt/raptor/certs/fullchain.pem
# TLS_KEY_PATH=/opt/raptor/certs/privkey.pem

# Logging level (debug, info, warn, error)
RUST_LOG=info
```

### 4. Set Up TLS Certificates (Recommended)

For HTTPS support, you need SSL/TLS certificates:

```bash
# Copy your certificates
sudo cp /path/to/fullchain.pem /opt/raptor/certs/fullchain.pem
sudo cp /path/to/privkey.pem /opt/raptor/certs/privkey.pem

# Set correct ownership and permissions
sudo chown raptor:raptor /opt/raptor/certs/*
sudo chmod 644 /opt/raptor/certs/fullchain.pem
sudo chmod 600 /opt/raptor/certs/privkey.pem

# Enable TLS in configuration
sudo nano /opt/raptor/.env
# Uncomment these lines:
# TLS_CERT_PATH=/opt/raptor/certs/fullchain.pem
# TLS_KEY_PATH=/opt/raptor/certs/privkey.pem
```

#### Using Let's Encrypt with ACME

If using acme.sh or similar:

```bash
# Example for acme.sh certificates
sudo cp /path/to/acme/domain.com/fullchain.cer /opt/raptor/certs/fullchain.pem
sudo cp /path/to/acme/domain.com/domain.com.key /opt/raptor/certs/privkey.pem
sudo chown raptor:raptor /opt/raptor/certs/*
sudo chmod 644 /opt/raptor/certs/fullchain.pem
sudo chmod 600 /opt/raptor/certs/privkey.pem
```

#### Automatic Certificate Renewal

Create a renewal script:

```bash
sudo tee /opt/raptor/renew-certs.sh << 'EOF'
#!/bin/bash
# Update paths to match your certificate location
SOURCE_CERT="/path/to/acme/domain.com/fullchain.cer"
SOURCE_KEY="/path/to/acme/domain.com/domain.com.key"

cp "$SOURCE_CERT" /opt/raptor/certs/fullchain.pem
cp "$SOURCE_KEY" /opt/raptor/certs/privkey.pem
chown raptor:raptor /opt/raptor/certs/*
chmod 644 /opt/raptor/certs/fullchain.pem
chmod 600 /opt/raptor/certs/privkey.pem
systemctl restart raptor-daemon
EOF

sudo chmod +x /opt/raptor/renew-certs.sh
```

Add to root's crontab (runs weekly on Sunday at 3 AM):

```bash
sudo crontab -e
# Add this line:
0 3 * * 0 /opt/raptor/renew-certs.sh
```

### 5. Start the Daemon

```bash
# Start the service
sudo systemctl start raptor-daemon

# Enable auto-start on boot
sudo systemctl enable raptor-daemon

# Check status
sudo systemctl status raptor-daemon
```

### 6. Verify Installation

```bash
# Check if the daemon is running
sudo systemctl status raptor-daemon

# View logs
sudo journalctl -u raptor-daemon -f

# Or check the log files
sudo tail -f /var/log/raptor/daemon.log

# Test the API (replace with your API key)
curl -k -H "Authorization: Bearer your-api-key" https://localhost:6969/health
```

---

## Directory Structure

After installation:

```
/opt/raptor/
├── raptor-daemon          # Main binary
├── .env                   # Configuration file
├── certs/                 # TLS certificates
│   ├── fullchain.pem
│   └── privkey.pem
└── renew-certs.sh         # Certificate renewal script (if created)

/var/lib/raptor/
├── volumes/               # Container data volumes
│   └── {container-uuid}/  # Per-container storage
└── sys/
    ├── containers.json    # Container state
    └── ftp_credentials.json

/var/log/raptor/
├── daemon.log             # Application logs
└── error.log              # Error logs
```

---

## Uninstallation

To remove the Raptor Daemon:

```bash
# Copy uninstall script if not already present
chmod +x uninstall.sh
sudo ./uninstall.sh
```

The uninstall script will:
- Stop and disable the systemd service
- Remove the systemd service file
- Remove log rotation configuration
- Remove `/opt/raptor` directory
- Remove `/var/log/raptor` directory
- Optionally remove `/var/lib/raptor` (data directory)
- Remove the `raptor` user and group

**Note:** You will be prompted whether to keep the data directory (container volumes and state).

### Manual Uninstallation

If the uninstall script is not available:

```bash
# Stop and disable service
sudo systemctl stop raptor-daemon
sudo systemctl disable raptor-daemon

# Remove systemd service
sudo rm /etc/systemd/system/raptor-daemon.service
sudo systemctl daemon-reload

# Remove directories
sudo rm -rf /opt/raptor
sudo rm -rf /var/log/raptor
# Optional: remove data (container volumes)
# sudo rm -rf /var/lib/raptor

# Remove logrotate config
sudo rm /etc/logrotate.d/raptor-daemon

# Remove user and group
sudo userdel raptor
sudo groupdel raptor
```

---

## Troubleshooting

### Daemon won't start

1. Check logs:
   ```bash
   sudo journalctl -u raptor-daemon -n 50
   ```

2. Verify Docker is running:
   ```bash
   sudo systemctl status docker
   ```

3. Check if raptor user is in docker group:
   ```bash
   groups raptor
   ```

4. Verify configuration file:
   ```bash
   sudo cat /opt/raptor/.env
   ```

### Permission denied errors

Ensure the raptor user has access to Docker:

```bash
sudo usermod -aG docker raptor
sudo systemctl restart raptor-daemon
```

### TLS certificate errors

1. Verify certificate files exist and are readable:
   ```bash
   sudo ls -la /opt/raptor/certs/
   ```

2. Check ownership and permissions:
   ```bash
   sudo chown raptor:raptor /opt/raptor/certs/*
   sudo chmod 644 /opt/raptor/certs/fullchain.pem
   sudo chmod 600 /opt/raptor/certs/privkey.pem
   ```

3. Verify certificate validity:
   ```bash
   openssl x509 -in /opt/raptor/certs/fullchain.pem -noout -dates
   ```

### Container volume permission issues

The daemon automatically sets permissions on volume directories. If issues persist:

```bash
# Ensure volumes directory has correct permissions
sudo chmod 777 /var/lib/raptor/volumes
```

---

## Updating

To update the daemon:

```bash
# Stop the service
sudo systemctl stop raptor-daemon

# Backup current binary (optional)
sudo cp /opt/raptor/raptor-daemon /opt/raptor/raptor-daemon.bak

# Copy new binary
sudo cp /path/to/new/raptor-daemon /opt/raptor/raptor-daemon
sudo chown raptor:raptor /opt/raptor/raptor-daemon
sudo chmod 755 /opt/raptor/raptor-daemon

# Start the service
sudo systemctl start raptor-daemon

# Verify
sudo systemctl status raptor-daemon
```

---

## Security Recommendations

1. **Use strong API keys** - Generate long, random API keys
2. **Enable TLS** - Always use HTTPS in production
3. **Firewall** - Only expose necessary ports (6969, 2121)
4. **Regular updates** - Keep the daemon and Docker updated
5. **Log monitoring** - Monitor logs for suspicious activity
