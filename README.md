<p align="center">
  <img src="panel/static/logo.webp" alt="Raptor Logo" width="400" />
</p>

<h1 align="center">Raptor</h1>

<p align="center">
  <strong>A Modern, Self-Hosted Game Server Management Panel</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.75+-orange?style=flat-square&logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/SvelteKit-2.0-FF3E00?style=flat-square&logo=svelte" alt="SvelteKit" />
  <img src="https://img.shields.io/badge/Docker-Ready-2496ED?style=flat-square&logo=docker" alt="Docker" />
  <img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="License" />
</p>

<p align="center">
  A lightweight, fast, and modern alternative to Pterodactyl Panel.<br/>
  Built with Rust for performance and SvelteKit for a beautiful, responsive UI.
</p>

---

## ✨ Why Raptor?

| Feature | Raptor | Pterodactyl |
|---------|--------|-------------|
| **Language** | Rust (blazing fast) | PHP |
| **Memory Usage** | ~30MB | ~150MB+ |
| **Frontend** | SvelteKit (modern, reactive) | Vue.js 2 |
| **Installation** | Single binary + Docker images | Complex PHP setup |
| **Database** | PostgreSQL only | MySQL/MariaDB |
| **Real-time** | Native WebSocket | Pusher/WebSocket |
| **Mobile UI** | Fully responsive | Limited |

### 🚀 Key Features

- **🔥 Blazing Fast** - Written in Rust with async/await for maximum performance
- **📱 Mobile First** - Responsive design that works beautifully on all devices
- **🔒 Secure** - JWT authentication, TLS everywhere, secure by default
- **🐳 Docker Native** - Containers managed through Docker API
- **📊 Real-time Stats** - Live CPU, memory, and network monitoring via WebSocket
- **📁 File Manager** - Built-in file browser with upload/download support
- **🔌 FTP Access** - Per-container FTP credentials with jailed access
- **🗃️ Database Support** - PostgreSQL, MySQL, and Redis database management
- **🎮 Game Ready** - Pre-configured templates (Flakes) for popular games
- **🌐 Multi-Node** - Support for multiple daemon hosts

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         NGINX PROXY                              │
│              (SSL Termination, Load Balancing)                   │
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
                                            │ PostgreSQL
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

| Component | Description | Technology |
|-----------|-------------|------------|
| **Panel** | Web-based management interface | SvelteKit + TailwindCSS |
| **API** | Backend REST API + WebSocket | Rust + Axum |
| **Daemon** | Host agent for container management | Rust + Bollard |
| **Database** | Persistent storage | PostgreSQL 16 |

---

## 📦 Installation

### Prerequisites

- **Docker** & **Docker Compose** installed
- A domain name with DNS configured
- **nginx-proxy** with Let's Encrypt (recommended for production)

### Option 1: Quick Start (Development)

```bash
git clone https://github.com/parallela/raptor.git
cd raptor
docker compose up -d
```

Access the panel at `http://localhost:5173` (admin/admin123)

### Option 2: Production Deployment

#### Step 1: Set up nginx-proxy (if not already running)

```bash
# Create the proxy network
docker network create nginx-proxy

# Start nginx-proxy with Let's Encrypt
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

**Important:** Configure nginx-proxy for large file uploads (required for chunk uploads):

```bash
# Create vhost config directory if it doesn't exist
docker exec nginx-proxy mkdir -p /etc/nginx/vhost.d

# Enable large uploads for the API domain
docker exec nginx-proxy bash -c 'echo "client_max_body_size 100m;" > /etc/nginx/vhost.d/api-raptor.yourdomain.com'

# Restart nginx-proxy
docker restart nginx-proxy
```

#### Step 2: Create the deployment directory

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

#### Step 4: Create environment files

**.env.production** (for API):

```bash
# Database
DATABASE_URL=postgres://raptor:your_secure_password@postgres:5432/raptor

# Security
JWT_SECRET=your_random_64_char_hex_string
JWT_EXPIRY_DAYS=7
BCRYPT_COST=12

# Server
API_ADDR=0.0.0.0:3000
APP_URL=https://raptor.yourdomain.com
RUST_LOG=info

# Initial Admin (only used on first run)
ADMIN_USERNAME=admin
ADMIN_EMAIL=admin@yourdomain.com
ADMIN_PASSWORD=your_secure_admin_password

# SMTP (optional, for email notifications)
# SMTP_HOST=smtp.example.com
# SMTP_PORT=587
# SMTP_USERNAME=your_smtp_user
# SMTP_PASSWORD=your_smtp_password
# SMTP_FROM_EMAIL=raptor@yourdomain.com
# SMTP_FROM_NAME=Raptor

# nginx-proxy integration
VIRTUAL_HOST=api-raptor.yourdomain.com
LETSENCRYPT_HOST=api-raptor.yourdomain.com
LETSENCRYPT_EMAIL=admin@yourdomain.com
```

Generate a secure JWT secret:
```bash
openssl rand -hex 32
```

**.env.frontend** (for Panel):

```bash
# API URL (must be publicly accessible)
API_URL=https://api-raptor.yourdomain.com

# SvelteKit settings
ORIGIN=https://raptor.yourdomain.com
BODY_SIZE_LIMIT=Infinity

# nginx-proxy integration
VIRTUAL_HOST=raptor.yourdomain.com
LETSENCRYPT_HOST=raptor.yourdomain.com
LETSENCRYPT_EMAIL=admin@yourdomain.com
```

#### Step 5: Start the services

```bash
docker compose up -d
```

#### Step 6: Verify deployment

```bash
# Check all services are running
docker compose ps

# View logs
docker compose logs -f

# Test API health
curl https://api-raptor.yourdomain.com/health
```

---

## 🖥️ Daemon Installation

The daemon must run **directly on each host machine** (not in Docker) to manage containers and access system resources.

### Quick Install

```bash
# Download the daemon binary
wget https://github.com/parallela/raptor/releases/latest/download/raptor-daemon-linux-amd64 -O /tmp/raptor-daemon
chmod +x /tmp/raptor-daemon

# Download and run the installer
wget https://github.com/parallela/raptor/releases/latest/download/install.sh -O /tmp/install.sh
chmod +x /tmp/install.sh
sudo /tmp/install.sh
```

### What the Installer Does

1. **Creates system user** - `raptor` user and group
2. **Sets up directories**:
   - `/opt/raptor` - Application home
   - `/var/lib/raptor/volumes` - Container volumes
   - `/var/lib/raptor/sys` - State files
   - `/var/log/raptor` - Log files
3. **Generates API key** - Random UUID for authentication
4. **Installs systemd service** - `raptor-daemon.service`
5. **Configures log rotation** - Daily rotation, 14 days retention

### Configure TLS (Required for HTTPS)

```bash
# Copy your SSL certificates
sudo cp /path/to/fullchain.pem /opt/raptor/certs/
sudo cp /path/to/privkey.pem /opt/raptor/certs/
sudo chown raptor:raptor /opt/raptor/certs/*
sudo chmod 600 /opt/raptor/certs/privkey.pem

# Edit configuration
sudo nano /opt/raptor/.env
```

Uncomment and set TLS paths:
```bash
TLS_CERT_PATH=/opt/raptor/certs/fullchain.pem
TLS_KEY_PATH=/opt/raptor/certs/privkey.pem
```

### Start the Daemon

```bash
# Enable and start
sudo systemctl enable raptor-daemon
sudo systemctl start raptor-daemon

# Check status
sudo systemctl status raptor-daemon

# View logs
sudo tail -f /var/log/raptor/daemon.log
```

### Add Daemon to Panel

1. Go to **Admin** → **Daemons** → **Add Daemon**
2. Enter:
   - **Name**: Descriptive name (e.g., "US-East-1")
   - **Host**: Server IP or hostname
   - **Port**: 6969 (default)
   - **Secure**: Yes (if TLS configured)
   - **API Key**: Copy from `/opt/raptor/.env`
3. Click **Save**

### Daemon Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DAEMON_API_KEY` | Authentication key | Generated |
| `DAEMON_ADDR` | Listen address | `0.0.0.0:6969` |
| `FTP_HOST` | FTP server bind address | `0.0.0.0` |
| `FTP_PORT` | FTP server port | `2121` |
| `AVAILABLE_IPS` | IPs for allocation | `0.0.0.0` |
| `TLS_CERT_PATH` | SSL certificate path | - |
| `TLS_KEY_PATH` | SSL private key path | - |
| `RUST_LOG` | Log level | `info` |

### Firewall Configuration

Open the required ports:

```bash
# UFW
sudo ufw allow 6969/tcp  # Daemon API
sudo ufw allow 2121/tcp  # FTP
sudo ufw allow 21000:21100/tcp  # FTP Passive ports (optional)

# iptables
sudo iptables -A INPUT -p tcp --dport 6969 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 2121 -j ACCEPT
```

---

## 🎮 Flakes (Server Templates)

Flakes are pre-configured templates for deploying game servers.

### Included Flakes

| Game | Image | Features |
|------|-------|----------|
| **Minecraft (Paper)** | `artifacts.lstan.eu/java:21` | Auto-download, plugin support |
| **Minecraft (Vanilla)** | `artifacts.lstan.eu/java:21` | EULA acceptance |
| **Hytale** | `ghcr.io/pterodactyl/games:hytale` | Auto-update, TTY support |
| **Node.js** | `artifacts.lstan.eu/nodejs:20` | Generic Node.js apps |

### Creating Custom Flakes

Go to **Admin** → **Flakes** → **Import Flake** and upload a JSON file:

```json
{
  "name": "My Custom Server",
  "slug": "my-server",
  "description": "Description of your server",
  "dockerImage": "your-image:tag",
  "startupCommand": "java -Xms{{SERVER_MEMORY}}M -Xmx{{SERVER_MEMORY}}M -jar server.jar",
  "stopCommand": "stop",
  "installScript": "#!/bin/bash\nwget -O server.jar https://...",
  "tty": false,
  "variables": [
    {
      "name": "Server Memory",
      "description": "Memory allocation in MB",
      "envVariable": "SERVER_MEMORY",
      "defaultValue": "1024",
      "userViewable": true,
      "userEditable": true,
      "rules": "required|numeric|min:512"
    }
  ]
}
```

### Variable Placeholders

Use `{{VARIABLE_NAME}}` in startup commands:

- `{{SERVER_MEMORY}}` - Server memory allocation
- `{{SERVER_PORT}}` - Primary port allocation
- `{{SERVER_IP}}` - Server IP address
- Any custom variable from the flake

---

## 🗄️ Database Management

Raptor supports PostgreSQL, MySQL, and Redis database management.

### Admin: Setting up Database Servers

1. Go to **Admin** → **Database Servers**
2. Click **Add Database Server**
3. Configure:
   - **Type**: PostgreSQL, MySQL, or Redis
   - **Daemon**: Which host to run on
   - **Name**: Container name
   - **Root Password**: Admin password
4. Click **Create** - The database container will be created

### Users: Creating Databases

1. Go to **Databases** in the sidebar
2. Click **Create Database**
3. Select database type from available servers
4. Credentials are automatically generated and displayed

**Note**: Redis databases use key prefixing for isolation. All keys are automatically prefixed with your database name.

---

## 🔐 Security

### Role-Based Access Control

| Role | Permissions |
|------|-------------|
| **Admin** | Full access to everything |
| **Manager** | Manage users, containers, allocations |
| **User** | Manage own containers only |

### Container Access Levels

- **Owner** - Full control, can delete
- **Admin** - Can manage settings
- **User** - Can start/stop, view console

### Security Features

- ✅ JWT token authentication
- ✅ HTTPS/TLS encryption
- ✅ Per-container FTP jailing
- ✅ Secure WebSocket connections
- ✅ CORS protection
- ✅ Input validation

---

## 📱 Mobile Support

Raptor features a fully responsive mobile interface:

- Collapsible sidebar with hamburger menu
- Touch-optimized controls
- Compact server status display
- Mobile-friendly console with send button
- Swipe-friendly file browser

---

## 🛠️ Building from Source

### API

```bash
cd api
cargo build --release
# Binary: target/release/raptor-api
```

### Daemon

```bash
cd daemon
# For Linux target from macOS:
cargo build --release --target x86_64-unknown-linux-gnu
# Binary: target/x86_64-unknown-linux-gnu/release/raptor-daemon
```

### Panel

```bash
cd panel
npm install
npm run build
# Output: build/
```

### Docker Images

```bash
# Build API image
docker build -t raptor-api:latest -f api/Dockerfile .

# Build Panel image  
docker build -t raptor-panel:latest -f panel/Dockerfile .

# Build for linux/amd64 (from macOS)
docker buildx build --platform linux/amd64 -t raptor-api:latest -f api/Dockerfile .
```

---

## 📚 Troubleshooting

### Daemon won't start

```bash
# Check logs
sudo journalctl -u raptor-daemon -f
sudo tail -f /var/log/raptor/daemon.log

# Verify Docker access
sudo -u raptor docker ps

# Check permissions
ls -la /var/lib/raptor/
ls -la /opt/raptor/certs/
```

### Container creation fails

1. Ensure daemon is connected (check Daemons page)
2. Verify IP allocations exist
3. Check Docker images are available

### WebSocket connection issues

1. Ensure nginx-proxy is configured for WebSocket
2. Check TLS certificates are valid
3. Verify firewall allows port 6969

### File upload fails

Configure nginx-proxy for large uploads:
```bash
docker exec nginx-proxy bash -c 'echo "client_max_body_size 100m;" > /etc/nginx/vhost.d/api-raptor.yourdomain.com'
docker restart nginx-proxy
```

---

## 🤝 Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests
5. Submit a pull request

---

## 📄 License

MIT License - see [LICENSE](LICENSE) for details.

---

## 🙏 Credits

- Built with ❤️ by [parallela](https://github.com/parallela)
- Inspired by [Pterodactyl Panel](https://pterodactyl.io)

---

<p align="center">
  <strong>⭐ Star this repo if you find it useful!</strong>
</p>
