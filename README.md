<p align="center">
  <img src="panel/static/logo.webp" alt="Raptor Logo" width="400" />
</p>

<h1 align="center">Raptor</h1>

<p align="center">
  <strong>A Modern, Self-Hosted Game Server Management Platform</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.75+-orange?style=flat-square&logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/SvelteKit-2.0-FF3E00?style=flat-square&logo=svelte" alt="SvelteKit" />
  <img src="https://img.shields.io/badge/Docker-Ready-2496ED?style=flat-square&logo=docker" alt="Docker" />
  <img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="License" />
</p>

<p align="center">
  A lightweight, high-performance alternative to Pterodactyl Panel.<br/>
  Engineered with Rust for exceptional throughput and SvelteKit for a modern, reactive user interface.
</p>

---

## ✨ Why Raptor?

| Feature | Raptor | Pterodactyl |
|---------|--------|-------------|
| **Runtime** | Rust (native compilation) | PHP (interpreted) |
| **Memory Footprint** | ~30MB | ~150MB+ |
| **Frontend Framework** | SvelteKit (reactive, compiled) | Vue.js 2 |
| **Deployment** | Single binary + containerized services | Complex multi-component PHP stack |
| **Database** | PostgreSQL | MySQL/MariaDB |
| **Real-time Communication** | Native WebSocket implementation | Pusher/WebSocket bridge |
| **Responsive Design** | Full mobile optimization | Limited mobile support |

### 🚀 Key Features

- **🔥 High Performance** - Implemented in Rust with asynchronous I/O via Tokio runtime for non-blocking concurrent operations
- **📱 Responsive Interface** - Mobile-first design principles with adaptive layouts across all viewport sizes
- **🔒 Security-First Architecture** - JWT-based stateless authentication, TLS encryption, RBAC authorization
- **🐳 Native Docker Integration** - Direct container orchestration via Docker Engine API (Bollard client)
- **📊 Real-time Telemetry** - Live resource monitoring with sub-second latency via persistent WebSocket connections
- **📁 Integrated File Management** - Browser-based file operations with chunked upload support for large files
- **🔌 SFTP/FTP Access** - Per-container isolated FTP credentials with chroot-jailed file system access
- **🗃️ Multi-Database Support** - Managed PostgreSQL, MySQL, and Redis instances with automated provisioning
- **🎮 Template System** - Flakes: declarative server templates with variable interpolation and installation scripts
- **🌐 Distributed Architecture** - Horizontal scaling with multiple daemon nodes across geographic regions

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         NGINX PROXY                              │
│              (TLS Termination, Load Balancing)                   │
└───────────────────────────┬─────────────────────────────────────┘
                            │
            ┌───────────────┴───────────────┐
            ▼                               ▼
    ┌───────────────┐               ┌───────────────┐
    │  Raptor Panel │               │  Raptor API   │
    │   (SvelteKit) │──────────────▶│    (Rust)     │
    │   Port: 3000  │   HTTP/WS     │   Port: 3000  │
    └───────────────┘               └───────┬───────┘
                                            │
                                            │ PostgreSQL Wire Protocol
                                            ▼
                                    ┌───────────────┐
                                    │   Database    │
                                    │  (Postgres)   │
                                    └───────────────┘
                                            
    ════════════════════════════════════════════════════════════
    
                            HTTPS/WSS (Port 6969)
                                    │
            ┌───────────────────────┼───────────────────────┐
            ▼                       ▼                       ▼
    ┌───────────────┐       ┌───────────────┐       ┌───────────────┐
    │ Raptor Daemon │       │ Raptor Daemon │       │ Raptor Daemon │
    │    Host 1     │       │    Host 2     │       │    Host N     │
    │  + FTP :2121  │       │  + FTP :2121  │       │  + FTP :2121  │
    └───────┬───────┘       └───────┬───────┘       └───────┬───────┘
            │                       │                       │
            ▼                       ▼                       ▼
    ┌───────────────┐       ┌───────────────┐       ┌───────────────┐
    │    Docker     │       │    Docker     │       │    Docker     │
    │  Containers   │       │  Containers   │       │  Containers   │
    └───────────────┘       └───────────────┘       └───────────────┘
```

### Components

| Component | Description | Technology Stack |
|-----------|-------------|------------------|
| **Panel** | Server-side rendered web application with reactive UI | SvelteKit, TailwindCSS, TypeScript |
| **API** | RESTful backend with WebSocket support for real-time events | Rust, Axum, SQLx, Tokio |
| **Daemon** | Host-level agent for container lifecycle management | Rust, Bollard (Docker API), libunftp |
| **Database** | Persistent storage with ACID compliance | PostgreSQL 16 |

### Technical Highlights

- **Asynchronous Runtime**: All network I/O operations are non-blocking, utilizing Tokio's multi-threaded scheduler for optimal CPU utilization
- **Zero-Copy Serialization**: Efficient JSON serialization with Serde for minimal memory allocation overhead
- **Connection Pooling**: Database connections managed via SQLx connection pool with configurable limits
- **Graceful Shutdown**: Proper signal handling ensures in-flight requests complete before termination
- **Structured Logging**: Tracing-based observability with configurable log levels and structured output

---

## 📦 Installation

### Prerequisites

- **Docker Engine** 20.10+ with Docker Compose V2
- A domain name with DNS A records configured
- **nginx-proxy** with automated Let's Encrypt (recommended for production TLS termination)

### Option 1: Development Environment

```bash
git clone https://github.com/parallela/raptor.git
cd raptor
docker compose up -d
```

Access the panel at `http://localhost:5173` (credentials: admin/admin123)

### Option 2: Production Deployment

#### Step 1: Configure nginx-proxy (if not already provisioned)

```bash
# Create the external proxy network
docker network create nginx-proxy

# Deploy nginx-proxy with automatic certificate management
docker run -d \
  --name nginx-proxy \
  --restart always \
  -p 80:80 \
  -p 443:443 \
  -v /var/run/docker.sock:/tmp/docker.sock:ro \
  -v certs:/etc/nginx/certs \
  -v vhost:/etc/nginx/vhost.d \
  -v html:/usr/share/nginx/html \
  --network nginx-proxy \
  jwilder/nginx-proxy

docker run -d \
  --name nginx-proxy-letsencrypt \
  --restart always \
  -v /var/run/docker.sock:/var/run/docker.sock:ro \
  -v certs:/etc/nginx/certs \
  -v vhost:/etc/nginx/vhost.d \
  -v html:/usr/share/nginx/html \
  --volumes-from nginx-proxy \
  -e DEFAULT_EMAIL=your-email@example.com \
  jrcs/letsencrypt-nginx-proxy-companion
```

**Important:** Configure nginx for large request bodies (required for chunked file uploads):

```bash
# Create virtual host configuration for increased body size limit
docker exec nginx-proxy mkdir -p /etc/nginx/vhost.d
docker exec nginx-proxy bash -c 'echo "client_max_body_size 100m;" > /etc/nginx/vhost.d/api-raptor.yourdomain.com'
docker restart nginx-proxy
```

#### Step 2: Create deployment directory

```bash
mkdir -p ~/raptor-service
cd ~/raptor-service
```

#### Step 3: Create docker-compose.yml

```yaml
version: '3.8'

services:
  postgres:
    image: postgres:16-alpine
    container_name: raptor-postgres
    environment:
      POSTGRES_USER: raptor
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-your_secure_password}
      POSTGRES_DB: raptor
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U raptor"]
      interval: 10s
      timeout: 5s
      retries: 5
    restart: unless-stopped
    networks:
      - raptor-network

  api:
    image: artifacts.lstan.eu/raptor-api:latest
    container_name: raptor-api
    env_file:
      - .env.production
    depends_on:
      postgres:
        condition: service_healthy
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
    networks:
      - nginx-proxy
      - raptor-network

  panel:
    image: artifacts.lstan.eu/raptor-panel:latest
    container_name: raptor-panel
    env_file:
      - .env.frontend
    depends_on:
      - api
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "wget", "-q", "--spider", "http://localhost:3000"]
      interval: 30s
      timeout: 10s
      retries: 3
    networks:
      - nginx-proxy
      - raptor-network

volumes:
  postgres_data:

networks:
  nginx-proxy:
    external: true
  raptor-network:
    driver: bridge
```

#### Step 4: Create environment configuration files

**.env.production** (API configuration):

```bash
# Database Connection
DATABASE_URL=postgres://raptor:your_secure_password@postgres:5432/raptor

# Authentication
JWT_SECRET=your_random_64_char_hex_string
JWT_EXPIRY_DAYS=7
BCRYPT_COST=12

# Server Configuration
API_ADDR=0.0.0.0:3000
APP_URL=https://raptor.yourdomain.com
RUST_LOG=info

# Initial Administrator (provisioned on first startup)
ADMIN_USERNAME=admin
ADMIN_EMAIL=admin@yourdomain.com
ADMIN_PASSWORD=your_secure_admin_password

# SMTP Configuration (optional - for email notifications)
# SMTP_HOST=smtp.example.com
# SMTP_PORT=587
# SMTP_USERNAME=your_smtp_user
# SMTP_PASSWORD=your_smtp_password
# SMTP_FROM_EMAIL=raptor@yourdomain.com
# SMTP_FROM_NAME=Raptor

# nginx-proxy Integration (automatic TLS provisioning)
VIRTUAL_HOST=api-raptor.yourdomain.com
LETSENCRYPT_HOST=api-raptor.yourdomain.com
LETSENCRYPT_EMAIL=admin@yourdomain.com
```

Generate a cryptographically secure JWT secret:
```bash
openssl rand -hex 32
```

**.env.frontend** (Panel configuration):

```bash
# API Endpoint (must be publicly accessible)
API_URL=https://api-raptor.yourdomain.com

# SvelteKit Runtime Configuration
ORIGIN=https://raptor.yourdomain.com
BODY_SIZE_LIMIT=Infinity

# nginx-proxy Integration
VIRTUAL_HOST=raptor.yourdomain.com
LETSENCRYPT_HOST=raptor.yourdomain.com
LETSENCRYPT_EMAIL=admin@yourdomain.com
```

#### Step 5: Deploy services

```bash
docker compose up -d
```

#### Step 6: Verify deployment

```bash
# Verify all services are healthy
docker compose ps

# Stream aggregated logs
docker compose logs -f

# Validate API endpoint
curl https://api-raptor.yourdomain.com/health
```

---

## 🖥️ Daemon Installation

The daemon executes **directly on the host system** (bare-metal or VM) to interface with the Docker Engine and manage system resources.

### Automated Installation

```bash
# Retrieve the daemon binary
wget https://github.com/parallela/raptor/releases/latest/download/raptor-daemon-linux-amd64 -O /tmp/raptor-daemon
chmod +x /tmp/raptor-daemon

# Execute the installation script
wget https://github.com/parallela/raptor/releases/latest/download/install.sh -O /tmp/install.sh
chmod +x /tmp/install.sh
sudo /tmp/install.sh
```

### Installation Script Operations

1. **System User Provisioning** - Creates dedicated `raptor` user and group with restricted privileges
2. **Directory Structure**:
   - `/opt/raptor` - Application binaries and configuration
   - `/var/lib/raptor/volumes` - Container persistent storage (bind mounts)
   - `/var/lib/raptor/sys` - Daemon state persistence (JSON)
   - `/var/log/raptor` - Application logs
3. **API Key Generation** - Cryptographically random UUID for daemon authentication
4. **Systemd Integration** - Service unit file with automatic restart policies
5. **Log Rotation** - Logrotate configuration for log lifecycle management

### TLS Configuration (Required for Production)

```bash
# Deploy SSL/TLS certificates
sudo cp /path/to/fullchain.pem /opt/raptor/certs/
sudo cp /path/to/privkey.pem /opt/raptor/certs/
sudo chown raptor:raptor /opt/raptor/certs/*
sudo chmod 600 /opt/raptor/certs/privkey.pem

# Update daemon configuration
sudo nano /opt/raptor/.env
```

Enable TLS by uncommenting:
```bash
TLS_CERT_PATH=/opt/raptor/certs/fullchain.pem
TLS_KEY_PATH=/opt/raptor/certs/privkey.pem
```

### Alternative: Nginx Reverse Proxy

For environments preferring centralized TLS termination, configure nginx as a reverse proxy:

```nginx
upstream raptor_daemon {
    server 127.0.0.1:6969;
}

server {
    listen 443 ssl http2;
    server_name daemon.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/daemon.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/daemon.yourdomain.com/privkey.pem;
    
    location / {
        proxy_pass http://raptor_daemon;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # WebSocket upgrade support
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 86400;
    }
}
```

### Service Management

```bash
# Enable service persistence across reboots
sudo systemctl enable raptor-daemon

# Start the daemon process
sudo systemctl start raptor-daemon

# Verify operational status
sudo systemctl status raptor-daemon

# Monitor real-time log output
sudo tail -f /var/log/raptor/daemon.log
```

### Register Daemon with Panel

1. Navigate to **Admin** → **Daemons** → **Add Daemon**
2. Configure connection parameters:
   - **Name**: Descriptive identifier (e.g., "US-East-1", "EU-Frankfurt")
   - **Host**: FQDN or IP address of the daemon host
   - **Port**: 6969 (default daemon port)
   - **Secure**: Enable for TLS-encrypted connections
   - **API Key**: Retrieve from `/opt/raptor/.env`
3. Click **Save** to establish connection

### Daemon Configuration Reference

| Environment Variable | Description | Default Value |
|---------------------|-------------|---------------|
| `DAEMON_API_KEY` | Authentication token for API requests | Auto-generated |
| `DAEMON_ADDR` | Socket bind address and port | `0.0.0.0:6969` |
| `FTP_HOST` | FTP server bind address | `0.0.0.0` |
| `FTP_PORT` | FTP server listening port | `2121` |
| `AVAILABLE_IPS` | Comma-separated list of allocatable IPs | `0.0.0.0` |
| `TLS_CERT_PATH` | Path to TLS certificate chain | - |
| `TLS_KEY_PATH` | Path to TLS private key | - |
| `RUST_LOG` | Logging verbosity (trace, debug, info, warn, error) | `info` |

### Firewall Rules

Configure ingress rules for required ports:

```bash
# UFW (Ubuntu)
sudo ufw allow 6969/tcp  # Daemon API/WebSocket
sudo ufw allow 2121/tcp  # FTP control channel
sudo ufw allow 21000:21100/tcp  # FTP passive data ports

# iptables
sudo iptables -A INPUT -p tcp --dport 6969 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 2121 -j ACCEPT
```

---

## 🎮 Flakes (Server Templates)

Flakes provide declarative server configuration templates with variable interpolation.

### Bundled Flakes

| Template | Container Image | Capabilities |
|----------|-----------------|--------------|
| **Minecraft (Paper)** | `artifacts.lstan.eu/java:21` | Automated JAR retrieval, plugin support |
| **Minecraft (Vanilla)** | `artifacts.lstan.eu/java:21` | EULA auto-acceptance |
| **Hytale** | `ghcr.io/pterodactyl/games:hytale` | Auto-updater, PTY allocation |
| **Node.js** | `artifacts.lstan.eu/nodejs:20` | Generic Node.js runtime |

### Custom Flake Definition

Navigate to **Admin** → **Flakes** → **Import Flake** and upload a JSON schema:

```json
{
  "name": "Custom Game Server",
  "slug": "custom-server",
  "description": "Template description for administrative reference",
  "dockerImage": "your-registry/image:tag",
  "startupCommand": "java -Xms{{SERVER_MEMORY}}M -Xmx{{SERVER_MEMORY}}M -jar server.jar",
  "stopCommand": "stop",
  "installScript": "#!/bin/bash\nwget -O server.jar https://...",
  "tty": false,
  "variables": [
    {
      "name": "Server Memory",
      "description": "JVM heap allocation in megabytes",
      "envVariable": "SERVER_MEMORY",
      "defaultValue": "1024",
      "userViewable": true,
      "userEditable": true,
      "rules": "required|numeric|min:512"
    }
  ]
}
```

### Variable Interpolation

Template variables use double-brace syntax `{{VARIABLE_NAME}}`:

- `{{SERVER_MEMORY}}` - Allocated memory from resource limits
- `{{SERVER_PORT}}` - Primary network allocation port
- `{{SERVER_IP}}` - Bound IP address
- Custom variables defined in the flake schema

---

## 🗄️ Database Management

Raptor provides managed database instances for PostgreSQL, MySQL, and Redis.

### Administrative Setup

1. Navigate to **Admin** → **Database Servers**
2. Click **Add Database Server**
3. Configure instance parameters:
   - **Type**: PostgreSQL, MySQL, or Redis
   - **Daemon**: Target host for container deployment
   - **Container Name**: Unique identifier
   - **Root Password**: Administrative credentials
4. Click **Create** to provision the database container

### User Database Provisioning

1. Navigate to **Databases** in the sidebar
2. Click **Create Database**
3. Select from available database server types
4. Credentials are automatically generated and displayed

**Redis Isolation Note**: Redis instances implement key-prefix isolation. All operations are automatically scoped to `{database_name}:` prefix.

---

## 🔐 Security Model

### Role-Based Access Control (RBAC)

| Role | Permission Scope |
|------|------------------|
| **Admin** | Full system administration privileges |
| **Manager** | User and resource management (no system config) |
| **User** | Self-service container management only |

### Container-Level Authorization

- **Owner** - Full control including deletion
- **Admin** - Configuration and management access
- **User** - Operational access (start/stop, console)

### Security Implementation

- ✅ Stateless JWT authentication with configurable expiration
- ✅ TLS 1.2+ encryption for all network communication
- ✅ Chroot-jailed FTP access per container
- ✅ Persistent WebSocket connections with token validation
- ✅ CORS policy enforcement
- ✅ Request payload validation and sanitization

---

## 📱 Mobile Interface

Raptor implements a fully responsive design system:

- Collapsible navigation with touch-optimized controls
- Adaptive grid layouts for varying viewport dimensions
- Compact resource monitoring widgets
- Mobile-optimized terminal with dedicated input controls
- Touch-friendly file management interface

---

## 🛠️ Building from Source

### API Service

```bash
cd api
cargo build --release
# Output: target/release/raptor-api
```

### Daemon Service

```bash
cd daemon
# Cross-compilation for Linux (from macOS)
cargo build --release --target x86_64-unknown-linux-gnu
# Output: target/x86_64-unknown-linux-gnu/release/raptor-daemon
```

### Panel Application

```bash
cd panel
npm install
npm run build
# Output: build/
```

### Container Images

```bash
# Build API container image
docker build -t raptor-api:latest -f api/Dockerfile .

# Build Panel container image  
docker build -t raptor-panel:latest -f panel/Dockerfile .

# Multi-architecture build (linux/amd64)
docker buildx build --platform linux/amd64 -t raptor-api:latest -f api/Dockerfile .
```

---

## 📚 Troubleshooting

### Daemon Startup Failures

```bash
# Inspect systemd journal
sudo journalctl -u raptor-daemon -f

# Review application logs
sudo tail -f /var/log/raptor/daemon.log

# Verify Docker socket access
sudo -u raptor docker ps

# Audit filesystem permissions
ls -la /var/lib/raptor/
ls -la /opt/raptor/certs/
```

### Container Provisioning Errors

1. Verify daemon connectivity status on the Daemons page
2. Confirm IP allocations are available for the target daemon
3. Validate Docker image accessibility

### WebSocket Connection Failures

1. Verify nginx-proxy WebSocket upgrade configuration
2. Validate TLS certificate chain integrity
3. Confirm firewall permits traffic on port 6969

### File Upload Failures

Configure nginx for increased request body limits:
```bash
docker exec nginx-proxy bash -c 'echo "client_max_body_size 100m;" > /etc/nginx/vhost.d/api-raptor.yourdomain.com'
docker restart nginx-proxy
```

---

## 🤝 Contributing

Contributions are welcome. Please follow these guidelines:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/improvement`)
3. Implement changes with appropriate test coverage
4. Ensure all tests pass
5. Submit a pull request with detailed description

---

## 📄 License

Released under the MIT License. See [LICENSE](LICENSE) for full terms.

---

## 🙏 Acknowledgments

- Developed by [parallela](https://github.com/parallela)
- Architecture inspired by [Pterodactyl Panel](https://pterodactyl.io)

---

<p align="center">
  <strong>⭐ Star this repository if you find it valuable!</strong>
</p>
