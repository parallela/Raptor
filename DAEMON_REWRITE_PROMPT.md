# Claude Sonnet Prompt for Raptor Daemon Container Management Rewrite

---

You are a senior Rust engineer specializing in Tokio, Axum, Docker (Bollard), and high-concurrency async systems.

## Project Context

I have an Axum-based daemon called **Raptor** that manages Docker containers for a game server hosting panel (similar to Pterodactyl). The daemon handles:

- Container lifecycle (create, start, stop, restart, recreate)
- Port allocation/binding management
- Real-time log streaming via WebSocket
- Container stats streaming via WebSocket
- File management for container volumes
- FTP/SFTP credential management
- State persistence to JSON files

## Current Architecture

### Tech Stack
- **Rust** with **Tokio** async runtime
- **Axum** for HTTP/WebSocket server
- **Bollard** for Docker API interaction
- **DashMap** for concurrent container state storage
- **serde_json** for state persistence

### Key Data Structures

```rust
pub struct AppState {
    pub docker: DockerClient,
    pub containers: DashMap<String, ManagedContainer>,
    pub api_key: String,
    pub ftp_state: Arc<FtpServerState>,
}

pub struct ManagedContainer {
    pub name: String,
    pub docker_id: String,
    pub image: String,
    pub startup_script: Option<String>,
    pub stop_command: Option<String>,
    pub allocation: Option<AllocationInfo>,  // Legacy single allocation
    pub allocations: Vec<ContainerAllocation>, // Multiple port bindings
    pub resources: ContainerResources,
}

pub struct ContainerAllocation {
    pub ip: String,
    pub port: i32,
    pub internal_port: i32,
    pub protocol: String,
    pub is_primary: bool,
}

pub struct ContainerResources {
    pub memory_limit: i64,
    pub cpu_limit: f64,
    pub disk_limit: i64,
    pub swap_limit: i64,
    pub io_weight: i32,
}
```

### Current Container Recreation Flow

When allocations change or on container start, the daemon recreates containers to apply port bindings:

1. Stop container (graceful with stop_command, then force after timeout)
2. Remove Docker container
3. Create new Docker container with updated port bindings
4. Update `docker_id` in state
5. Save state to `/var/lib/raptor-daemon/containers.json`
6. Start the new container

## The Problem

After container recreation (stop → remove → create → start), the **entire daemon becomes unresponsive**:
- All HTTP requests hang indefinitely
- All WebSocket connections hang
- The process stays alive but does nothing
- No errors are logged

This suggests the Tokio runtime is blocked or deadlocked.

## Suspected Issues in Current Code

1. **Synchronous file I/O in async context:**
```rust
// PROBLEM: Using std::fs inside async function
let _ = std::fs::create_dir_all(parent);
if let Ok(json) = serde_json::to_string_pretty(&containers_to_save) {
    let _ = std::fs::write(&state_file, json);
}
```

2. **DashMap iteration while holding references:**
```rust
// PROBLEM: Iterating DashMap and collecting while potentially holding locks
let containers_to_save: Vec<ManagedContainer> = state
    .containers
    .iter()
    .map(|r| r.value().clone())
    .collect();
```

3. **No serialization of container operations:**
```rust
// PROBLEM: Multiple concurrent start/restart/recreate calls can race
pub async fn start_container(...) {
    // No lock to prevent concurrent recreation of same container
    recreate_container_with_allocations(&state, &container).await
}
```

4. **Potential deadlock in recreate flow:**
```rust
// PROBLEM: Complex async operations with state mutations
async fn recreate_container_with_allocations(...) {
    // Stop container
    // Remove container  
    // Create container
    // Update DashMap  <-- potential lock issue
    // Save state synchronously <-- blocks runtime
    // Start container
}
```

5. **WebSocket tasks may not be properly cleaned up:**
```rust
// Log streaming spawns tasks that may leak
state.docker.stream_logs(&docker_id, tx);
```

## Your Task

**Completely rewrite the container management layer** to be correct, safe, and non-blocking.

---

## HARD REQUIREMENTS

### 1. ZERO Blocking Calls Inside Async Contexts

- ❌ No `std::fs::*`
- ❌ No `std::thread::sleep`
- ❌ No blocking Docker calls on executor threads
- ✅ Use `tokio::fs`, `tokio::time::sleep`
- ✅ Use `spawn_blocking` where blocking is unavoidable

### 2. Never Hold Locks Across `.await`

- ❌ Do NOT hold `DashMap` guards across await points
- ❌ Do NOT hold mutex guards across await points
- ❌ Do NOT iterate shared maps while awaiting
- ✅ Clone required state first, then drop locks
- ✅ Use scoped blocks to ensure guards are dropped

Example pattern:
```rust
// CORRECT: Clone and drop guard before await
let container = {
    state.containers.get(&id)
        .map(|r| r.value().clone())
        .ok_or((StatusCode::NOT_FOUND, "Not found".into()))?
};
// Guard is dropped here
do_async_operation(&container).await;
```

### 3. Container Recreation Must Be Serialized

- Only ONE recreate/start/restart operation per container at a time
- Use per-container async locks (e.g., `tokio::sync::Mutex` keyed by container name)
- Or use a task queue pattern with `tokio::sync::mpsc`

Example:
```rust
pub struct ContainerLocks {
    locks: DashMap<String, Arc<tokio::sync::Mutex<()>>>,
}

impl ContainerLocks {
    pub fn get_lock(&self, container_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.locks
            .entry(container_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }
}
```

### 4. Rewrite Container Lifecycle Operations

- `start`, `stop`, `restart`, `recreate` must be:
  - Clean and idempotent
  - Race-safe
  - Handle stale Docker IDs correctly
  - Never assume container existence

### 5. State Persistence Must Be Async and Safe

- All file I/O via `tokio::fs`
- No synchronous filesystem writes on Tokio runtime
- Snapshot state before saving (clone everything first)
- Consider debouncing saves to avoid excessive I/O

### 6. WebSocket Streaming Requirements

Log and stats streams MUST:
- Stop cleanly when the socket closes
- Not leak spawned tasks
- Not block the runtime
- Use proper cancellation tokens or `select!` with abort handles

### 7. Error Handling

- Docker 404s (container not found) must be treated as expected state transitions, not errors
- Recreate logic must survive partial failure (e.g., remove succeeded but create failed)
- Always leave state consistent

---

## Deliverables

Provide a complete rewrite of:

1. **Container lifecycle handlers** (`start_container`, `stop_container`, `restart_container`, `recreate_container`)

2. **State persistence functions** (`save_container_state`, `load_container_state`)

3. **Per-container locking mechanism**

4. **WebSocket streaming handlers** with proper cleanup

5. **Helper function for safe DashMap operations**

---

## Code Style Guidelines

- Use `tracing` for logging (`tracing::info!`, `tracing::error!`, etc.)
- Use `thiserror` or explicit error types
- Prefer `Result<T, (StatusCode, String)>` for Axum handlers
- Use `serde` with `#[serde(rename_all = "camelCase")]` for JSON
- Keep functions focused and testable

---

## Example of Expected Patterns

### Safe State Access
```rust
async fn get_container_clone(state: &AppState, id: &str) -> Option<ManagedContainer> {
    state.containers.get(id).map(|r| r.value().clone())
}
```

### Safe State Update
```rust
async fn update_container_docker_id(state: &AppState, name: &str, new_docker_id: String) {
    if let Some(mut entry) = state.containers.get_mut(name) {
        entry.docker_id = new_docker_id;
    }
    // Guard dropped here, now safe to await
    save_container_state_async(state).await;
}
```

### Async State Save
```rust
async fn save_container_state_async(state: &AppState) -> Result<(), std::io::Error> {
    // Snapshot state without holding locks across await
    let containers: Vec<ManagedContainer> = {
        state.containers.iter().map(|r| r.value().clone()).collect()
    };
    
    let json = serde_json::to_string_pretty(&containers)?;
    let path = get_state_file_path();
    
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    
    tokio::fs::write(&path, json).await
}
```

### Per-Container Locking
```rust
pub async fn start_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Acquire per-container lock
    let lock = state.container_locks.get_lock(&id);
    let _guard = lock.lock().await;
    
    // Now safe to do container operations
    // ...
}
```

---

## Current Problematic Code

Below is the current `handlers.rs` that needs to be rewritten. Pay special attention to the `recreate_container_with_allocations` function and all container lifecycle handlers:

```rust
/// Save container state to disk (non-blocking - spawns a task)
pub fn save_container_state_background(containers: Vec<ManagedContainer>) {
    let state_file = get_state_file_path();

    tokio::spawn(async move {
        if let Some(parent) = state_file.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                tracing::error!("Failed to create state directory: {}", e);
                return;
            }
        }

        match serde_json::to_string_pretty(&containers) {
            Ok(json) => {
                if let Err(e) = tokio::fs::write(&state_file, json).await {
                    tracing::error!("Failed to save container state: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize container state: {}", e);
            }
        }
    });
}

pub async fn start_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    // Check if we have managed container state - always recreate to ensure port bindings are correct
    if let Some(container) = state.containers.get(&id) {
        let container = container.clone();

        tracing::info!("Container {} has {} allocations, recreating to ensure port bindings are correct",
            id, container.allocations.len());

        // Always recreate container on start to ensure allocations/port bindings are applied
        return recreate_container_with_allocations(&state, &container).await;
    }

    container_action(&state, &id, "start").await
}

async fn recreate_container_with_allocations(
    state: &AppState,
    container: &ManagedContainer,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // ... builds port_bindings ...
    
    // Try to stop and remove old container
    // ... docker operations ...
    
    // Create new container
    let docker_id = state.docker.create_container_with_resources(...).await?;

    // PROBLEM AREA: This section causes deadlock
    {
        if let Some(mut managed) = state.containers.get_mut(&container.name) {
            managed.docker_id = docker_id.clone();
        }
    }

    // Using synchronous file I/O - BLOCKS THE RUNTIME
    {
        let containers_to_save: Vec<ManagedContainer> = state
            .containers
            .iter()
            .map(|r| r.value().clone())
            .collect();
        
        let state_file = get_state_file_path();
        if let Some(parent) = state_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&containers_to_save) {
            let _ = std::fs::write(&state_file, json);
        }
    }

    // Start container
    state.docker.start_container(&docker_id).await?;
    
    Ok(Json(serde_json::json!({ "success": true })))
}
```

---

## Additional Context

### Files Structure
```
daemon/
├── src/
│   ├── main.rs          # Axum router setup
│   ├── handlers.rs      # All HTTP/WS handlers (NEEDS REWRITE)
│   ├── docker.rs        # Docker client wrapper
│   ├── models.rs        # Data structures
│   ├── ftp.rs           # FTP server
│   └── config.rs        # Configuration
├── Cargo.toml
└── Dockerfile
```

### State File Location
- Default: `/var/lib/raptor-daemon/containers.json`
- Configurable via `DAEMON_DATA_DIR` env var

### FTP Credentials Location  
- Default: `/var/lib/raptor-daemon/ftp_credentials.json`
- Keyed by container UUID for O(1) lookup

---

## Expected Output

Please provide:

1. Complete rewritten `handlers.rs` with all fixes applied
2. Any necessary changes to `models.rs` (e.g., adding `ContainerLocks`)
3. Any necessary changes to `main.rs` for initialization
4. Explanation of the key changes and why they fix the deadlock

Focus on correctness and safety over optimization. The daemon must never hang.
