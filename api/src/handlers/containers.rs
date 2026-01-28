use axum::{
    extract::{Path, Query, State},
    Extension,
    Json,
};
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{AppState, Claims, Container, ContainerPort, CreateContainerRequest, Daemon};

/// Allocation info for container responses
#[derive(Debug, Serialize, Clone, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AllocationInfo {
    pub id: Uuid,
    pub allocation_id: Option<Uuid>,
    pub ip: String,
    pub port: i32,
    pub internal_port: i32,
    pub protocol: String,
    pub is_primary: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerResponse {
    #[serde(flatten)]
    pub container: Container,
    pub allocations: Vec<AllocationInfo>,
    /// Primary allocation IP (convenience field)
    pub allocation_ip: Option<String>,
    /// Primary allocation port (convenience field)
    pub allocation_port: Option<i32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ContainerWithAllocation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub daemon_id: Uuid,
    pub name: String,
    pub image: String,
    pub startup_script: Option<String>,
    pub status: String,
    pub memory_limit: Option<i64>,
    pub cpu_limit: Option<rust_decimal::Decimal>,
    pub disk_limit: Option<i64>,
    pub swap_limit: Option<i64>,
    pub io_weight: Option<i32>,
    pub sftp_user: Option<String>,
    pub sftp_pass: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
    /// Primary allocation IP (from container_allocations JOIN allocations)
    pub allocation_ip: Option<String>,
    /// Primary allocation port
    pub allocation_port: Option<i32>,
}

pub async fn list_containers(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> AppResult<Json<Vec<ContainerWithAllocation>>> {
    let containers: Vec<ContainerWithAllocation> = if claims.has_permission("containers.view_all") || claims.is_manager() {
        sqlx::query_as(
            r#"SELECT c.*, ca.ip as allocation_ip, ca.port as allocation_port
               FROM containers c
               LEFT JOIN container_allocations ca ON ca.container_id = c.id AND ca.is_primary = TRUE
               ORDER BY c.created_at DESC"#
        )
            .fetch_all(&state.db)
            .await?
    } else {
        sqlx::query_as(
            r#"SELECT c.*, ca.ip as allocation_ip, ca.port as allocation_port
               FROM containers c
               LEFT JOIN container_allocations ca ON ca.container_id = c.id AND ca.is_primary = TRUE
               WHERE c.user_id = $1
               ORDER BY c.created_at DESC"#
        )
            .bind(claims.sub)
            .fetch_all(&state.db)
            .await?
    };
    Ok(Json(containers))
}

pub async fn list_all_containers(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<ContainerWithAllocation>>> {
    let containers: Vec<ContainerWithAllocation> =
        sqlx::query_as(
            r#"SELECT c.*, ca.ip as allocation_ip, ca.port as allocation_port
               FROM containers c
               LEFT JOIN container_allocations ca ON ca.container_id = c.id AND ca.is_primary = TRUE
               ORDER BY c.created_at DESC"#
        )
            .fetch_all(&state.db)
            .await?;
    Ok(Json(containers))
}

pub async fn create_container(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateContainerRequest>,
) -> AppResult<Json<Container>> {
    if !claims.has_permission("containers.create") && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(req.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Generate container UUID first - this will be used as the Docker container name
    let container_id = Uuid::new_v4();
    let container_name_for_docker = container_id.to_string();

    let client = reqwest::Client::new();
    let daemon_url = format!("http://{}:{}/containers", daemon.host, daemon.port);

    let port_mappings: Vec<serde_json::Value> = req.ports.iter().map(|p| {
        serde_json::json!({
            "hostPort": p.host_port,
            "containerPort": p.container_port,
            "protocol": p.protocol
        })
    }).collect();

    // Build allocations array for daemon
    let mut allocations_for_daemon: Vec<serde_json::Value> = Vec::new();

    // Add primary allocation if provided
    if let Some(allocation_id) = req.allocation_id {
        let allocation: crate::models::Allocation = sqlx::query_as(
            "SELECT * FROM allocations WHERE id = $1 AND daemon_id = $2"
        )
            .bind(allocation_id)
            .bind(req.daemon_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::BadRequest("Primary allocation not found or belongs to different daemon".into()))?;

        allocations_for_daemon.push(serde_json::json!({
            "id": Uuid::new_v4().to_string(),
            "allocationId": allocation_id.to_string(),
            "ip": allocation.ip,
            "port": allocation.port,
            "internalPort": allocation.port,
            "protocol": "tcp",
            "isPrimary": true
        }));
    }

    // Add additional allocations
    for additional_allocation_id in &req.additional_allocations {
        let allocation: crate::models::Allocation = sqlx::query_as(
            "SELECT * FROM allocations WHERE id = $1 AND daemon_id = $2"
        )
            .bind(additional_allocation_id)
            .bind(req.daemon_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::BadRequest("Additional allocation not found or belongs to different daemon".into()))?;

        allocations_for_daemon.push(serde_json::json!({
            "id": Uuid::new_v4().to_string(),
            "allocationId": additional_allocation_id.to_string(),
            "ip": allocation.ip,
            "port": allocation.port,
            "internalPort": allocation.port,
            "protocol": "tcp",
            "isPrimary": false
        }));
    }

    // Use container UUID as the Docker container name
    let daemon_req = serde_json::json!({
        "name": container_name_for_docker,
        "image": req.image,
        "startupScript": req.startup_script,
        "memoryLimit": req.memory_limit,
        "cpuLimit": req.cpu_limit,
        "diskLimit": req.disk_limit,
        "swapLimit": req.swap_limit,
        "ioWeight": req.io_weight,
        "ports": port_mappings,
        "allocations": allocations_for_daemon
    });

    let res = client
        .post(&daemon_url)
        .header("X-API-Key", &daemon.api_key)
        .json(&daemon_req)
        .send()
        .await
        .map_err(|e| AppError::Daemon(e.to_string()))?;

    if !res.status().is_success() {
        let error_text = res.text().await.unwrap_or_default();
        return Err(AppError::Daemon(format!("Failed to create container: {}", error_text)));
    }

    let daemon_container: serde_json::Value = res
        .json()
        .await
        .map_err(|e| AppError::Daemon(e.to_string()))?;

    let now = Utc::now();

    // Allow admins/managers to assign container to a different user
    let user_id = if claims.is_manager() {
        req.user_id.unwrap_or(claims.sub)
    } else {
        claims.sub
    };

    // Generate FTP username using first 8 chars of container UUID (e.g., c3e12c86)
    // Password is not set initially - user must set it before using FTP
    let sftp_user = container_id.to_string().replace("-", "")[..8].to_string();

    // Default stop command for Minecraft servers
    let stop_command = req.stop_command.clone().unwrap_or_else(|| "stop".to_string());

    let container: Container = sqlx::query_as(
        r#"
        INSERT INTO containers (id, user_id, daemon_id, name, image, startup_script, stop_command, status, memory_limit, cpu_limit, disk_limit, swap_limit, io_weight, sftp_user, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'stopped', $8, $9, $10, $11, $12, $13, $14, $14)
        RETURNING *
        "#,
    )
    .bind(container_id)
    .bind(user_id)
    .bind(req.daemon_id)
    .bind(&req.name)  // Store user-friendly name in DB
    .bind(&req.image)
    .bind(&req.startup_script)
    .bind(&stop_command)
    .bind(req.memory_limit)
    .bind(rust_decimal::Decimal::try_from(req.cpu_limit).unwrap_or_default())
    .bind(req.disk_limit)
    .bind(req.swap_limit)
    .bind(req.io_weight)
    .bind(&sftp_user)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    // Create primary allocation if provided (allocation already validated above when building daemon request)
    if let Some(allocation_id) = req.allocation_id {
        // Get allocation data from the allocations_for_daemon we already built
        if let Some(alloc_json) = allocations_for_daemon.iter().find(|a| a["isPrimary"].as_bool() == Some(true)) {
            let ip = alloc_json["ip"].as_str().unwrap_or("");
            let port = alloc_json["port"].as_i64().unwrap_or(0) as i32;

            // Create container_allocation entry
            sqlx::query(
                r#"INSERT INTO container_allocations (id, container_id, allocation_id, ip, port, internal_port, protocol, is_primary, created_at)
                   VALUES ($1, $2, $3, $4, $5, $5, 'tcp', TRUE, NOW())"#
            )
                .bind(Uuid::new_v4())
                .bind(container_id)
                .bind(allocation_id)
                .bind(ip)
                .bind(port)
                .execute(&state.db)
                .await?;
        }
    }

    // Create additional allocations if provided (already validated above)
    for additional_allocation_id in &req.additional_allocations {
        // Find matching allocation from allocations_for_daemon
        if let Some(alloc_json) = allocations_for_daemon.iter().find(|a| {
            a["allocationId"].as_str() == Some(&additional_allocation_id.to_string())
        }) {
            let ip = alloc_json["ip"].as_str().unwrap_or("");
            let port = alloc_json["port"].as_i64().unwrap_or(0) as i32;

            // Create container_allocation entry (not primary)
            sqlx::query(
                r#"INSERT INTO container_allocations (id, container_id, allocation_id, ip, port, internal_port, protocol, is_primary, created_at)
                   VALUES ($1, $2, $3, $4, $5, $5, 'tcp', FALSE, NOW())"#
            )
                .bind(Uuid::new_v4())
                .bind(container_id)
                .bind(additional_allocation_id)
                .bind(ip)
                .bind(port)
                .execute(&state.db)
                .await?;
        }
    }

    for port in &req.ports {
        sqlx::query(
            "INSERT INTO container_ports (id, container_id, host_port, container_port, protocol) VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(Uuid::new_v4())
        .bind(container_id)
        .bind(port.host_port)
        .bind(port.container_port)
        .bind(&port.protocol)
        .execute(&state.db)
        .await?;
    }

    tracing::info!("Created container on daemon: {:?}", daemon_container);

    Ok(Json(container))
}

pub async fn get_container(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ContainerResponse>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if container.user_id != claims.sub && !claims.has_permission("containers.view_all") && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    // Get all allocations for this container (ip is now stored directly in container_allocations)
    let allocations: Vec<AllocationInfo> = sqlx::query_as(
        r#"SELECT ca.id, ca.allocation_id, ca.ip, ca.port, ca.internal_port, ca.protocol, COALESCE(ca.is_primary, FALSE) as is_primary
           FROM container_allocations ca
           WHERE ca.container_id = $1
           ORDER BY ca.is_primary DESC, ca.ip, ca.port"#
    )
        .bind(id)
        .fetch_all(&state.db)
        .await?;

    // Get primary allocation for convenience fields
    let primary = allocations.iter().find(|a| a.is_primary);
    let allocation_ip = primary.map(|a| a.ip.clone());
    let allocation_port = primary.map(|a| a.port);

    Ok(Json(ContainerResponse {
        container,
        allocations,
        allocation_ip,
        allocation_port,
    }))
}

pub async fn get_container_ports(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<ContainerPort>>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if container.user_id != claims.sub && !claims.has_permission("containers.view_all") && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    let ports: Vec<ContainerPort> = sqlx::query_as("SELECT * FROM container_ports WHERE container_id = $1")
        .bind(id)
        .fetch_all(&state.db)
        .await?;

    Ok(Json(ports))
}

pub async fn delete_container(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<()>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check permission - only admin/manager or container owner can delete
    if container.user_id != claims.sub && !claims.has_permission("containers.delete") && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = reqwest::Client::new();
    let daemon_url = format!("http://{}:{}/containers/{}", daemon.host, daemon.port, container.id);

    client
        .delete(&daemon_url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::Daemon(e.to_string()))?;

    sqlx::query("DELETE FROM containers WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(()))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateContainerRequest {
    #[serde(default)]
    pub memory_limit: Option<i64>,
    #[serde(default)]
    pub cpu_limit: Option<f64>,
    #[serde(default)]
    pub disk_limit: Option<i64>,
    #[serde(default)]
    pub swap_limit: Option<i64>,
    #[serde(default)]
    pub io_weight: Option<i32>,
    #[serde(default)]
    pub allocation_id: Option<Uuid>,
}

pub async fn update_container(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateContainerRequest>,
) -> AppResult<Json<Container>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check basic permission - owner or manager
    let is_owner = container.user_id == claims.sub;
    let is_manager = claims.has_permission("containers.manage") || claims.is_manager();

    if !is_owner && !is_manager {
        return Err(AppError::Unauthorized);
    }

    // Check if user is trying to change resource limits
    let changing_resources = req.memory_limit.is_some()
        || req.cpu_limit.is_some()
        || req.disk_limit.is_some()
        || req.swap_limit.is_some()
        || req.io_weight.is_some();

    // Resource limit changes require special permission (unless manager/admin)
    if changing_resources && !is_manager && !claims.has_permission("containers.edit_resources") {
        return Err(AppError::Forbidden("You don't have permission to change resource limits".into()));
    }

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Build update payload for daemon
    let mut daemon_payload = serde_json::json!({});

    if let Some(memory) = req.memory_limit {
        daemon_payload["memoryLimit"] = serde_json::json!(memory);
    }
    if let Some(cpu) = req.cpu_limit {
        daemon_payload["cpuLimit"] = serde_json::json!(cpu);
    }
    if let Some(disk) = req.disk_limit {
        daemon_payload["diskLimit"] = serde_json::json!(disk);
    }
    if let Some(swap) = req.swap_limit {
        daemon_payload["swapLimit"] = serde_json::json!(swap);
    }
    if let Some(io) = req.io_weight {
        daemon_payload["ioWeight"] = serde_json::json!(io);
    }

    // Update daemon container resources
    let client = reqwest::Client::new();
    let daemon_url = format!("http://{}:{}/containers/{}", daemon.host, daemon.port, container.id);

    let res = client
        .patch(&daemon_url)
        .header("X-API-Key", &daemon.api_key)
        .json(&daemon_payload)
        .send()
        .await
        .map_err(|e| AppError::Daemon(e.to_string()))?;

    if !res.status().is_success() {
        let error_text = res.text().await.unwrap_or_default();
        tracing::warn!("Daemon update warning: {}", error_text);
        // Don't fail - continue to update DB
    }

    // Update database - handle Option fields properly
    let memory_limit = req.memory_limit.or(container.memory_limit);
    let cpu_limit = req.cpu_limit.map(|c| rust_decimal::Decimal::try_from(c).ok()).flatten().or(container.cpu_limit);
    let disk_limit = req.disk_limit.or(container.disk_limit);
    let swap_limit = req.swap_limit.or(container.swap_limit);
    let io_weight = req.io_weight.or(container.io_weight);

    let updated_container: Container = sqlx::query_as(
        r#"UPDATE containers SET
            memory_limit = $1,
            cpu_limit = $2,
            disk_limit = $3,
            swap_limit = $4,
            io_weight = $5,
            updated_at = NOW()
        WHERE id = $6
        RETURNING *"#
    )
    .bind(memory_limit)
    .bind(cpu_limit)
    .bind(disk_limit)
    .bind(swap_limit)
    .bind(io_weight)
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    // Handle allocation update separately via container_allocations if provided
    if let Some(allocation_id) = req.allocation_id {
        // Remove existing primary and set new one
        sqlx::query("DELETE FROM container_allocations WHERE container_id = $1 AND is_primary = TRUE")
            .bind(id)
            .execute(&state.db)
            .await?;

        let allocation: crate::models::Allocation = sqlx::query_as(
            "SELECT * FROM allocations WHERE id = $1"
        )
            .bind(allocation_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::BadRequest("Allocation not found".into()))?;

        sqlx::query(
            r#"INSERT INTO container_allocations (id, container_id, allocation_id, ip, port, internal_port, protocol, is_primary, created_at)
               VALUES ($1, $2, $3, $4, $5, $5, 'tcp', TRUE, NOW())"#
        )
            .bind(Uuid::new_v4())
            .bind(id)
            .bind(allocation_id)
            .bind(&allocation.ip)
            .bind(allocation.port)
            .execute(&state.db)
            .await?;
    }

    Ok(Json(updated_container))
}

async fn proxy_container_action(
    state: &AppState,
    claims: &Claims,
    id: Uuid,
    action: &str,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check if user has permission to manage this container
    let can_manage = container.user_id == claims.sub
        || claims.has_permission("containers.manage")
        || claims.has_permission("containers.manage_own") && container.user_id == claims.sub
        || claims.is_manager();

    if !can_manage {
        return Err(AppError::Unauthorized);
    }

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = reqwest::Client::new();
    let daemon_url = format!(
        "http://{}:{}/containers/{}/{}",
        daemon.host, daemon.port, container.id, action
    );

    let res = client
        .post(&daemon_url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::Daemon(e.to_string()))?;

    // Determine the correct status based on action
    let status = match action {
        "start" | "restart" => "running",
        "stop" | "kill" => "stopped",
        _ => "unknown"
    };
    sqlx::query("UPDATE containers SET status = $1, updated_at = NOW() WHERE id = $2")
        .bind(status)
        .bind(id)
        .execute(&state.db)
        .await?;

    let body: serde_json::Value = res.json().await.unwrap_or(serde_json::json!({"success": true}));
    Ok(Json(body))
}

pub async fn start_container(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check permission
    let can_manage = container.user_id == claims.sub
        || claims.has_permission("containers.manage")
        || claims.has_permission("containers.manage_own") && container.user_id == claims.sub
        || claims.is_manager();

    if !can_manage {
        return Err(AppError::Unauthorized);
    }

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Get all container allocations from DB
    let allocations: Vec<crate::models::ContainerAllocation> = sqlx::query_as(
        r#"SELECT ca.id, ca.allocation_id, ca.ip, ca.port, ca.internal_port, ca.protocol, COALESCE(ca.is_primary, FALSE) as is_primary
           FROM container_allocations ca
           WHERE ca.container_id = $1
           ORDER BY ca.is_primary DESC, ca.ip, ca.port"#
    )
        .bind(id)
        .fetch_all(&state.db)
        .await?;

    let client = reqwest::Client::new();

    // First, update the daemon with current allocations
    if !allocations.is_empty() {
        let allocations_json: Vec<serde_json::Value> = allocations.iter().map(|a| {
            serde_json::json!({
                "id": a.id.to_string(),
                "allocationId": a.allocation_id.map(|id| id.to_string()),
                "ip": a.ip,
                "port": a.port,
                "internalPort": a.internal_port,
                "protocol": a.protocol,
                "isPrimary": a.is_primary
            })
        }).collect();

        let update_url = format!("http://{}:{}/containers/{}", daemon.host, daemon.port, container.id);
        let update_res = client
            .patch(&update_url)
            .header("X-API-Key", &daemon.api_key)
            .json(&serde_json::json!({
                "allocations": allocations_json
            }))
            .send()
            .await;

        if let Err(e) = update_res {
            tracing::warn!("Failed to sync allocations to daemon: {}", e);
        }
    }

    // Now start the container
    let start_url = format!("http://{}:{}/containers/{}/start", daemon.host, daemon.port, container.id);
    let start_res = client
        .post(&start_url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::Daemon(e.to_string()))?;

    if !start_res.status().is_success() {
        let error_text = start_res.text().await.unwrap_or_default();
        return Err(AppError::Daemon(format!("Failed to start container: {}", error_text)));
    }

    // Update status
    sqlx::query("UPDATE containers SET status = 'running', updated_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    let body: serde_json::Value = serde_json::json!({ "success": true });
    Ok(Json(body))
}

pub async fn stop_container(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check if user has permission to manage this container
    let can_manage = container.user_id == claims.sub
        || claims.has_permission("containers.manage")
        || claims.has_permission("containers.manage_own") && container.user_id == claims.sub
        || claims.is_manager();

    if !can_manage {
        return Err(AppError::Unauthorized);
    }

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = reqwest::Client::new();

    // Use the container's configured stop command (defaults to "stop")
    let stop_command = container.stop_command.unwrap_or_else(|| "stop".to_string());

    // First, attempt graceful stop by sending the stop command to stdin
    let graceful_url = format!("http://{}:{}/containers/{}/graceful-stop", daemon.host, daemon.port, container.id);
    let graceful_res = client
        .post(&graceful_url)
        .header("X-API-Key", &daemon.api_key)
        .json(&serde_json::json!({
            "stopCommand": stop_command,
            "timeoutSecs": 30
        }))
        .send()
        .await;

    match graceful_res {
        Ok(res) if res.status().is_success() => {
            // Update container status
            sqlx::query("UPDATE containers SET status = 'stopped', updated_at = NOW() WHERE id = $1")
                .bind(id)
                .execute(&state.db)
                .await?;

            Ok(Json(serde_json::json!({ "success": true, "method": "graceful" })))
        }
        _ => {
            // Graceful stop failed or timed out, fallback to Docker stop
            tracing::warn!("Graceful stop failed for container {}, using Docker stop", id);

            let docker_stop_url = format!("http://{}:{}/containers/{}/stop", daemon.host, daemon.port, container.id);
            let docker_res = client
                .post(&docker_stop_url)
                .header("X-API-Key", &daemon.api_key)
                .send()
                .await
                .map_err(|e| AppError::Daemon(e.to_string()))?;

            if !docker_res.status().is_success() {
                let error_text = docker_res.text().await.unwrap_or_default();
                return Err(AppError::Daemon(format!("Failed to stop container: {}", error_text)));
            }

            // Update container status
            sqlx::query("UPDATE containers SET status = 'stopped', updated_at = NOW() WHERE id = $1")
                .bind(id)
                .execute(&state.db)
                .await?;

            Ok(Json(serde_json::json!({ "success": true, "method": "force" })))
        }
    }
}

pub async fn restart_container(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check if user has permission to manage this container
    let can_manage = container.user_id == claims.sub
        || claims.has_permission("containers.manage")
        || claims.is_manager();

    if !can_manage {
        return Err(AppError::Unauthorized);
    }

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = reqwest::Client::new();

    // Use the container's configured stop command
    let stop_command = container.stop_command.clone().unwrap_or_else(|| "stop".to_string());

    // First, do graceful stop
    let stop_url = format!("http://{}:{}/containers/{}/graceful-stop", daemon.host, daemon.port, container.id);
    let stop_res = client
        .post(&stop_url)
        .header("X-API-Key", &daemon.api_key)
        .json(&serde_json::json!({
            "stopCommand": stop_command,
            "timeoutSecs": 15
        }))
        .send()
        .await
        .map_err(|e| AppError::Daemon(e.to_string()))?;

    if !stop_res.status().is_success() {
        tracing::warn!("Graceful stop failed, trying force stop");
        // Fallback to force stop
        let force_stop_url = format!("http://{}:{}/containers/{}/stop", daemon.host, daemon.port, container.id);
        client
            .post(&force_stop_url)
            .header("X-API-Key", &daemon.api_key)
            .send()
            .await
            .map_err(|e| AppError::Daemon(e.to_string()))?;
    }

    // Wait a moment for container to fully stop
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Then start
    let start_url = format!("http://{}:{}/containers/{}/start", daemon.host, daemon.port, container.id);
    let start_res = client
        .post(&start_url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::Daemon(e.to_string()))?;

    if !start_res.status().is_success() {
        let error_text = start_res.text().await.unwrap_or_default();
        return Err(AppError::Daemon(format!("Failed to start after stop: {}", error_text)));
    }

    // Update status to running
    sqlx::query("UPDATE containers SET status = 'running', updated_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn kill_container(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    proxy_container_action(&state, &claims, id, "kill").await
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendCommandRequest {
    pub command: String,
}

pub async fn send_command(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(req): Json<SendCommandRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check if user has permission to manage this container
    let can_manage = container.user_id == claims.sub
        || claims.has_permission("containers.manage")
        || claims.is_manager();

    if !can_manage {
        return Err(AppError::Unauthorized);
    }

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = reqwest::Client::new();
    let url = format!("http://{}:{}/containers/{}/command", daemon.host, daemon.port, container.id);

    let res = client
        .post(&url)
        .header("X-API-Key", &daemon.api_key)
        .json(&serde_json::json!({ "command": req.command }))
        .send()
        .await
        .map_err(|e| AppError::Daemon(e.to_string()))?;

    if !res.status().is_success() {
        let error_text = res.text().await.unwrap_or_default();
        return Err(AppError::Daemon(format!("Failed to send command: {}", error_text)));
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GracefulStopRequest {
    #[serde(default)]
    pub stop_command: Option<String>,
    #[serde(default = "default_stop_timeout")]
    pub timeout_secs: u64,
}

fn default_stop_timeout() -> u64 {
    10
}

pub async fn graceful_stop_container(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(req): Json<GracefulStopRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check if user has permission to manage this container
    let can_manage = container.user_id == claims.sub
        || claims.has_permission("containers.manage")
        || claims.is_manager();

    if !can_manage {
        return Err(AppError::Unauthorized);
    }

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = reqwest::Client::new();
    let url = format!("http://{}:{}/containers/{}/graceful-stop", daemon.host, daemon.port, container.id);

    let res = client
        .post(&url)
        .header("X-API-Key", &daemon.api_key)
        .json(&serde_json::json!({
            "stopCommand": req.stop_command,
            "timeoutSecs": req.timeout_secs
        }))
        .send()
        .await
        .map_err(|e| AppError::Daemon(e.to_string()))?;

    if !res.status().is_success() {
        let error_text = res.text().await.unwrap_or_default();
        return Err(AppError::Daemon(format!("Failed to stop: {}", error_text)));
    }

    // Update container status in database
    sqlx::query("UPDATE containers SET status = 'stopped', updated_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignAllocationRequest {
    pub allocation_id: Uuid,
}

/// Assign a primary allocation to a container (replaces any existing primary)
pub async fn assign_allocation(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(req): Json<AssignAllocationRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check permission
    if container.user_id != claims.sub && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    // Check allocation exists and belongs to the same daemon
    let allocation: crate::models::Allocation = sqlx::query_as(
        "SELECT * FROM allocations WHERE id = $1 AND daemon_id = $2"
    )
        .bind(req.allocation_id)
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::BadRequest("Allocation not found or belongs to different daemon".into()))?;

    // Check if allocation is already assigned to another container
    let existing_other: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM container_allocations WHERE allocation_id = $1 AND container_id != $2"
    )
        .bind(req.allocation_id)
        .bind(container.id)
        .fetch_optional(&state.db)
        .await?;

    if existing_other.is_some() {
        return Err(AppError::BadRequest("Allocation is already in use by another container".into()));
    }

    // Check if this allocation is already assigned to this container
    let existing_for_container: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT id, COALESCE(is_primary, FALSE) FROM container_allocations WHERE allocation_id = $1 AND container_id = $2"
    )
        .bind(req.allocation_id)
        .bind(container.id)
        .fetch_optional(&state.db)
        .await?;

    if let Some((existing_id, is_already_primary)) = existing_for_container {
        if is_already_primary {
            // Already primary, nothing to do
            return Ok(Json(serde_json::json!({
                "message": "Allocation is already primary",
                "allocationIp": allocation.ip,
                "allocationPort": allocation.port
            })));
        }

        // Unset current primary
        sqlx::query("UPDATE container_allocations SET is_primary = FALSE WHERE container_id = $1 AND is_primary = TRUE")
            .bind(container.id)
            .execute(&state.db)
            .await?;

        // Set this one as primary
        sqlx::query("UPDATE container_allocations SET is_primary = TRUE WHERE id = $1")
            .bind(existing_id)
            .execute(&state.db)
            .await?;
    } else {
        // Allocation not yet assigned to this container - remove existing primary and create new
        sqlx::query("UPDATE container_allocations SET is_primary = FALSE WHERE container_id = $1 AND is_primary = TRUE")
            .bind(container.id)
            .execute(&state.db)
            .await?;

        // Create new primary allocation
        sqlx::query(
            r#"INSERT INTO container_allocations (id, container_id, allocation_id, ip, port, internal_port, protocol, is_primary, created_at)
               VALUES ($1, $2, $3, $4, $5, $5, 'tcp', TRUE, NOW())"#
        )
            .bind(Uuid::new_v4())
            .bind(container.id)
            .bind(req.allocation_id)
            .bind(&allocation.ip)
            .bind(allocation.port)
            .execute(&state.db)
            .await?;
    }

    Ok(Json(serde_json::json!({
        "message": "Allocation assigned successfully",
        "allocationIp": allocation.ip,
        "allocationPort": allocation.port
    })))
}

/// Get available allocations for a container's daemon (not assigned to any container)
pub async fn get_available_allocations(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<crate::models::Allocation>>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check permission
    if container.user_id != claims.sub && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    // Get available allocations (not assigned to any container via container_allocations) for this daemon
    let allocations: Vec<crate::models::Allocation> = sqlx::query_as(
        r#"SELECT a.* FROM allocations a
           WHERE a.daemon_id = $1
           AND NOT EXISTS (SELECT 1 FROM container_allocations ca WHERE ca.allocation_id = a.id)
           ORDER BY a.ip, a.port"#
    )
        .bind(container.daemon_id)
        .fetch_all(&state.db)
        .await?;

    Ok(Json(allocations))
}

/// Get all allocations assigned to a container
pub async fn get_container_allocations(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<AllocationInfo>>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check permission
    if container.user_id != claims.sub && !claims.has_permission("containers.view_all") && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    // Get all allocations assigned to this container (ip stored directly in container_allocations)
    let allocations: Vec<AllocationInfo> = sqlx::query_as(
        r#"SELECT ca.id, ca.allocation_id, ca.ip, ca.port, ca.internal_port, ca.protocol, COALESCE(ca.is_primary, FALSE) as is_primary
           FROM container_allocations ca
           WHERE ca.container_id = $1
           ORDER BY ca.is_primary DESC, ca.ip, ca.port"#
    )
        .bind(id)
        .fetch_all(&state.db)
        .await?;

    Ok(Json(allocations))
}

/// Add an additional allocation to a container
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAllocationRequest {
    pub allocation_id: Uuid,
    #[serde(default = "default_tcp")]
    pub protocol: String,
    #[serde(default)]
    pub is_primary: bool,
}

fn default_tcp() -> String {
    "tcp".to_string()
}

pub async fn add_allocation(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(req): Json<AddAllocationRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check permission
    if container.user_id != claims.sub && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    // Check allocation exists and belongs to the same daemon
    let allocation: crate::models::Allocation = sqlx::query_as(
        "SELECT * FROM allocations WHERE id = $1 AND daemon_id = $2"
    )
        .bind(req.allocation_id)
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::BadRequest("Allocation not found or belongs to different daemon".into()))?;

    // Check if allocation is already assigned to any container
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM container_allocations WHERE allocation_id = $1"
    )
        .bind(req.allocation_id)
        .fetch_optional(&state.db)
        .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Allocation is already in use".into()));
    }

    // If setting as primary, remove existing primary
    if req.is_primary {
        sqlx::query("UPDATE container_allocations SET is_primary = FALSE WHERE container_id = $1 AND is_primary = TRUE")
            .bind(container.id)
            .execute(&state.db)
            .await?;
    }

    // Check if this will be the first allocation (auto-set as primary)
    let has_allocations: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM container_allocations WHERE container_id = $1"
    )
        .bind(container.id)
        .fetch_one(&state.db)
        .await
        .ok();

    let is_primary = req.is_primary || has_allocations.map(|c| c.0 == 0).unwrap_or(true);

    // Create container_allocation entry
    sqlx::query(
        r#"INSERT INTO container_allocations (id, container_id, allocation_id, ip, port, internal_port, protocol, is_primary, created_at)
           VALUES ($1, $2, $3, $4, $5, $5, $6, $7, NOW())"#
    )
        .bind(Uuid::new_v4())
        .bind(container.id)
        .bind(req.allocation_id)
        .bind(&allocation.ip)
        .bind(allocation.port)
        .bind(&req.protocol)
        .bind(is_primary)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({
        "message": "Allocation added successfully",
        "allocationIp": allocation.ip,
        "allocationPort": allocation.port,
        "isPrimary": is_primary
    })))
}

/// Remove an allocation from a container
pub async fn remove_allocation(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((id, allocation_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check permission
    if container.user_id != claims.sub && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    // Check container_allocation exists for this container
    let container_allocation: (Uuid, bool) = sqlx::query_as(
        "SELECT id, COALESCE(is_primary, FALSE) FROM container_allocations WHERE allocation_id = $1 AND container_id = $2"
    )
        .bind(allocation_id)
        .bind(container.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::BadRequest("Allocation not found or doesn't belong to this container".into()))?;

    let was_primary = container_allocation.1;

    // Remove container_allocation
    sqlx::query("DELETE FROM container_allocations WHERE id = $1")
        .bind(container_allocation.0)
        .execute(&state.db)
        .await?;

    // If this was the primary allocation, promote another allocation to primary
    if was_primary {
        sqlx::query(
            r#"UPDATE container_allocations
               SET is_primary = TRUE
               WHERE id = (
                   SELECT id FROM container_allocations
                   WHERE container_id = $1
                   ORDER BY created_at ASC
                   LIMIT 1
               )"#
        )
            .bind(container.id)
            .execute(&state.db)
            .await?;
    }

    Ok(Json(serde_json::json!({
        "message": "Allocation removed successfully"
    })))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSftpPasswordRequest {
    pub password: String,
}

pub async fn set_sftp_password(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(req): Json<SetSftpPasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if container.user_id != claims.sub && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    if req.password.len() < 8 {
        return Err(AppError::BadRequest("Password must be at least 8 characters".into()));
    }

    // Register FTP user with daemon
    let daemon: crate::models::Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = reqwest::Client::new();
    let url = format!("http://{}:{}/containers/{}/ftp", daemon.host, daemon.port, container.id);

    let daemon_result = client
        .post(&url)
        .header("X-API-Key", &daemon.api_key)
        .json(&serde_json::json!({ "password": req.password }))
        .send()
        .await;

    if let Err(e) = daemon_result {
        tracing::warn!("Failed to register FTP user with daemon: {}", e);
    }

    let password_hash = bcrypt::hash(&req.password, state.config.bcrypt_cost)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // SFTP username is first 8 chars of container ID
    let sftp_user = container.id.to_string().replace("-", "")[..8].to_string();

    sqlx::query("UPDATE containers SET sftp_user = $1, sftp_pass = $2, updated_at = NOW() WHERE id = $3")
        .bind(&sftp_user)
        .bind(&password_hash)
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({
        "message": "SFTP password set successfully",
        "sftpUser": sftp_user
    })))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddContainerUserRequest {
    pub user_id: Uuid,
    pub permission_level: Option<String>,
}

pub async fn list_container_users(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<crate::models::ContainerUserResponse>>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if !can_access_container(&claims, &container) {
        return Err(AppError::Unauthorized);
    }

    let users: Vec<crate::models::ContainerUserResponse> = sqlx::query_as(
        r#"
        SELECT cu.id, cu.container_id, cu.user_id, u.username, cu.permission_level, cu.created_at
        FROM container_users cu
        JOIN users u ON u.id = cu.user_id
        WHERE cu.container_id = $1
        ORDER BY cu.created_at
        "#
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(users))
}

pub async fn add_container_user(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(req): Json<AddContainerUserRequest>,
) -> AppResult<Json<crate::models::ContainerUser>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if container.user_id != claims.sub && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    let _user: crate::models::User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(req.user_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::BadRequest("User not found".into()))?;

    let permission_level = req.permission_level.unwrap_or_else(|| "user".to_string());

    let container_user: crate::models::ContainerUser = sqlx::query_as(
        r#"
        INSERT INTO container_users (id, container_id, user_id, permission_level, created_at)
        VALUES ($1, $2, $3, $4, NOW())
        ON CONFLICT (container_id, user_id) DO UPDATE SET permission_level = $4
        RETURNING *
        "#
    )
    .bind(Uuid::new_v4())
    .bind(id)
    .bind(req.user_id)
    .bind(&permission_level)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(container_user))
}

pub async fn remove_container_user(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((container_id, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(container_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if container.user_id != claims.sub && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    if container.user_id == user_id {
        return Err(AppError::BadRequest("Cannot remove the container owner".into()));
    }

    sqlx::query("DELETE FROM container_users WHERE container_id = $1 AND user_id = $2")
        .bind(container_id)
        .bind(user_id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "message": "User removed from container" })))
}

fn can_access_container(claims: &Claims, container: &Container) -> bool {
    container.user_id == claims.sub
        || claims.has_permission("containers.view_all")
        || claims.is_manager()
}

#[derive(Debug, serde::Deserialize)]
pub struct ListFilesQuery {
    pub path: Option<String>,
}

pub async fn list_files(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Query(query): Query<ListFilesQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if !can_access_container(&claims, &container) {
        return Err(AppError::Unauthorized);
    }

    let daemon: crate::models::Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = reqwest::Client::new();
    let path = query.path.unwrap_or_else(|| "/".to_string());
    // Use container ID (UUID) for volume path
    let url = format!("http://{}:{}/containers/{}/files?path={}", daemon.host, daemon.port, container.id, path);

    let resp = client
        .get(&url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Daemon error: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(AppError::BadRequest(format!("Daemon returned {}: {}", status, error_text)));
    }

    let files: serde_json::Value = resp.json().await
        .map_err(|e| AppError::BadRequest(format!("Parse error: {}", e)))?;

    Ok(Json(files))
}

#[derive(Debug, serde::Deserialize)]
pub struct ReadFileQuery {
    pub path: String,
}

pub async fn read_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Query(query): Query<ReadFileQuery>,
) -> AppResult<String> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if !can_access_container(&claims, &container) {
        return Err(AppError::Unauthorized);
    }

    let daemon: crate::models::Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = reqwest::Client::new();
    // Use container ID (UUID) for volume path
    let url = format!("http://{}:{}/containers/{}/files/read?path={}", daemon.host, daemon.port, container.id, query.path);

    let resp = client
        .get(&url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Daemon error: {}", e)))?;

    let content = resp.text().await
        .map_err(|e| AppError::BadRequest(format!("Read error: {}", e)))?;

    Ok(content)
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileRequest {
    pub path: String,
    pub content: String,
}

pub async fn write_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(req): Json<WriteFileRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if !can_access_container(&claims, &container) {
        return Err(AppError::Unauthorized);
    }

    let daemon: crate::models::Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = reqwest::Client::new();
    // Use container ID (UUID) for volume path
    let url = format!("http://{}:{}/containers/{}/files/write", daemon.host, daemon.port, container.id);

    let resp = client
        .post(&url)
        .header("X-API-Key", &daemon.api_key)
        .json(&req)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Daemon error: {}", e)))?;

    let result: serde_json::Value = resp.json().await
        .map_err(|e| AppError::BadRequest(format!("Parse error: {}", e)))?;

    Ok(Json(result))
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFolderRequest {
    pub path: String,
}

pub async fn create_folder(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateFolderRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if !can_access_container(&claims, &container) {
        return Err(AppError::Unauthorized);
    }

    let daemon: crate::models::Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = reqwest::Client::new();
    // Use container ID (UUID) for volume path
    let url = format!("http://{}:{}/containers/{}/files/folder", daemon.host, daemon.port, container.id);

    let resp = client
        .post(&url)
        .header("X-API-Key", &daemon.api_key)
        .json(&req)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Daemon error: {}", e)))?;

    let result: serde_json::Value = resp.json().await
        .map_err(|e| AppError::BadRequest(format!("Parse error: {}", e)))?;

    Ok(Json(result))
}

#[derive(Debug, serde::Deserialize)]
pub struct DeleteFileQuery {
    pub path: String,
}

pub async fn delete_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Query(query): Query<DeleteFileQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if !can_access_container(&claims, &container) {
        return Err(AppError::Unauthorized);
    }

    let daemon: crate::models::Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = reqwest::Client::new();
    // Use container ID (UUID) for volume path
    let url = format!("http://{}:{}/containers/{}/files/delete?path={}", daemon.host, daemon.port, container.id, query.path);

    let resp = client
        .delete(&url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Daemon error: {}", e)))?;

    let result: serde_json::Value = resp.json().await
        .map_err(|e| AppError::BadRequest(format!("Parse error: {}", e)))?;

    Ok(Json(result))
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerStats {
    pub cpu_percent: f64,
    pub memory_usage: u64,
    pub memory_limit: u64,
    pub memory_percent: f64,
    pub network_rx: u64,
    pub network_tx: u64,
    pub block_read: u64,
    pub block_write: u64,
}

pub async fn get_container_stats(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ContainerStats>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if !can_access_container(&claims, &container) {
        return Err(AppError::Unauthorized);
    }

    let daemon: crate::models::Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = reqwest::Client::new();
    let url = format!("http://{}:{}/containers/{}/stats", daemon.host, daemon.port, container.id);

    let resp = client
        .get(&url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::Daemon(format!("Failed to fetch stats: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::Daemon("Failed to fetch container stats".into()));
    }

    let stats: ContainerStats = resp.json().await
        .map_err(|e| AppError::BadRequest(format!("Parse error: {}", e)))?;

    Ok(Json(stats))
}
