# Raptor Daemon Installation Guide

The Raptor Daemon is a standalone service that runs on each host machine to manage Docker containers, provide FTP access, and report system resources to the central panel.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Installation](#quick-installation)
- [Configuration](#configuration)
- [TLS Setup](#tls-setup)
- [Service Management](#service-management)
- [Connecting to Panel](#connecting-to-panel)
- [Troubleshooting](#troubleshooting)

---

## Prerequisites

Before installing the daemon, ensure your system meets these requirements:

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| **OS** | Ubuntu 20.04, Debian 11, RHEL 8 | Ubuntu 22.04 |
| **Docker** | 20.10+ | Latest |
| **RAM** | 512 MB (daemon only) | 1 GB+ |
| **Disk** | 10 GB | Based on containers |
| **Ports** | 6969, 2121 | Open inbound |

### Install Docker (if not installed)

```bash
# Ubuntu/Debian
curl -fsSL https://get.docker.com | sh
sudo systemctl enable docker
sudo systemctl start docker

# Verify installation
docker --version
```

---

## Quick Installation

### Step 1: Download Files

```bash
# Create temporary directory
mkdir -p /tmp/raptor-install
cd /tmp/raptor-install

# Download the latest release
wget https://github.com/parallela/raptor/releases/latest/download/raptor-daemon-linux-amd64
wget https://github.com/parallela/raptor/releases/latest/download/install.sh

# Make executable
chmod +x raptor-daemon-linux-amd64 install.sh
```

### Step 2: Run Installer

```bash
sudo ./install.sh
```

**Save the API key displayed during installation!** You'll need it to connect to the panel.

### Step 3: Configure TLS (Required for Production)

```bash
# Copy your SSL certificates
sudo cp /path/to/fullchain.pem /opt/raptor/certs/
sudo cp /path/to/privkey.pem /opt/raptor/certs/
sudo chown raptor:raptor /opt/raptor/certs/*
sudo chmod 600 /opt/raptor/certs/privkey.pem

# Enable TLS in configuration
sudo nano /opt/raptor/.env
# Uncomment TLS_CERT_PATH and TLS_KEY_PATH
```

### Step 4: Start the Service

```bash
sudo systemctl enable raptor-daemon
sudo systemctl start raptor-daemon
sudo systemctl status raptor-daemon
```

---

## Configuration

### Environment Variables

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `DAEMON_API_KEY` | Authentication key for API | - | ✅ Yes |
| `DAEMON_ADDR` | Listen address:port | `0.0.0.0:6969` | No |
| `DOCKER_HOST` | Docker socket path | `unix:///var/run/docker.sock` | No |
| `DAEMON_DATA_DIR` | State files directory | `/var/lib/raptor/sys` | No |
| `FTP_BASE_PATH` | Container volumes path | `/var/lib/raptor` | No |
| `FTP_HOST` | FTP bind address | `0.0.0.0` | No |
| `FTP_PORT` | FTP server port | `2121` | No |
| `AVAILABLE_IPS` | IPs for allocation | `0.0.0.0` | No |
| `TLS_CERT_PATH` | SSL certificate path | - | For HTTPS |
| `TLS_KEY_PATH` | SSL private key path | - | For HTTPS |
| `RUST_LOG` | Log level | `info` | No |

---

## TLS Setup

TLS is **required** for production deployments. The API communicates over HTTPS/WSS.

### Option 1: Using Let's Encrypt (Standalone)

```bash
# Install certbot
sudo apt install certbot

# Obtain certificate (standalone mode)
sudo certbot certonly --standalone -d daemon.yourdomain.com

# Copy to raptor directory
sudo cp /etc/letsencrypt/live/daemon.yourdomain.com/fullchain.pem /opt/raptor/certs/
sudo cp /etc/letsencrypt/live/daemon.yourdomain.com/privkey.pem /opt/raptor/certs/
sudo chown raptor:raptor /opt/raptor/certs/*
sudo chmod 600 /opt/raptor/certs/privkey.pem
```

### Option 2: Using Your Own Nginx Reverse Proxy

If you prefer to use your own nginx instance on the physical machine (instead of TLS directly in the daemon), you can configure nginx as a reverse proxy with SSL termination.

#### Step 1: Install Nginx

```bash
sudo apt install nginx
```

#### Step 2: Obtain SSL Certificate

```bash
# Using certbot with nginx plugin
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d daemon.yourdomain.com
```

#### Step 3: Configure Nginx

Create `/etc/nginx/sites-available/raptor-daemon`:

```nginx
upstream raptor_daemon {
    server 127.0.0.1:6969;
}

server {
    listen 443 ssl http2;
    server_name daemon.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/daemon.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/daemon.yourdomain.com/privkey.pem;
    
    # SSL settings
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256;
    ssl_prefer_server_ciphers off;

    # Proxy settings
    location / {
        proxy_pass http://raptor_daemon;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # WebSocket support
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 86400;
    }
}

server {
    listen 80;
    server_name daemon.yourdomain.com;
    return 301 https://$server_name$request_uri;
}
```

#### Step 4: Enable the Site

```bash
sudo ln -s /etc/nginx/sites-available/raptor-daemon /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

#### Step 5: Configure Daemon (No TLS)

When using nginx as reverse proxy, the daemon runs without TLS. Edit `/opt/raptor/.env`:

```bash
# Keep TLS paths commented out
# TLS_CERT_PATH=/opt/raptor/certs/fullchain.pem
# TLS_KEY_PATH=/opt/raptor/certs/privkey.pem

# Bind only to localhost since nginx handles external traffic
DAEMON_ADDR=127.0.0.1:6969
```

#### Step 6: Update Panel Daemon Settings

In the panel, when adding this daemon:
- **Host**: `daemon.yourdomain.com`
- **Port**: `443`
- **Secure**: `Yes`

The panel will connect via nginx (port 443), which proxies to the daemon (port 6969).

---

## Service Management

```bash
# Start the daemon
sudo systemctl start raptor-daemon

# Stop the daemon
sudo systemctl stop raptor-daemon

# Restart the daemon
sudo systemctl restart raptor-daemon

# Check status
sudo systemctl status raptor-daemon

# View logs
sudo tail -f /var/log/raptor/daemon.log
```

---

## Connecting to Panel

1. Get your API key: `sudo grep DAEMON_API_KEY /opt/raptor/.env`
2. In the panel, go to **Admin** → **Daemons** → **Add Daemon**
3. Enter host, port (6969), and API key
4. Click **Save**

---

## Troubleshooting

### Daemon Won't Start

```bash
sudo journalctl -u raptor-daemon -n 50 --no-pager
sudo -u raptor docker ps
```

### Permission Issues

```bash
sudo usermod -aG docker raptor
sudo systemctl restart raptor-daemon
```

---

## Support

- **GitHub Issues**: https://github.com/parallela/raptor/issues
