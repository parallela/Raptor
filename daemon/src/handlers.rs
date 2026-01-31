use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use bollard::service::PortBinding;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::broadcast;
use serde::{Deserialize, Serialize};

use crate::models::{
    AppState, AssignAllocationRequest, AvailableAllocation, CreateContainerRequest,
    ManagedContainer,
};
use crate::ftp::{create_ftp_access, FtpCredentials};
use crate::database_manager::{
    self, CreateDatabaseServerRequest, CreateUserDatabaseRequest,
    DatabaseServer, DeleteUserDatabaseRequest, ResetPasswordRequest,
};

// ============================================================================
// STATE PERSISTENCE - All async, no blocking I/O
// ============================================================================

/// Get the path to the container state file
fn get_state_file_path() -> PathBuf {
    let data_dir = std::env::var("DAEMON_DATA_DIR")
        .unwrap_or_else(|_| "/var/lib/raptor-daemon".to_string());
    PathBuf::from(data_dir).join("containers.json")
}

/// Snapshot current container state from DashMap.
/// IMPORTANT: This clones all data and releases all locks before returning.
fn snapshot_containers(state: &AppState) -> Vec<ManagedContainer> {
    state.containers.iter().map(|r| r.value().clone()).collect()
}

/// Save container state to disk asynchronously.
/// This function:
/// 1. Snapshots state without holding locks across await points
/// 2. Uses tokio::fs for all I/O operations
/// 3. Is safe to call from any async context
pub async fn save_container_state(state: &AppState) {
    // Snapshot state first - DashMap guards are dropped immediately
    let containers = snapshot_containers(state);
    let state_file = get_state_file_path();

    tracing::debug!("Saving {} containers to {:?}", containers.len(), state_file);

    // Ensure parent directory exists using async I/O
    if let Some(parent) = state_file.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            tracing::error!("Failed to create state directory: {}", e);
            return;
        }
    }

    // Serialize and write using async I/O
    match serde_json::to_string_pretty(&containers) {
        Ok(json) => {
            if let Err(e) = tokio::fs::write(&state_file, json).await {
                tracing::error!("Failed to save container state: {}", e);
            } else {
                tracing::debug!("Container state saved to {:?}", state_file);
            }
        }
        Err(e) => {
            tracing::error!("Failed to serialize container state: {}", e);
        }
    }
}

/// Save container state in background (fire-and-forget).
/// Useful when you don't need to wait for the save to complete.
pub fn save_container_state_background(state: &AppState) {
    // Snapshot state before spawning - guards dropped immediately
    let containers = snapshot_containers(state);
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
                } else {
                    tracing::debug!("Container state saved in background to {:?}", state_file);
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize container state: {}", e);
            }
        }
    });
}

/// Load container state from disk
pub async fn load_container_state() -> Vec<ManagedContainer> {
    let state_file = get_state_file_path();

    match tokio::fs::read_to_string(&state_file).await {
        Ok(json) => {
            match serde_json::from_str(&json) {
                Ok(containers) => {
                    tracing::info!("Loaded container state from {:?}", state_file);
                    containers
                }
                Err(e) => {
                    tracing::error!("Failed to parse container state: {}", e);
                    Vec::new()
                }
            }
        }
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::error!("Failed to read container state: {}", e);
            }
            Vec::new()
        }
    }
}

// ============================================================================
// SAFE STATE ACCESS HELPERS
// These ensure DashMap guards are never held across await points
// ============================================================================

/// Get a clone of a container from state. Guard is dropped immediately.
fn get_container_clone(state: &AppState, id: &str) -> Option<ManagedContainer> {
    state.containers.get(id).map(|r| r.value().clone())
}

/// Get Docker ID for a container, falling back to the provided id.
fn get_docker_id(state: &AppState, id: &str) -> String {
    state.containers
        .get(id)
        .map(|r| r.value().docker_id.clone())
        .unwrap_or_else(|| id.to_string())
}

/// Update a container's docker_id. Guard is dropped after the update.
fn update_container_docker_id(state: &AppState, name: &str, new_docker_id: String) {
    if let Some(mut entry) = state.containers.get_mut(name) {
        entry.docker_id = new_docker_id;
    }
    // Guard is dropped here
}

/// Mark a container as installed. Guard is dropped after the update.
fn mark_container_installed(state: &AppState, name: &str) {
    if let Some(mut entry) = state.containers.get_mut(name) {
        entry.installed = true;
        // Clear install script to save memory
        entry.install_script = None;
    }
    // Guard is dropped here
}

/// Replace {{VAR}} placeholders in a startup script with actual values from environment and resources
/// Also handles backward compatibility: if SERVER_MEMORY was already replaced with a hardcoded value,
/// it will update -Xmx and -Xms parameters to use the current server_memory
fn replace_startup_placeholders(
    script: &str,
    environment: &std::collections::HashMap<String, String>,
    resources: &crate::models::ContainerResources,
) -> String {
    let mut result = script.to_string();

    // Replace from environment variables
    for (key, value) in environment {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }

    // Use server_memory for {{SERVER_MEMORY}} - this is the JVM heap memory
    // If server_memory is 0, fall back to memory_limit for backward compatibility
    let server_memory = if resources.server_memory > 0 {
        resources.server_memory
    } else {
        resources.memory_limit
    };

    // Replace {{SERVER_MEMORY}} with server_memory from resources
    result = result.replace("{{SERVER_MEMORY}}", &server_memory.to_string());

    // Backward compatibility: If -Xmx is hardcoded with a different value, update it
    // This handles old containers where {{SERVER_MEMORY}} was already replaced
    let memory_str = server_memory.to_string();

    // Use regex-like replacement for -Xmx parameters
    // Match patterns like -Xmx1024M, -Xmx2048M, etc.
    let xmx_pattern = regex::Regex::new(r"-Xmx\d+M").ok();

    if let Some(re) = xmx_pattern {
        // Only replace if the current value doesn't match
        if !result.contains(&format!("-Xmx{}M", memory_str)) {
            result = re.replace(&result, format!("-Xmx{}M", memory_str).as_str()).to_string();
        }
    }

    // For -Xms, we typically want it to be a smaller value (e.g., 128M), so don't auto-replace
    // unless it's set to {{SERVER_MEMORY}} pattern

    result
}

// ============================================================================
// AUTHENTICATION
// ============================================================================

fn verify_api_key(headers: &HeaderMap, state: &AppState) -> bool {
    headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(|k| k == state.api_key)
        .unwrap_or(false)
}

// ============================================================================
// CONTAINER CRUD OPERATIONS
// ============================================================================

pub async fn list_containers(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<ManagedContainer>>, StatusCode> {
    if !verify_api_key(&headers, &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Snapshot containers - guards released immediately
    let containers = snapshot_containers(&state);

    // Also list Docker containers if needed for reconciliation
    if let Ok(docker_containers) = state.docker.list_containers().await {
        tracing::debug!("Docker has {} containers", docker_containers.len());
    }

    Ok(Json(containers))
}

pub async fn create_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CreateContainerRequest>,
) -> Result<Json<ManagedContainer>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    tracing::info!("Creating container {} with {} allocations", req.name, req.allocations.len());

    // Build port bindings - multiple allocations can map to the same internal port
    let mut port_bindings: HashMap<String, Vec<PortBinding>> = HashMap::new();

    // Handle multiple allocations (new model)
    for alloc in &req.allocations {
        let key = format!("{}/{}", alloc.internal_port, alloc.protocol);
        tracing::info!("Adding allocation: {} -> {}:{}", key, alloc.ip, alloc.port);
        port_bindings
            .entry(key)
            .or_insert_with(Vec::new)
            .push(PortBinding {
                host_ip: Some(alloc.ip.clone()),
                host_port: Some(alloc.port.to_string()),
            });
    }

    // Legacy single allocation support
    if let Some(ref alloc) = req.allocation {
        if req.allocations.is_empty() {
            let key = format!("{}/tcp", alloc.port);
            tracing::info!("Adding legacy allocation: {} -> {}:{}", key, alloc.ip, alloc.port);
            port_bindings
                .entry(key)
                .or_insert_with(Vec::new)
                .push(PortBinding {
                    host_ip: Some(alloc.ip.clone()),
                    host_port: Some(alloc.port.to_string()),
                });
        }
    }

    for port in &req.ports {
        let key = format!("{}/{}", port.container_port, port.protocol);
        port_bindings
            .entry(key)
            .or_insert_with(Vec::new)
            .push(PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some(port.host_port.to_string()),
            });
    }

    tracing::info!("Total port bindings: {:?}", port_bindings);

    // server_memory is for JVM heap, memory_limit is for Docker container
    // If server_memory is not provided, use memory_limit for backward compatibility
    let server_memory = req.server_memory.unwrap_or(req.memory_limit);

    let resources = crate::models::ContainerResources {
        memory_limit: req.memory_limit,
        server_memory,
        cpu_limit: req.cpu_limit,
        disk_limit: req.disk_limit,
        swap_limit: req.swap_limit,
        io_weight: req.io_weight,
    };

    let docker_id = state
        .docker
        .create_container_with_resources(
            &req.name,
            &req.image,
            req.startup_script.as_deref(),
            if port_bindings.is_empty() { None } else { Some(port_bindings) },
            &resources,
            &req.restart_policy,
            req.tty,
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Check if there's an install script - if so, mark as not installed
    let has_install_script = req.install_script.is_some();

    // Merge environment with SERVER_MEMORY set from server_memory
    let mut environment = req.environment.clone();
    environment.insert("SERVER_MEMORY".to_string(), server_memory.to_string());

    let managed = ManagedContainer {
        name: req.name.clone(),
        docker_id: docker_id.clone(),
        image: req.image.clone(),
        startup_script: req.startup_script.clone(),
        stop_command: req.stop_command.clone(),
        allocation: req.allocation.clone(),
        allocations: req.allocations.clone(),
        resources: resources.clone(),
        install_script: req.install_script.clone(),
        installed: !has_install_script, // If no install script, consider it installed
        environment,
        restart_policy: req.restart_policy.clone(),
        tty: req.tty,
    };

    // Insert into state - guard dropped immediately
    state.containers.insert(req.name.clone(), managed.clone());

    // Save state asynchronously (no locks held)
    save_container_state(&state).await;

    // Note: Install script will run on first start, not here
    // This allows the user to see installation progress in the console
    if has_install_script {
        tracing::info!("Container {} has install script - will run on first start", req.name);
    }

    Ok(Json(managed))
}

pub async fn get_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ManagedContainer>, StatusCode> {
    if !verify_api_key(&headers, &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Get from managed state - guard dropped immediately
    let container = get_container_clone(&state, &id)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Also get real Docker status
    if let Ok(docker_info) = state.docker.get_container(&container.docker_id).await {
        tracing::debug!("Docker container {} status: {}", id, docker_info.status);
    }

    Ok(Json(container))
}

pub async fn delete_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<()>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    // Get container info - guard dropped immediately
    let container = get_container_clone(&state, &id)
        .ok_or((StatusCode::NOT_FOUND, "Container not found".into()))?;

    // Remove from Docker (may already be removed)
    if let Err(e) = state.docker.remove_container(&container.docker_id).await {
        tracing::warn!("Failed to remove Docker container (may not exist): {}", e);
    }

    // Remove from managed state
    state.containers.remove(&id);


    // Save state asynchronously
    save_container_state(&state).await;

    Ok(Json(()))
}

pub async fn update_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<crate::models::UpdateContainerRequest>,
) -> Result<Json<ManagedContainer>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }


    // Get the container from managed state - guard dropped immediately
    let mut container = get_container_clone(&state, &id)
        .ok_or((StatusCode::NOT_FOUND, "Container not found".into()))?;

    // Update resources if provided
    if let Some(memory) = req.memory_limit {
        container.resources.memory_limit = memory;
    }
    if let Some(server_memory) = req.server_memory {
        container.resources.server_memory = server_memory;
        // Update SERVER_MEMORY in environment so startup script uses new value
        container.environment.insert("SERVER_MEMORY".to_string(), server_memory.to_string());
    }
    if let Some(cpu) = req.cpu_limit {
        container.resources.cpu_limit = cpu;
    }
    if let Some(disk) = req.disk_limit {
        container.resources.disk_limit = disk;
    }
    if let Some(swap) = req.swap_limit {
        container.resources.swap_limit = swap;
    }
    if let Some(io) = req.io_weight {
        container.resources.io_weight = io;
    }

    // Update allocation if provided (legacy)
    if let Some(alloc) = req.allocation {
        container.allocation = Some(alloc);
    }

    // Update allocations array if provided
    if let Some(allocations) = req.allocations {
        container.allocations = allocations;
    }

    // Try to update Docker container resources (only works on running containers)
    if let Err(e) = state.docker.update_container_resources(
        &container.docker_id,
        &container.resources,
    ).await {
        tracing::warn!("Failed to update Docker container resources (container might be stopped): {}", e);
        // Don't fail - we still update the managed state
    }

    // Save updated container to state - guard dropped immediately
    state.containers.insert(id.clone(), container.clone());

    // Save container state asynchronously
    save_container_state(&state).await;

    Ok(Json(container))
}

// ============================================================================
// CONTAINER LIFECYCLE OPERATIONS (with per-container locking)
// ============================================================================

/// Internal helper for simple container actions (no recreation)
async fn container_action(
    state: &AppState,
    id: &str,
    action: &str,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Get docker_id - guard dropped immediately
    let docker_id = get_docker_id(state, id);

    let result = match action {
        "start" => state.docker.start_container(&docker_id).await,
        "stop" => state.docker.stop_container(&docker_id).await,
        "restart" => state.docker.restart_container(&docker_id).await,
        "kill" => state.docker.kill_container(&docker_id).await,
        _ => return Err((StatusCode::BAD_REQUEST, "Invalid action".into())),
    };

    result.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn start_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    // Get container from state
    let container = get_container_clone(&state, &id);

    if let Some(container) = container {
        tracing::info!(
            "Starting container {} with {} allocations - will recreate to ensure port bindings",
            id,
            container.allocations.len()
        );

        // Build port bindings from allocations
        let mut port_bindings: HashMap<String, Vec<PortBinding>> = HashMap::new();

        for alloc in &container.allocations {
            let key = format!("{}/{}", alloc.internal_port, alloc.protocol);
            tracing::info!("Binding allocation: {} -> {}:{}", key, alloc.ip, alloc.port);
            port_bindings
                .entry(key)
                .or_insert_with(Vec::new)
                .push(PortBinding {
                    host_ip: Some(alloc.ip.clone()),
                    host_port: Some(alloc.port.to_string()),
                });
        }

        // Legacy allocation support
        if let Some(ref alloc) = container.allocation {
            if container.allocations.is_empty() {
                let key = format!("{}/tcp", alloc.port);
                port_bindings
                    .entry(key)
                    .or_insert_with(Vec::new)
                    .push(PortBinding {
                        host_ip: Some(alloc.ip.clone()),
                        host_port: Some(alloc.port.to_string()),
                    });
            }
        }

        tracing::info!("Port bindings: {:?}", port_bindings);

        // Stop the container gracefully using docker stop (sends SIGTERM)
        // This allows the server to shut down properly and show shutdown logs
        // Ignore errors - container might not be running
        let _ = state.docker.graceful_stop(&container.docker_id, 30).await;

        // Clean up ALL old containers with this name (force remove now that it's stopped)
        if let Err(e) = state.docker.cleanup_containers_by_name(&container.name).await {
            tracing::warn!("Failed to cleanup old containers: {}", e);
        }

        // Replace {{VAR}} placeholders in startup script with actual values
        let startup_script = container.startup_script.as_ref().map(|s| {
            let replaced = replace_startup_placeholders(s, &container.environment, &container.resources);
            tracing::info!("Original startup script: {}", s);
            tracing::info!("Memory limit from resources: {}", container.resources.memory_limit);
            tracing::info!("SERVER_MEMORY from env: {:?}", container.environment.get("SERVER_MEMORY"));
            tracing::info!("Replaced startup script: {}", replaced);
            replaced
        });

        // Create new container with port bindings
        let docker_id = state
            .docker
            .create_container_with_resources(
                &container.name,
                &container.image,
                startup_script.as_deref(),
                if port_bindings.is_empty() { None } else { Some(port_bindings) },
                &container.resources,
                &container.restart_policy,
                container.tty,
            )
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Update state with new docker_id
        update_container_docker_id(&state, &container.name, docker_id.clone());

        // Save state
        save_container_state(&state).await;

        // Check if container needs installation (first time setup)
        // If so, we DON'T start the container here - the WebSocket handler will do installation
        // and then start the container, streaming logs to the client
        if !container.installed && container.install_script.is_some() {
            tracing::info!("Container {} needs installation - will be triggered via WebSocket", container.name);
            return Ok(Json(serde_json::json!({
                "success": true,
                "recreated": true,
                "dockerId": docker_id,
                "needsInstall": true
            })));
        }

        // Start the container (no install needed)
        state.docker
            .start_container(&docker_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        return Ok(Json(serde_json::json!({
            "success": true,
            "recreated": true,
            "dockerId": docker_id
        })));
    }

    // Fallback for containers not in managed state - just try to start
    let docker_id = id.clone();
    tracing::info!("Starting unmanaged container {} directly", docker_id);

    state.docker
        .start_container(&docker_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn stop_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    // Get docker_id - guards dropped immediately
    let docker_id = get_docker_id(&state, &id);

    tracing::info!("Stopping container {} (docker_id: {})", id, docker_id);

    // Use docker stop which sends SIGTERM - the server will shut down gracefully
    // and show shutdown logs, then the container will be properly stopped
    // (preventing "unless-stopped" from restarting it)
    state.docker
        .graceful_stop(&docker_id, 30)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn restart_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    // Get docker_id - guard dropped immediately
    let docker_id = get_docker_id(&state, &id);

    tracing::info!("Restarting container {} (docker_id: {})", id, docker_id);

    // Simple restart
    state.docker
        .restart_container(&docker_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

/// Recreate a container (stop, remove, create, start) to apply new configuration like port bindings
pub async fn recreate_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    // Get container - guard dropped immediately
    let container = get_container_clone(&state, &id)
        .ok_or((StatusCode::NOT_FOUND, "Container not found in managed state".into()))?;

    tracing::info!("Recreating container {} to apply configuration changes", id);

    // Build port bindings from allocations
    // Multiple allocations can map to the same internal port (e.g., multiple IPs)
    let mut port_bindings: HashMap<String, Vec<PortBinding>> = HashMap::new();

    for alloc in &container.allocations {
        let key = format!("{}/{}", alloc.internal_port, alloc.protocol);
        tracing::info!("Adding allocation: {} -> {}:{}", key, alloc.ip, alloc.port);

        // Append to existing bindings for this port (don't replace)
        port_bindings
            .entry(key)
            .or_insert_with(Vec::new)
            .push(PortBinding {
                host_ip: Some(alloc.ip.clone()),
                host_port: Some(alloc.port.to_string()),
            });
    }

    // Legacy allocation support
    if let Some(ref alloc) = container.allocation {
        if container.allocations.is_empty() {
            let key = format!("{}/tcp", alloc.port);
            port_bindings
                .entry(key)
                .or_insert_with(Vec::new)
                .push(PortBinding {
                    host_ip: Some(alloc.ip.clone()),
                    host_port: Some(alloc.port.to_string()),
                });
        }
    }

    tracing::info!("Total port bindings: {:?}", port_bindings);

    // Stop the container gracefully using docker stop (sends SIGTERM)
    // Ignore errors - container might not be running
    let _ = state.docker.graceful_stop(&container.docker_id, 30).await;

    // Clean up ALL old containers with this name (force remove now that it's stopped)
    if let Err(e) = state.docker.cleanup_containers_by_name(&container.name).await {
        tracing::warn!("Failed to cleanup old containers: {}", e);
    }

    // Create new container
    let docker_id = state
        .docker
        .create_container_with_resources(
            &container.name,
            &container.image,
            container.startup_script.as_deref(),
            if port_bindings.is_empty() { None } else { Some(port_bindings) },
            &container.resources,
            &container.restart_policy,
            container.tty,
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Update state with new docker_id
    update_container_docker_id(&state, &container.name, docker_id.clone());

    // Save state
    save_container_state(&state).await;

    // Start the new container
    state.docker
        .start_container(&docker_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "recreated": true,
        "dockerId": docker_id
    })))
}


pub async fn kill_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    let docker_id = get_docker_id(&state, &id);

    state.docker
        .kill_container(&docker_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

// ============================================================================
// CONTAINER COMMANDS AND GRACEFUL STOP
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendCommandRequest {
    pub command: String,
}

pub async fn send_command(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<SendCommandRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    // Get docker_id - guard dropped immediately
    let docker_id = get_docker_id(&state, &id);

    state
        .docker
        .send_command(&docker_id, &req.command)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GracefulStopRequest {
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    30
}

pub async fn graceful_stop_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<GracefulStopRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    // Get docker_id - guard dropped immediately
    let docker_id = get_docker_id(&state, &id);

    // Use docker stop which sends SIGTERM for graceful shutdown
    state
        .docker
        .graceful_stop(&docker_id, req.timeout_secs)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

// ============================================================================
// FTP ACCESS
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFtpRequest {
    pub password: String,
}

pub async fn create_ftp(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<CreateFtpRequest>,
) -> Result<Json<FtpCredentials>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    // Create FTP access with the provided password
    let creds = create_ftp_access(&state.ftp_state, &id, &req.password);

    Ok(Json(creds))
}

// ============================================================================
// ALLOCATIONS
// ============================================================================

pub async fn list_allocations(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<AvailableAllocation>>, StatusCode> {
    if !verify_api_key(&headers, &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Get available IPs from environment or default
    let ips: Vec<String> = std::env::var("AVAILABLE_IPS")
        .unwrap_or_else(|_| "0.0.0.0".into())
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let port_range: Vec<i32> = (25565..=25600).collect();

    // Get used ports - guards dropped immediately
    let used_ports: std::collections::HashSet<i32> = {
        state.containers
            .iter()
            .filter_map(|r| r.value().allocation.as_ref().map(|a| a.port))
            .collect()
    };

    let available_ports: Vec<i32> = port_range
        .into_iter()
        .filter(|p| !used_ports.contains(p))
        .collect();

    let allocations: Vec<AvailableAllocation> = ips
        .into_iter()
        .map(|ip| AvailableAllocation {
            ip,
            ports: available_ports.clone(),
        })
        .collect();

    Ok(Json(allocations))
}

pub async fn assign_allocation(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<AssignAllocationRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    // Update allocation - guard dropped at end of scope
    {
        if let Some(mut container) = state.containers.get_mut(&req.container_name) {
            container.allocation = Some(crate::models::AllocationInfo {
                ip: req.ip,
                port: req.port,
            });
        } else {
            return Err((StatusCode::NOT_FOUND, "Container not found".into()));
        }
    }
    // Guard dropped here

    // Save state asynchronously
    save_container_state(&state).await;

    Ok(Json(serde_json::json!({ "success": true })))
}

// ============================================================================
// WEBSOCKET HANDLERS - with proper cleanup and cancellation
// ============================================================================

pub async fn ws_logs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let api_key = params.get("api_key").cloned().unwrap_or_default();

    if api_key != state.api_key {
        return (StatusCode::UNAUTHORIZED, "Invalid API key").into_response();
    }

    ws.on_upgrade(move |socket| handle_logs_websocket(socket, state, id)).into_response()
}

async fn handle_logs_websocket(socket: WebSocket, state: Arc<AppState>, container_name: String) {
    let (mut sender, mut receiver) = socket.split();

    // Get container info - guards dropped immediately
    let container_info = {
        state.containers.get(&container_name)
            .map(|entry| (
                entry.value().docker_id.clone(),
                entry.value().installed,
                entry.value().install_script.clone(),
                entry.value().image.clone(),
                entry.value().environment.clone(),
            ))
    };

    let (docker_id, installed, install_script, image, environment) = match container_info {
        Some(info) => info,
        None => {
            // Try to find by docker_id prefix
            let found = state.containers.iter()
                .find(|entry| entry.value().docker_id.starts_with(&container_name))
                .map(|entry| (
                    entry.value().docker_id.clone(),
                    entry.value().installed,
                    entry.value().install_script.clone(),
                    entry.value().image.clone(),
                    entry.value().environment.clone(),
                ));
            match found {
                Some(info) => info,
                None => {
                    let _ = sender.send(Message::Text("\x1b[31m● Container not found\x1b[0m".to_string())).await;
                    return;
                }
            }
        }
    };

    // Send initial connection message
    if sender.send(Message::Text(format!("\x1b[32m● Connected to container: {}\x1b[0m", container_name))).await.is_err() {
        return;
    }

    let (tx, mut rx) = broadcast::channel::<String>(1000);

    // Check if installation is needed
    if !installed {
        if let Some(script) = install_script {
            // Send install starting message
            let _ = sender.send(Message::Text("\x1b[33m● Starting installation...\x1b[0m".to_string())).await;

            let tx_for_install = tx.clone();
            let container_name_clone = container_name.clone();
            let state_clone = state.clone();

            // Run installation with logs going to the broadcast channel
            tracing::info!("Running install script for {} via WebSocket", container_name);

            // Create the install future
            let install_fut = state.docker.run_install_in_temp_container_with_logs(
                &container_name,
                &image,
                &script,
                &environment,
                Some(tx_for_install),
            );

            // Pin the future for use in select
            tokio::pin!(install_fut);

            // Forward install logs to WebSocket while install is running
            let install_result = loop {
                tokio::select! {
                    biased;

                    result = &mut install_fut => {
                        // Drain remaining logs
                        while let Ok(log) = rx.try_recv() {
                            let _ = sender.send(Message::Text(log)).await;
                        }
                        break result;
                    }
                    log_result = rx.recv() => {
                        match log_result {
                            Ok(log) => {
                                if sender.send(Message::Text(log)).await.is_err() {
                                    return;
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => continue,
                        }
                    }
                }
            };

            match install_result {
                Ok(_) => {
                    tracing::info!("Install completed for {}", container_name);
                    mark_container_installed(&state_clone, &container_name_clone);
                    save_container_state(&state_clone).await;

                    // Don't auto-start - let the user click Start
                    let _ = sender.send(Message::Text("\x1b[32m● Installation complete! Click Start to launch the server.\x1b[0m".to_string())).await;

                    // Keep connection open but don't start the container
                    // The user will click Start which triggers the HTTP endpoint
                }
                Err(e) => {
                    tracing::error!("Install failed for {}: {}", container_name, e);
                    mark_container_installed(&state_clone, &container_name_clone);
                    save_container_state(&state_clone).await;
                    let _ = sender.send(Message::Text(format!("\x1b[31m● Installation failed: {}\x1b[0m", e))).await;
                    return;
                }
            }
        }
    }

    // Get the current docker_id after potential install
    let docker_id = get_docker_id(&state, &container_name);

    // Start streaming logs
    state.docker.stream_logs(&docker_id, tx);


    // Clone for the receive task
    let docker_id_for_cmd = docker_id.clone();
    let state_for_cmd = state.clone();

    // Use tokio::select! for clean cancellation
    let send_task = async {
        loop {
            match rx.recv().await {
                Ok(log) => {
                    if sender.send(Message::Text(log)).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Log receiver lagged by {} messages", n);
                    // Continue receiving
                }
                Err(broadcast::error::RecvError::Closed) => {
                    // Stream ended - send close message
                    let _ = sender.send(Message::Close(None)).await;
                    break;
                }
            }
        }
    };

    let recv_task = async {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Handle incoming command from client
                    let text = text.trim();
                    if !text.is_empty() {
                        tracing::info!("Received command for container {}: {}", docker_id_for_cmd, text);
                        if let Err(e) = state_for_cmd.docker.send_command(&docker_id_for_cmd, text).await {
                            tracing::error!("Failed to send command: {}", e);
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }
    };

    // Both tasks complete cleanly when either finishes
    tokio::select! {
        _ = send_task => {
            tracing::debug!("Log send task completed for {}", container_name);
        },
        _ = recv_task => {
            tracing::debug!("Log receive task completed for {}", container_name);
        },
    }

    tracing::debug!("WebSocket logs handler completed for {}", container_name);
}

// ============================================================================
// SYSTEM RESOURCES
// ============================================================================

pub async fn get_system_resources(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<crate::models::SystemResources>, StatusCode> {
    if !verify_api_key(&headers, &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Use spawn_blocking for sysinfo operations which may block
    let result = tokio::task::spawn_blocking(|| {
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();
        sys.refresh_memory();
        sys.refresh_cpu_all();

        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let available_memory = total_memory.saturating_sub(used_memory);
        let cpu_cores = sys.cpus().len();
        let cpu_usage = sys.global_cpu_usage() as f64;

        let disks = sysinfo::Disks::new_with_refreshed_list();
        let (total_disk, available_disk) = disks.iter().fold((0u64, 0u64), |(total, avail), disk| {
            (total + disk.total_space(), avail + disk.available_space())
        });

        let hostname = sysinfo::System::host_name().unwrap_or_else(|| "unknown".to_string());

        crate::models::SystemResources {
            total_memory,
            available_memory,
            cpu_cores,
            cpu_usage,
            total_disk,
            available_disk,
            hostname,
        }
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(result))
}

pub async fn ws_system_stats(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let api_key = params.get("api_key").cloned().unwrap_or_default();

    if api_key != state.api_key {
        return (StatusCode::UNAUTHORIZED, "Invalid API key").into_response();
    }

    ws.on_upgrade(|socket| handle_system_stats_socket(socket))
}

async fn handle_system_stats_socket(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();

    let send_task = async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));

        // Use spawn_blocking for the initial system creation
        let sys = tokio::task::spawn_blocking(|| {
            let mut sys = sysinfo::System::new_all();
            sys.refresh_all();
            sys
        })
        .await;

        let mut sys = match sys {
            Ok(s) => s,
            Err(_) => return,
        };

        loop {
            interval.tick().await;

            // Use spawn_blocking for refreshing system stats
            let stats = {
                // Clone isn't available, so we need to refresh in the blocking thread
                let stats_result = tokio::task::spawn_blocking(move || {
                    sys.refresh_memory();
                    sys.refresh_cpu_all();

                    let total_memory = sys.total_memory();
                    let used_memory = sys.used_memory();
                    let available_memory = total_memory.saturating_sub(used_memory);
                    let cpu_cores = sys.cpus().len();
                    let cpu_usage = sys.global_cpu_usage() as f64;

                    let disks = sysinfo::Disks::new_with_refreshed_list();
                    let (total_disk, available_disk) = disks.iter().fold((0u64, 0u64), |(total, avail), disk| {
                        (total + disk.total_space(), avail + disk.available_space())
                    });

                    let hostname = sysinfo::System::host_name().unwrap_or_else(|| "unknown".to_string());

                    let stats = crate::models::SystemResources {
                        total_memory,
                        available_memory,
                        cpu_cores,
                        cpu_usage,
                        total_disk,
                        available_disk,
                        hostname,
                    };

                    (sys, stats)
                })
                .await;

                match stats_result {
                    Ok((returned_sys, stats)) => {
                        sys = returned_sys;
                        stats
                    }
                    Err(_) => return,
                }
            };

            let json = serde_json::to_string(&stats).unwrap_or_default();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    };

    let recv_task = async {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    };

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

// ============================================================================
// CONTAINER STATS
// ============================================================================

pub async fn get_container_stats(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<crate::models::ContainerStats>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    // Get docker_id - guard dropped immediately
    let docker_id = get_docker_id(&state, &id);

    state
        .docker
        .get_container_stats(&docker_id)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerStatusResponse {
    pub status: String,
    pub running: bool,
    pub exit_code: Option<i64>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

pub async fn get_container_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ContainerStatusResponse>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    // Get docker_id - guard dropped immediately
    let docker_id = get_docker_id(&state, &id);

    // Get container info from Docker
    let info = state.docker.get_container(&docker_id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let running = info.state.to_lowercase() == "running";

    Ok(Json(ContainerStatusResponse {
        status: info.state.clone(),
        running,
        exit_code: None,
        started_at: None,
        finished_at: None,
    }))
}

pub async fn ws_container_stats(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let api_key = params.get("api_key").cloned().unwrap_or_default();

    if api_key != state.api_key {
        return (StatusCode::UNAUTHORIZED, "Invalid API key").into_response();
    }

    ws.on_upgrade(move |socket| handle_container_stats_socket(socket, state, id))
}

async fn handle_container_stats_socket(socket: WebSocket, state: Arc<AppState>, container_name: String) {
    let (mut sender, mut receiver) = socket.split();

    // Find docker_id - guards dropped immediately
    let docker_id = {
        state.containers.iter()
            .find(|entry| entry.key() == &container_name || entry.value().docker_id.starts_with(&container_name))
            .map(|entry| entry.value().docker_id.clone())
            .unwrap_or_else(|| container_name.clone())
    };

    let (tx, mut rx) = broadcast::channel::<String>(100);

    state.docker.stream_container_stats(&docker_id, tx);

    let send_task = async {
        while let Ok(stats) = rx.recv().await {
            if sender.send(Message::Text(stats)).await.is_err() {
                break;
            }
        }
    };

    let recv_task = async {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = send_task => {
            tracing::debug!("Container stats send task completed for {}", container_name);
        },
        _ = recv_task => {
            tracing::debug!("Container stats receive task completed for {}", container_name);
        },
    }

    tracing::debug!("WebSocket container stats handler completed for {}", container_name);
}

// ============================================================================
// FILE MANAGEMENT
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilesQuery {
    pub path: Option<String>,
}

pub async fn list_files(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(container_name): Path<String>,
    Query(query): Query<ListFilesQuery>,
) -> Result<Json<Vec<FileEntry>>, StatusCode> {
    if !verify_api_key(&headers, &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Use same base path as FTP and Docker volume mounts
    let base_path = std::env::var("FTP_BASE_PATH")
        .unwrap_or_else(|_| std::env::var("SFTP_BASE_PATH")
            .unwrap_or_else(|_| "/data/raptor".into()));
    let container_path = std::path::Path::new(&base_path).join("volumes").join(&container_name);
    let rel_path = query.path.unwrap_or_else(|| "/".into());
    let full_path = container_path.join(rel_path.trim_start_matches('/'));

    tracing::info!("list_files: base_path={}, container_path={:?}, full_path={:?}", base_path, container_path, full_path);

    if !full_path.starts_with(&container_path) {
        return Err(StatusCode::FORBIDDEN);
    }

    if !full_path.exists() {
        tokio::fs::create_dir_all(&full_path).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(&full_path).await.map_err(|_| StatusCode::NOT_FOUND)?;

    while let Ok(Some(entry)) = dir.next_entry().await {
        let metadata = entry.metadata().await.ok();
        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified = metadata.and_then(|m| m.modified().ok()).map(|t| {
            chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()
        });

        entries.push(FileEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            is_dir,
            size,
            modified,
        });
    }

    entries.sort_by(|a, b| {
        if a.is_dir == b.is_dir {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        } else if a.is_dir {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });

    Ok(Json(entries))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileQuery {
    pub path: String,
}

pub async fn read_file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(container_name): Path<String>,
    Query(query): Query<ReadFileQuery>,
) -> Result<String, StatusCode> {
    if !verify_api_key(&headers, &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let base_path = std::env::var("FTP_BASE_PATH")
        .unwrap_or_else(|_| std::env::var("SFTP_BASE_PATH")
            .unwrap_or_else(|_| "/data/raptor".into()));
    let container_path = std::path::Path::new(&base_path).join("volumes").join(&container_name);
    let full_path = container_path.join(query.path.trim_start_matches('/'));

    if !full_path.starts_with(&container_path) {
        return Err(StatusCode::FORBIDDEN);
    }

    tokio::fs::read_to_string(&full_path).await.map_err(|_| StatusCode::NOT_FOUND)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileRequest {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub encoding: Option<String>,
}

pub async fn write_file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(container_name): Path<String>,
    Json(req): Json<WriteFileRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !verify_api_key(&headers, &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let base_path = std::env::var("FTP_BASE_PATH")
        .unwrap_or_else(|_| std::env::var("SFTP_BASE_PATH")
            .unwrap_or_else(|_| "/data/raptor".into()));
    let container_path = std::path::Path::new(&base_path).join("volumes").join(&container_name);
    let full_path = container_path.join(req.path.trim_start_matches('/'));

    tracing::info!("write_file: container={}, path={}, full_path={:?}", container_name, req.path, full_path);

    if !full_path.starts_with(&container_path) {
        tracing::warn!("write_file: path traversal attempt blocked");
        return Err(StatusCode::FORBIDDEN);
    }

    if let Some(parent) = full_path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            tracing::error!("write_file: failed to create parent dirs: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    // Handle base64 encoding for binary files
    let content_bytes = if req.encoding.as_deref() == Some("base64") {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(&req.content)
            .map_err(|e| {
                tracing::error!("write_file: failed to decode base64: {}", e);
                StatusCode::BAD_REQUEST
            })?
    } else {
        req.content.into_bytes()
    };

    if let Err(e) = tokio::fs::write(&full_path, &content_bytes).await {
        tracing::error!("write_file: failed to write file {:?}: {}", full_path, e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    tracing::info!("write_file: saved {} bytes to {:?}", content_bytes.len(), full_path);
    Ok(Json(serde_json::json!({"message": "File saved successfully"})))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFolderRequest {
    pub path: String,
}

pub async fn create_folder(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(container_name): Path<String>,
    Json(req): Json<CreateFolderRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !verify_api_key(&headers, &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let base_path = std::env::var("FTP_BASE_PATH")
        .unwrap_or_else(|_| std::env::var("SFTP_BASE_PATH")
            .unwrap_or_else(|_| "/data/raptor".into()));
    let container_path = std::path::Path::new(&base_path).join("volumes").join(&container_name);
    let full_path = container_path.join(req.path.trim_start_matches('/'));

    if !full_path.starts_with(&container_path) {
        return Err(StatusCode::FORBIDDEN);
    }

    tokio::fs::create_dir_all(&full_path).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({"message": "Folder created successfully"})))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFileQuery {
    pub path: String,
}

pub async fn delete_file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(container_name): Path<String>,
    Query(query): Query<DeleteFileQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !verify_api_key(&headers, &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let base_path = std::env::var("FTP_BASE_PATH")
        .unwrap_or_else(|_| std::env::var("SFTP_BASE_PATH")
            .unwrap_or_else(|_| "/data/raptor".into()));
    let container_path = std::path::Path::new(&base_path).join("volumes").join(&container_name);
    let full_path = container_path.join(query.path.trim_start_matches('/'));

    if !full_path.starts_with(&container_path) {
        return Err(StatusCode::FORBIDDEN);
    }

    if full_path.is_dir() {
        tokio::fs::remove_dir_all(&full_path).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        tokio::fs::remove_file(&full_path).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(serde_json::json!({"message": "Deleted successfully"})))
}

// ============================================================================
// CHUNKED UPLOAD SUPPORT
// ============================================================================

use std::collections::HashMap as StdHashMap;
use tokio::sync::Mutex;
use once_cell::sync::Lazy;

/// Storage for chunked uploads in progress on the daemon
struct DaemonChunkUpload {
    path: String,
    total_chunks: u32,
    received_chunks: StdHashMap<u32, bool>,
    temp_dir: PathBuf,
    created_at: std::time::Instant,
}

static DAEMON_CHUNK_STORAGE: Lazy<Mutex<StdHashMap<String, DaemonChunkUpload>>> = 
    Lazy::new(|| Mutex::new(StdHashMap::new()));

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteChunkRequest {
    pub upload_id: String,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub path: String,
    pub content: String, // Base64 encoded chunk
}

/// Write a single chunk to disk
pub async fn write_file_chunk(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(container_name): Path<String>,
    Json(req): Json<WriteChunkRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !verify_api_key(&headers, &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let base_path = std::env::var("FTP_BASE_PATH")
        .unwrap_or_else(|_| std::env::var("SFTP_BASE_PATH")
            .unwrap_or_else(|_| "/data/raptor".into()));
    let container_path = std::path::Path::new(&base_path).join("volumes").join(&container_name);
    let final_path = container_path.join(req.path.trim_start_matches('/'));

    if !final_path.starts_with(&container_path) {
        tracing::warn!("write_file_chunk: path traversal attempt blocked");
        return Err(StatusCode::FORBIDDEN);
    }

    // Decode chunk content
    use base64::Engine;
    let chunk_data = base64::engine::general_purpose::STANDARD
        .decode(&req.content)
        .map_err(|e| {
            tracing::error!("write_file_chunk: failed to decode base64: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    let storage_key = format!("{}:{}", container_name, req.upload_id);
    let mut storage = DAEMON_CHUNK_STORAGE.lock().await;

    // Clean up old uploads (older than 30 minutes)
    let now = std::time::Instant::now();
    let keys_to_remove: Vec<_> = storage
        .iter()
        .filter(|(_, upload)| now.duration_since(upload.created_at).as_secs() > 1800)
        .map(|(k, _)| k.clone())
        .collect();
    for key in keys_to_remove {
        if let Some(upload) = storage.remove(&key) {
            let _ = tokio::fs::remove_dir_all(&upload.temp_dir).await;
        }
    }

    // Create or get upload tracking
    let temp_dir = container_path.join(".uploads").join(&req.upload_id);
    
    let upload = storage.entry(storage_key.clone()).or_insert_with(|| {
        DaemonChunkUpload {
            path: req.path.clone(),
            total_chunks: req.total_chunks,
            received_chunks: StdHashMap::new(),
            temp_dir: temp_dir.clone(),
            created_at: now,
        }
    });

    // Ensure temp directory exists
    if let Err(e) = tokio::fs::create_dir_all(&upload.temp_dir).await {
        tracing::error!("write_file_chunk: failed to create temp dir: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Write chunk to temp file
    let chunk_file = upload.temp_dir.join(format!("chunk_{:06}", req.chunk_index));
    if let Err(e) = tokio::fs::write(&chunk_file, &chunk_data).await {
        tracing::error!("write_file_chunk: failed to write chunk: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    upload.received_chunks.insert(req.chunk_index, true);
    
    tracing::info!(
        "write_file_chunk: received chunk {}/{} for upload {} (path: {})",
        req.chunk_index + 1,
        req.total_chunks,
        req.upload_id,
        req.path
    );

    // Check if all chunks received
    if upload.received_chunks.len() == req.total_chunks as usize {
        // Assemble the file
        let temp_dir = upload.temp_dir.clone();
        let path = upload.path.clone();
        let total = req.total_chunks;
        
        // Remove from storage before assembling
        storage.remove(&storage_key);
        drop(storage); // Release lock

        // Ensure parent directory exists
        if let Some(parent) = final_path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                tracing::error!("write_file_chunk: failed to create parent dirs: {}", e);
                let _ = tokio::fs::remove_dir_all(&temp_dir).await;
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }

        // Create or truncate final file
        let mut final_file = match tokio::fs::File::create(&final_path).await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("write_file_chunk: failed to create final file: {}", e);
                let _ = tokio::fs::remove_dir_all(&temp_dir).await;
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        // Append all chunks in order
        use tokio::io::AsyncWriteExt;
        let mut total_bytes = 0u64;
        for i in 0..total {
            let chunk_file = temp_dir.join(format!("chunk_{:06}", i));
            match tokio::fs::read(&chunk_file).await {
                Ok(data) => {
                    total_bytes += data.len() as u64;
                    if let Err(e) = final_file.write_all(&data).await {
                        tracing::error!("write_file_chunk: failed to write to final file: {}", e);
                        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
                Err(e) => {
                    tracing::error!("write_file_chunk: failed to read chunk {}: {}", i, e);
                    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        }

        // Flush and sync
        if let Err(e) = final_file.sync_all().await {
            tracing::error!("write_file_chunk: failed to sync file: {}", e);
        }

        // Clean up temp directory
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;

        tracing::info!(
            "write_file_chunk: assembled file {} ({} bytes) from {} chunks",
            path,
            total_bytes,
            total
        );

        return Ok(Json(serde_json::json!({
            "message": "File uploaded successfully",
            "complete": true,
            "totalBytes": total_bytes
        })));
    }

    Ok(Json(serde_json::json!({
        "message": format!("Chunk {} of {} received", req.chunk_index + 1, req.total_chunks),
        "complete": false,
        "received": upload.received_chunks.len(),
        "total": req.total_chunks
    })))
}

/// Database server handler functions
/// These handlers manage the lifecycle of database servers (e.g., MySQL, PostgreSQL)
/// They are separate from container handlers as databases may have different lifecycles

// ============================================================================
// DATABASE SERVER HANDLERS
// ============================================================================

pub async fn list_database_servers(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DatabaseServer>>, StatusCode> {
    Ok(Json(state.database_manager.list_servers()))
}

pub async fn get_database_server(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DatabaseServer>, StatusCode> {
    state.database_manager.get_server(&id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn create_database_server(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateDatabaseServerRequest>,
) -> Result<Json<DatabaseServer>, (StatusCode, String)> {
    let server = DatabaseServer {
        id: req.id,
        db_type: req.db_type,
        container_id: None,
        container_name: req.container_name,
        host: req.host,
        port: req.port,
        root_password: req.root_password,
        status: "stopped".to_string(),
    };

    state.database_manager.add_server(server.clone());
    state.database_manager.save_state().await;

    Ok(Json(server))
}

pub async fn delete_database_server(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let server = state.database_manager.get_server(&id)
        .ok_or((StatusCode::NOT_FOUND, "Server not found".to_string()))?;

    // Delete the container if it exists
    if let Err(e) = database_manager::delete_database_container(&server).await {
        tracing::warn!("Failed to delete database container: {}", e);
    }

    state.database_manager.remove_server(&id);
    state.database_manager.save_state().await;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn start_database_server(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DatabaseServer>, (StatusCode, String)> {
    let server = state.database_manager.get_server(&id)
        .ok_or((StatusCode::NOT_FOUND, "Server not found".to_string()))?;

    let container_id = database_manager::create_and_start_database_container(&server)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    state.database_manager.update_server_status(&id, "running", Some(container_id));
    state.database_manager.save_state().await;

    let updated_server = state.database_manager.get_server(&id)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Failed to get updated server".to_string()))?;

    Ok(Json(updated_server))
}

pub async fn stop_database_server(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DatabaseServer>, (StatusCode, String)> {
    let server = state.database_manager.get_server(&id)
        .ok_or((StatusCode::NOT_FOUND, "Server not found".to_string()))?;

    database_manager::stop_database_container(&server)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    state.database_manager.update_server_status(&id, "stopped", None);
    state.database_manager.save_state().await;

    let updated_server = state.database_manager.get_server(&id)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Failed to get updated server".to_string()))?;

    Ok(Json(updated_server))
}

pub async fn restart_database_server(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DatabaseServer>, (StatusCode, String)> {
    let server = state.database_manager.get_server(&id)
        .ok_or((StatusCode::NOT_FOUND, "Server not found".to_string()))?;

    // Stop first (ignore errors if not running)
    let _ = database_manager::stop_database_container(&server).await;
    state.database_manager.update_server_status(&id, "restarting", None);

    // Start
    let container_id = database_manager::create_and_start_database_container(&server)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    state.database_manager.update_server_status(&id, "running", Some(container_id));
    state.database_manager.save_state().await;

    let updated_server = state.database_manager.get_server(&id)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Failed to get updated server".to_string()))?;

    Ok(Json(updated_server))
}

pub async fn create_user_database(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<CreateUserDatabaseRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let server = state.database_manager.get_server(&id)
        .ok_or((StatusCode::NOT_FOUND, "Server not found".to_string()))?;

    if server.status != "running" {
        return Err((StatusCode::BAD_REQUEST, "Database server is not running".to_string()));
    }

    database_manager::create_user_database(&server, &req.db_name, &req.db_user, &req.db_password)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({
        "message": "Database created successfully"
    })))
}

pub async fn delete_user_database(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<DeleteUserDatabaseRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let server = state.database_manager.get_server(&id)
        .ok_or((StatusCode::NOT_FOUND, "Server not found".to_string()))?;

    if server.status != "running" {
        return Err((StatusCode::BAD_REQUEST, "Database server is not running".to_string()));
    }

    database_manager::delete_user_database(&server, &req.db_name, &req.db_user)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({
        "message": "Database deleted successfully"
    })))
}

pub async fn reset_user_database_password(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<ResetPasswordRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let server = state.database_manager.get_server(&id)
        .ok_or((StatusCode::NOT_FOUND, "Server not found".to_string()))?;

    if server.status != "running" {
        return Err((StatusCode::BAD_REQUEST, "Database server is not running".to_string()));
    }

    database_manager::reset_user_database_password(&server, &req.db_name, &req.db_user, &req.new_password)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({
        "message": "Password reset successfully"
    })))
}
