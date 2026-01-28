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

/// Get the path to the container state file
fn get_state_file_path() -> PathBuf {
    let data_dir = std::env::var("DAEMON_DATA_DIR")
        .unwrap_or_else(|_| "/var/lib/raptor-daemon".to_string());
    PathBuf::from(data_dir).join("containers.json")
}

/// Save container state to disk
pub async fn save_container_state(state: &AppState) {
    let containers: Vec<ManagedContainer> = state
        .containers
        .iter()
        .map(|r| r.value().clone())
        .collect();

    let state_file = get_state_file_path();

    // Ensure parent directory exists
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
                tracing::debug!("Container state saved to {:?}", state_file);
            }
        }
        Err(e) => {
            tracing::error!("Failed to serialize container state: {}", e);
        }
    }
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

fn verify_api_key(headers: &HeaderMap, state: &AppState) -> bool {
    headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(|k| k == state.api_key)
        .unwrap_or(false)
}

pub async fn list_containers(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<ManagedContainer>>, StatusCode> {
    if !verify_api_key(&headers, &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Get managed containers from state
    let containers: Vec<ManagedContainer> = state
        .containers
        .iter()
        .map(|r| r.value().clone())
        .collect();

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

    let mut port_bindings: HashMap<String, Vec<PortBinding>> = HashMap::new();

    // Handle multiple allocations (new model)
    for alloc in &req.allocations {
        let key = format!("{}/{}", alloc.internal_port, alloc.protocol);
        tracing::info!("Adding allocation: {} -> {}:{}", key, alloc.ip, alloc.port);
        port_bindings.insert(
            key,
            vec![PortBinding {
                host_ip: Some(alloc.ip.clone()),
                host_port: Some(alloc.port.to_string()),
            }],
        );
    }

    // Legacy single allocation support
    if let Some(ref alloc) = req.allocation {
        if req.allocations.is_empty() {
            let key = format!("{}/tcp", alloc.port);
            tracing::info!("Adding legacy allocation: {} -> {}:{}", key, alloc.ip, alloc.port);
            port_bindings.insert(
                key,
                vec![PortBinding {
                    host_ip: Some(alloc.ip.clone()),
                    host_port: Some(alloc.port.to_string()),
                }],
            );
        }
    }

    for port in &req.ports {
        let key = format!("{}/{}", port.container_port, port.protocol);
        port_bindings.insert(
            key,
            vec![PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some(port.host_port.to_string()),
            }],
        );
    }

    tracing::info!("Total port bindings: {:?}", port_bindings.keys().collect::<Vec<_>>());

    let resources = crate::models::ContainerResources {
        memory_limit: req.memory_limit,
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
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let managed = ManagedContainer {
        name: req.name.clone(),
        docker_id,
        image: req.image,
        startup_script: req.startup_script,
        stop_command: req.stop_command,
        allocation: req.allocation,
        allocations: req.allocations,
        ports: req.ports,
        resources,
        sftp_user: None,
        sftp_pass: None,
    };

    state.containers.insert(req.name.clone(), managed.clone());

    // Save container state to disk
    save_container_state(&state).await;

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

    // Get from managed state
    if let Some(container) = state.containers.get(&id) {
        // Also get real Docker status
        if let Ok(docker_info) = state.docker.get_container(&container.docker_id).await {
            tracing::debug!("Docker container {} status: {}", id, docker_info.status);
        }
        return Ok(Json(container.value().clone()));
    }

    Err(StatusCode::NOT_FOUND)
}

pub async fn delete_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<()>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    let container = state
        .containers
        .get(&id)
        .map(|r| r.value().clone())
        .ok_or((StatusCode::NOT_FOUND, "Container not found".into()))?;

    state
        .docker
        .remove_container(&container.docker_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state.containers.remove(&id);

    // Save container state to disk
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

    // Get the container from managed state
    let mut container = state
        .containers
        .get(&id)
        .map(|r| r.value().clone())
        .ok_or((StatusCode::NOT_FOUND, "Container not found".into()))?;

    // Update resources if provided
    if let Some(memory) = req.memory_limit {
        container.resources.memory_limit = memory;
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

    // Update allocations array if provided (new model)
    if let Some(allocations) = req.allocations {
        container.allocations = allocations;
    }

    // Update ports if provided
    if let Some(ports) = req.ports {
        container.ports = ports;
    }

    // Try to update Docker container resources (only works on running containers)
    if let Err(e) = state.docker.update_container_resources(
        &container.docker_id,
        &container.resources,
    ).await {
        tracing::warn!("Failed to update Docker container resources (container might be stopped): {}", e);
        // Don't fail - we still update the managed state
    }

    // Save updated container to state
    state.containers.insert(id.clone(), container.clone());

    // Save container state to disk
    save_container_state(&state).await;

    Ok(Json(container))
}

async fn container_action(
    state: &AppState,
    id: &str,
    action: &str,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Try to get docker_id from managed state first, fall back to using id directly
    let docker_id = state
        .containers
        .get(id)
        .map(|r| r.value().docker_id.clone())
        .unwrap_or_else(|| id.to_string());

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

/// Recreate a container with its stored allocations
async fn recreate_container_with_allocations(
    state: &AppState,
    container: &ManagedContainer,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut port_bindings: HashMap<String, Vec<PortBinding>> = HashMap::new();

    // Build port bindings from allocations
    for alloc in &container.allocations {
        let key = format!("{}/{}", alloc.internal_port, alloc.protocol);
        tracing::info!("Recreating with allocation: {} -> {}:{}", key, alloc.ip, alloc.port);
        port_bindings.insert(
            key,
            vec![PortBinding {
                host_ip: Some(alloc.ip.clone()),
                host_port: Some(alloc.port.to_string()),
            }],
        );
    }

    // Legacy allocation support
    if let Some(ref alloc) = container.allocation {
        if container.allocations.is_empty() {
            let key = format!("{}/tcp", alloc.port);
            port_bindings.insert(
                key,
                vec![PortBinding {
                    host_ip: Some(alloc.ip.clone()),
                    host_port: Some(alloc.port.to_string()),
                }],
            );
        }
    }

    // Add any other port mappings
    for port in &container.ports {
        let key = format!("{}/{}", port.container_port, port.protocol);
        port_bindings.insert(
            key,
            vec![PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some(port.host_port.to_string()),
            }],
        );
    }

    tracing::info!("Recreating container with port bindings: {:?}", port_bindings.keys().collect::<Vec<_>>());

    // Try to remove old container if it exists
    let _ = state.docker.remove_container(&container.docker_id).await;

    // Create new container
    let docker_id = state
        .docker
        .create_container_with_resources(
            &container.name,
            &container.image,
            container.startup_script.as_deref(),
            if port_bindings.is_empty() { None } else { Some(port_bindings) },
            &container.resources,
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Update managed container with new docker_id
    if let Some(mut managed) = state.containers.get_mut(&container.name) {
        managed.docker_id = docker_id.clone();
    }

    // Save state
    save_container_state(state).await;

    // Start the new container
    state
        .docker
        .start_container(&docker_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true, "recreated": true, "dockerId": docker_id })))
}

pub async fn stop_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }
    container_action(&state, &id, "stop").await
}

pub async fn restart_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }
    container_action(&state, &id, "restart").await
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

    let container = state
        .containers
        .get(&id)
        .map(|r| r.value().clone())
        .ok_or((StatusCode::NOT_FOUND, "Container not found in managed state".into()))?;

    tracing::info!("Recreating container {} to apply configuration changes", id);

    // Stop the container first if running
    let _ = state.docker.stop_container(&container.docker_id).await;

    // Recreate with allocations
    recreate_container_with_allocations(&state, &container).await
}

pub async fn kill_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }
    container_action(&state, &id, "kill").await
}

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

    // Try to get docker_id from managed state first, fall back to using id directly
    let docker_id = state
        .containers
        .get(&id)
        .map(|r| r.value().docker_id.clone())
        .unwrap_or_else(|| id.to_string());

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
    #[serde(default)]
    pub stop_command: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    10
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

    // Try to get docker_id from managed state first, fall back to using id directly
    let docker_id = state
        .containers
        .get(&id)
        .map(|r| r.value().docker_id.clone())
        .unwrap_or_else(|| id.to_string());

    state
        .docker
        .graceful_stop(&docker_id, req.stop_command.as_deref(), req.timeout_secs)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

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

    // Update container with FTP info
    if let Some(mut container) = state.containers.get_mut(&id) {
        container.sftp_user = Some(creds.user.clone());
        container.sftp_pass = Some(creds.pass.clone());
    }

    Ok(Json(creds))
}

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

    // Filter out used ports
    let used_ports: std::collections::HashSet<i32> = state
        .containers
        .iter()
        .filter_map(|r| r.value().allocation.as_ref().map(|a| a.port))
        .collect();

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

    if let Some(mut container) = state.containers.get_mut(&req.container_name) {
        container.allocation = Some(crate::models::AllocationInfo {
            ip: req.ip,
            port: req.port,
        });
    } else {
        return Err((StatusCode::NOT_FOUND, "Container not found".into()));
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

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

    // Try to find docker_id from managed state, otherwise use container_name directly
    let docker_id = state.containers.iter()
        .find(|entry| entry.key() == &container_name || entry.value().docker_id.starts_with(&container_name))
        .map(|entry| entry.value().docker_id.clone())
        .unwrap_or_else(|| container_name.clone());

    // Send initial connection message
    let _ = sender.send(Message::Text(format!("\x1b[32m● Connected to container: {}\x1b[0m", container_name))).await;

    let (tx, mut rx) = broadcast::channel::<String>(1000);

    // Start streaming logs (handles both running and stopped containers)
    state.docker.stream_logs(&docker_id, tx);

    // Small delay to let the stream task start
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Clone docker_id for the receive task
    let docker_id_for_cmd = docker_id.clone();
    let state_for_cmd = state.clone();

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

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

pub async fn get_system_resources(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<crate::models::SystemResources>, StatusCode> {
    if !verify_api_key(&headers, &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();
    std::thread::sleep(std::time::Duration::from_millis(100));
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

    Ok(Json(crate::models::SystemResources {
        total_memory,
        available_memory,
        cpu_cores,
        cpu_usage,
        total_disk,
        available_disk,
        hostname,
    }))
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

    let send_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
        let mut sys = sysinfo::System::new_all();

        // Initial refresh to get baseline values
        sys.refresh_all();
        std::thread::sleep(std::time::Duration::from_millis(200));

        loop {
            interval.tick().await;

            // Refresh memory and CPU separately for more accurate readings
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

            let json = serde_json::to_string(&stats).unwrap_or_default();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

pub async fn get_container_stats(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<crate::models::ContainerStats>, (StatusCode, String)> {
    if !verify_api_key(&headers, &state) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

    // Try to get docker_id from managed state first
    let docker_id = state
        .containers
        .get(&id)
        .map(|r| r.value().docker_id.clone())
        .unwrap_or_else(|| id.clone()); // Fall back to using id directly as docker name/id

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

    // Try to get docker_id from managed state first
    let docker_id = state
        .containers
        .get(&id)
        .map(|r| r.value().docker_id.clone())
        .unwrap_or_else(|| id.clone());

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

    let container = state.containers.iter()
        .find(|entry| entry.key() == &container_name || entry.value().docker_id.starts_with(&container_name))
        .map(|entry| entry.value().clone());

    let docker_id = match container {
        Some(c) => c.docker_id,
        None => container_name.clone(),
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
        _ = send_task => {},
        _ = recv_task => {},
    }
}

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

    // Volumes are stored in base_path/volumes/{container_id}
    let base_path = std::env::var("FTP_BASE_PATH").unwrap_or_else(|_| "/data/raptor".into());
    let container_path = std::path::Path::new(&base_path).join("volumes").join(&container_name);
    let rel_path = query.path.unwrap_or_else(|| "/".into());
    let full_path = container_path.join(rel_path.trim_start_matches('/'));

    tracing::debug!("list_files: base_path={}, container_path={:?}, full_path={:?}", base_path, container_path, full_path);

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

    let base_path = std::env::var("FTP_BASE_PATH").unwrap_or_else(|_| "/data/raptor".into());
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

    let base_path = std::env::var("FTP_BASE_PATH").unwrap_or_else(|_| "/data/raptor".into());
    let container_path = std::path::Path::new(&base_path).join("volumes").join(&container_name);
    let full_path = container_path.join(req.path.trim_start_matches('/'));

    if !full_path.starts_with(&container_path) {
        return Err(StatusCode::FORBIDDEN);
    }

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    tokio::fs::write(&full_path, req.content).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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

    let base_path = std::env::var("FTP_BASE_PATH").unwrap_or_else(|_| "/data/raptor".into());
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

    let base_path = std::env::var("FTP_BASE_PATH").unwrap_or_else(|_| "/data/raptor".into());
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

