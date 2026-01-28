# Raptor - Pterodactyl Alternative

A modern container management panel built with Rust and SvelteKit.

## Architecture

```
┌───────────────┐
│   Svelte UI   │  ← HTTP →  ┌─────────────┐
└───────────────┘             │ Rust API    │ ←→ Postgres
                              └─────┬───────┘
                                    │ HTTP + WebSocket
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
             ┌───────────┐   ┌───────────┐   ┌───────────┐
             │  Daemon   │   │  Daemon   │   │  Daemon   │
             │  Host 1   │   │  Host 2   │   │  Host N   │
             └───────────┘   └───────────┘   └───────────┘
```

## Components

- **API Backend** (`/api`): Rust/Axum server handling authentication, user management, and proxying requests to daemons
- **Daemon** (`/daemon`): Standalone Rust binary running on host machines, managing Docker containers and SFTP
- **Panel** (`/panel`): SvelteKit + Tailwind UI for managing containers

## Quick Start

### 1. Start Core Services (Docker Compose)

```bash
docker compose up -d
```

This will start:
- PostgreSQL on port 5432
- API on port 3000
- Panel on port 5173

### 2. Run Daemon on Host Machine(s)

The daemon must run as a **standalone binary** on each host machine (not in Docker) to properly manage containers and report system resources.

```bash
cd daemon
cp .env.example .env
# Edit .env and set DAEMON_API_KEY (get from panel after creating daemon)
cargo build --release
./target/release/raptor-daemon
```

### 3. Add Daemon in Panel

1. Login to panel at http://localhost:5173 (admin/admin123)
2. Go to Daemons → Add Daemon
3. Enter the host IP/hostname and port (default 8080)
4. Copy the generated API key to the daemon's .env file
5. Restart the daemon

## Development Setup

### Prerequisites
- Rust 1.75+
- Node.js 20+
- PostgreSQL 16+
- Docker (on daemon hosts)

### API Backend

```bash
cd api
cp .env.example .env
# Edit .env with your settings
cargo run
```

### Daemon (run on host, not in Docker)

```bash
cd daemon
cp .env.example .env
# Edit .env - set DAEMON_API_KEY from panel
cargo run
```

### Panel

```bash
cd panel
npm install
npm run dev
```

## Configuration

### API Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `APP_KEY` | Application secret key (required) | Generate with `openssl rand -base64 32` |
| `DATABASE_URL` | PostgreSQL connection string | Required |
| `JWT_SECRET` | Secret for JWT tokens | Uses `APP_KEY` if not set |
| `JWT_EXPIRY_DAYS` | JWT token expiry in days | `7` |
| `API_ADDR` | Listen address | `0.0.0.0:3000` |
| `APP_URL` | Frontend URL for emails | `http://localhost:5173` |
| `BCRYPT_COST` | Bcrypt hashing cost | `12` |
| `ADMIN_USERNAME` | Initial admin username | `admin` |
| `ADMIN_EMAIL` | Initial admin email | `admin@localhost` |
| `ADMIN_PASSWORD` | Initial admin password | Required for admin creation |
| `SMTP_*` | SMTP configuration for emails | Optional |

### Daemon Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DAEMON_ADDR` | Listen address | `0.0.0.0:8080` |
| `DAEMON_API_KEY` | API key (from panel) | Required |
| `SFTP_BASE_PATH` | Container data path | `/data/raptor/containers` |
| `SFTP_HOST` | SFTP listen address | `0.0.0.0` |
| `SFTP_PORT` | SFTP port | `2222` |
| `AVAILABLE_IPS` | Available IPs for allocation | `0.0.0.0` |
| `RUST_LOG` | Log level | `info` |

### Panel Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `VITE_API_URL` | API backend URL | `http://localhost:3000` |

## Permission System

Raptor uses a flexible role-based permission system.

### Default Roles

| Role | Permissions |
|------|-------------|
| `admin` | `*` (full access) |
| `manager` | Admin panel access, manage users/containers/allocations |
| `user` | View and manage own containers only |

### Container Access

Containers support multiple users with different permission levels:
- `owner` - Full control (assigned on creation)
- `admin` - Can manage and modify
- `user` - Can start/stop/view

## TODO

- [ ] Unit tests
- [ ] Integration tests
- [ ] E2E tests
- [ ] File manager UI
- [ ] Backup system
- [ ] Server templates/eggs

## License

MIT
