use axum::{
    extract::{Path, Query, State},
    Extension,
    Json,
};
use base64::{Engine as _, engine::general_purpose};
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;
use std::collections::HashMap;

use crate::error::{AppError, AppResult};
use crate::models::{AppState, Claims, Container, ContainerPort, CreateContainerRequest, Daemon};

pub fn daemon_client() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

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
    pub allocation_ip: Option<String>,
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
    pub allocation_ip: Option<String>,
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

    let (image, startup_script, flake_id, install_script, mut flake_variables, restart_policy, tty) = if let Some(fid) = req.flake_id {
        let flake: crate::handlers::flakes::Flake = sqlx::query_as("SELECT * FROM flakes WHERE id = $1")
            .bind(fid)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::BadRequest("Flake not found".into()))?;

        let vars: Vec<crate::handlers::flakes::FlakeVariable> = sqlx::query_as(
            "SELECT * FROM flake_variables WHERE flake_id = $1 ORDER BY sort_order"
        )
            .bind(fid)
            .fetch_all(&state.db)
            .await?;

        let mut env_vars: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for var in &vars {
            let value = req.variables.get(&var.env_variable)
                .cloned()
                .unwrap_or_else(|| var.default_value.clone().unwrap_or_default());
            env_vars.insert(var.env_variable.clone(), value);
        }

        let server_memory = req.server_memory.unwrap_or(req.memory_limit);
        env_vars.insert("SERVER_MEMORY".to_string(), server_memory.to_string());

        let startup = req.startup_script.clone()
            .unwrap_or_else(|| flake.startup_command.clone());

        (
            flake.docker_image,
            Some(startup),
            Some(fid),
            flake.install_script,
            env_vars,
            flake.restart_policy,
            flake.tty,
        )
    } else {

        let image = req.image.clone().ok_or(AppError::BadRequest("Either flake_id or image is required".into()))?;
        (image, req.startup_script.clone(), None, None, std::collections::HashMap::new(), "unless-stopped".to_string(), false)
    };

    let container_id = Uuid::new_v4();
    let container_name_for_docker = container_id.to_string();

    let client = daemon_client();
    let daemon_url = format!("{}/containers", daemon.base_url());

    let port_mappings: Vec<serde_json::Value> = req.ports.iter().map(|p| {
        serde_json::json!({
            "hostPort": p.host_port,
            "containerPort": p.container_port,
            "protocol": p.protocol
        })
    }).collect();

    let mut allocations_for_daemon: Vec<serde_json::Value> = Vec::new();
    let mut primary_port: Option<i32> = None;
    let mut primary_ip: Option<String> = None;

    if let Some(allocation_id) = req.allocation_id {
        let allocation: crate::models::Allocation = sqlx::query_as(
            "SELECT * FROM allocations WHERE id = $1 AND daemon_id = $2"
        )
            .bind(allocation_id)
            .bind(req.daemon_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::BadRequest("Primary allocation not found or belongs to different daemon".into()))?;

        primary_port = Some(allocation.port);
        primary_ip = Some(allocation.ip.clone());

        allocations_for_daemon.push(serde_json::json!({
            "id": Uuid::new_v4().to_string(),
            "allocationId": allocation_id.to_string(),
            "ip": allocation.ip,
            "port": allocation.port,
            "internalPort": allocation.port,
            "protocol": allocation.protocol,
            "isPrimary": true
        }));
    }

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
            "protocol": allocation.protocol,
            "isPrimary": false
        }));
    }

    if let Some(port) = primary_port {
        flake_variables.insert("SERVER_PORT".to_string(), port.to_string());
    }
    if let Some(ip) = primary_ip {
        flake_variables.insert("SERVER_IP".to_string(), ip);
    }

    let server_memory = req.server_memory.unwrap_or(req.memory_limit);

    let daemon_req = serde_json::json!({
        "name": container_name_for_docker,
        "image": image,
        "startupScript": startup_script,
        "memoryLimit": req.memory_limit,
        "serverMemory": server_memory,
        "cpuLimit": req.cpu_limit,
        "diskLimit": req.disk_limit,
        "swapLimit": req.swap_limit,
        "ioWeight": req.io_weight,
        "ports": port_mappings,
        "allocations": allocations_for_daemon,
        "installScript": install_script,
        "environment": flake_variables,
        "restartPolicy": restart_policy,
        "tty": tty
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

    let user_id = if claims.is_manager() {
        req.user_id.unwrap_or(claims.sub)
    } else {
        claims.sub
    };

    let sftp_user = container_id.to_string().replace("-", "")[..8].to_string();

    let stop_command = req.stop_command.clone().unwrap_or_else(|| "stop".to_string());

    let container: Container = sqlx::query_as(
        r#"
        INSERT INTO containers (id, user_id, daemon_id, flake_id, name, image, startup_script, stop_command, status, memory_limit, cpu_limit, disk_limit, swap_limit, io_weight, sftp_user, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'stopped', $9, $10, $11, $12, $13, $14, $15, $15)
        RETURNING *
        "#,
    )
    .bind(container_id)
    .bind(user_id)
    .bind(req.daemon_id)
    .bind(flake_id)
    .bind(&req.name)
    .bind(&image)
    .bind(&startup_script)
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

    if let Some(allocation_id) = req.allocation_id {

        if let Some(alloc_json) = allocations_for_daemon.iter().find(|a| a["isPrimary"].as_bool() == Some(true)) {
            let ip = alloc_json["ip"].as_str().unwrap_or("");
            let port = alloc_json["port"].as_i64().unwrap_or(0) as i32;

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

    for additional_allocation_id in &req.additional_allocations {

        let alloc_id_str = additional_allocation_id.to_string();
        if let Some(alloc_json) = allocations_for_daemon.iter().find(|a| {
            a["allocationId"].as_str() == Some(alloc_id_str.as_str())
        }) {
            let ip = alloc_json["ip"].as_str().unwrap_or("");
            let port = alloc_json["port"].as_i64().unwrap_or(0) as i32;

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

    let allocations: Vec<AllocationInfo> = sqlx::query_as(
        r#"SELECT ca.id, ca.allocation_id, ca.ip, ca.port, ca.internal_port, ca.protocol, COALESCE(ca.is_primary, FALSE) as is_primary
           FROM container_allocations ca
           WHERE ca.container_id = $1
           ORDER BY ca.is_primary DESC, ca.ip, ca.port"#
    )
        .bind(id)
        .fetch_all(&state.db)
        .await?;

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

    if container.user_id != claims.sub && !claims.has_permission("containers.delete") && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = daemon_client();
    let daemon_url = format!("{}/containers/{}", daemon.base_url(), container.id);

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
    pub server_memory: Option<i64>,
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
    #[serde(default)]
    pub startup_script: Option<String>,
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

    let is_owner = container.user_id == claims.sub;
    let is_manager = claims.has_permission("containers.manage") || claims.is_manager();

    if !is_owner && !is_manager {
        return Err(AppError::Unauthorized);
    }

    let changing_resources = req.memory_limit.is_some()
        || req.server_memory.is_some()
        || req.cpu_limit.is_some()
        || req.disk_limit.is_some()
        || req.swap_limit.is_some()
        || req.io_weight.is_some();

    if changing_resources && !is_manager && !claims.has_permission("containers.edit_resources") {
        return Err(AppError::Forbidden("You don't have permission to change resource limits".into()));
    }

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let mut daemon_payload = serde_json::json!({});

    if let Some(memory) = req.memory_limit {
        daemon_payload["memoryLimit"] = serde_json::json!(memory);
    }
    if let Some(server_memory) = req.server_memory {
        daemon_payload["serverMemory"] = serde_json::json!(server_memory);
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
    if let Some(ref startup) = req.startup_script {
        daemon_payload["startupScript"] = serde_json::json!(startup);
    }

    let client = daemon_client();
    let daemon_url = format!("{}/containers/{}", daemon.base_url(), container.id);

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
    }

    let memory_limit = req.memory_limit.or(container.memory_limit);
    let cpu_limit = req.cpu_limit.map(|c| rust_decimal::Decimal::try_from(c).ok()).flatten().or(container.cpu_limit);
    let disk_limit = req.disk_limit.or(container.disk_limit);
    let swap_limit = req.swap_limit.or(container.swap_limit);
    let io_weight = req.io_weight.or(container.io_weight);
    let startup_script = req.startup_script.or(container.startup_script.clone());

    let updated_container: Container = sqlx::query_as(
        r#"UPDATE containers SET
            memory_limit = $1,
            cpu_limit = $2,
            disk_limit = $3,
            swap_limit = $4,
            io_weight = $5,
            startup_script = $6,
            updated_at = NOW()
        WHERE id = $7
        RETURNING *"#
    )
    .bind(memory_limit)
    .bind(cpu_limit)
    .bind(disk_limit)
    .bind(swap_limit)
    .bind(io_weight)
    .bind(&startup_script)
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    if let Some(allocation_id) = req.allocation_id {

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

    let client = daemon_client();
    let daemon_url = format!(
        "{}/containers/{}/{}",
        daemon.base_url(), container.id, action
    );

    let res = client
        .post(&daemon_url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::Daemon(e.to_string()))?;

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

    let allocations: Vec<crate::models::ContainerAllocation> = sqlx::query_as(
        r#"SELECT ca.id, ca.container_id, ca.allocation_id, ca.ip, ca.port, ca.internal_port, ca.protocol, COALESCE(ca.is_primary, FALSE) as is_primary, ca.created_at
           FROM container_allocations ca
           WHERE ca.container_id = $1
           ORDER BY ca.is_primary DESC, ca.ip, ca.port"#
    )
        .bind(id)
        .fetch_all(&state.db)
        .await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?;

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

        let update_url = format!("{}/containers/{}", daemon.base_url(), container.id);
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

    let start_url = format!("{}/containers/{}/start", daemon.base_url(), container.id);
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

    let client = daemon_client();

    let stop_command = container.stop_command.unwrap_or_else(|| "stop".to_string());

    let graceful_url = format!("{}/containers/{}/graceful-stop", daemon.base_url(), container.id);
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

            sqlx::query("UPDATE containers SET status = 'stopped', updated_at = NOW() WHERE id = $1")
                .bind(id)
                .execute(&state.db)
                .await?;

            Ok(Json(serde_json::json!({ "success": true, "method": "graceful" })))
        }
        _ => {

            tracing::warn!("Graceful stop failed for container {}, using Docker stop", id);

            let docker_stop_url = format!("{}/containers/{}/stop", daemon.base_url(), container.id);
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

    let client = daemon_client();

    let stop_command = container.stop_command.clone().unwrap_or_else(|| "stop".to_string());

    let stop_url = format!("{}/containers/{}/graceful-stop", daemon.base_url(), container.id);
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

        let force_stop_url = format!("{}/containers/{}/stop", daemon.base_url(), container.id);
        client
            .post(&force_stop_url)
            .header("X-API-Key", &daemon.api_key)
            .send()
            .await
            .map_err(|e| AppError::Daemon(e.to_string()))?;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let start_url = format!("{}/containers/{}/start", daemon.base_url(), container.id);
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

    let client = daemon_client();
    let url = format!("{}/containers/{}/command", daemon.base_url(), container.id);

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
    #[serde(default = "default_stop_timeout")]
    pub timeout_secs: u64,
}

fn default_stop_timeout() -> u64 {
    30
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

    let client = daemon_client();
    let url = format!("{}/containers/{}/graceful-stop", daemon.base_url(), container.id);

    let res = client
        .post(&url)
        .header("X-API-Key", &daemon.api_key)
        .json(&serde_json::json!({
            "timeoutSecs": req.timeout_secs
        }))
        .send()
        .await
        .map_err(|e| AppError::Daemon(e.to_string()))?;

    if !res.status().is_success() {
        let error_text = res.text().await.unwrap_or_default();
        return Err(AppError::Daemon(format!("Failed to stop: {}", error_text)));
    }

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

    if container.user_id != claims.sub && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    let allocation: crate::models::Allocation = sqlx::query_as(
        "SELECT * FROM allocations WHERE id = $1 AND daemon_id = $2"
    )
        .bind(req.allocation_id)
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::BadRequest("Allocation not found or belongs to different daemon".into()))?;

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

    let existing_for_container: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT id, COALESCE(is_primary, FALSE) FROM container_allocations WHERE allocation_id = $1 AND container_id = $2"
    )
        .bind(req.allocation_id)
        .bind(container.id)
        .fetch_optional(&state.db)
        .await?;

    if let Some((existing_id, is_already_primary)) = existing_for_container {
        if is_already_primary {

            return Ok(Json(serde_json::json!({
                "message": "Allocation is already primary",
                "allocationIp": allocation.ip,
                "allocationPort": allocation.port
            })));
        }

        sqlx::query("UPDATE container_allocations SET is_primary = FALSE WHERE container_id = $1 AND is_primary = TRUE")
            .bind(container.id)
            .execute(&state.db)
            .await?;

        sqlx::query("UPDATE container_allocations SET is_primary = TRUE WHERE id = $1")
            .bind(existing_id)
            .execute(&state.db)
            .await?;
    } else {

        sqlx::query("UPDATE container_allocations SET is_primary = FALSE WHERE container_id = $1 AND is_primary = TRUE")
            .bind(container.id)
            .execute(&state.db)
            .await?;

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

    if container.user_id != claims.sub && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

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

    if container.user_id != claims.sub && !claims.has_permission("containers.view_all") && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

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

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAllocationRequest {
    pub allocation_id: Uuid,
    #[serde(default)]
    pub is_primary: bool,
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

    if container.user_id != claims.sub && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    let allocation: crate::models::Allocation = sqlx::query_as(
        "SELECT * FROM allocations WHERE id = $1 AND daemon_id = $2"
    )
        .bind(req.allocation_id)
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::BadRequest("Allocation not found or belongs to different daemon".into()))?;

    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM container_allocations WHERE allocation_id = $1"
    )
        .bind(req.allocation_id)
        .fetch_optional(&state.db)
        .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Allocation is already in use".into()));
    }

    if req.is_primary {
        sqlx::query("UPDATE container_allocations SET is_primary = FALSE WHERE container_id = $1 AND is_primary = TRUE")
            .bind(container.id)
            .execute(&state.db)
            .await?;
    }

    let has_allocations: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM container_allocations WHERE container_id = $1"
    )
        .bind(container.id)
        .fetch_one(&state.db)
        .await
        .ok();

    let is_primary = req.is_primary || has_allocations.map(|c| c.0 == 0).unwrap_or(true);

    sqlx::query(
        r#"INSERT INTO container_allocations (id, container_id, allocation_id, ip, port, internal_port, protocol, is_primary, created_at)
           VALUES ($1, $2, $3, $4, $5, $5, $6, $7, NOW())"#
    )
        .bind(Uuid::new_v4())
        .bind(container.id)
        .bind(req.allocation_id)
        .bind(&allocation.ip)
        .bind(allocation.port)
        .bind(&allocation.protocol)
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

    if container.user_id != claims.sub && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    let container_allocation: (Uuid, bool) = sqlx::query_as(
        "SELECT id, COALESCE(is_primary, FALSE) FROM container_allocations WHERE allocation_id = $1 AND container_id = $2"
    )
        .bind(allocation_id)
        .bind(container.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::BadRequest("Allocation not found or doesn't belong to this container".into()))?;

    let was_primary = container_allocation.1;

    sqlx::query("DELETE FROM container_allocations WHERE id = $1")
        .bind(container_allocation.0)
        .execute(&state.db)
        .await?;

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

pub async fn set_primary_allocation(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((id, allocation_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if container.user_id != claims.sub && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    let container_allocation: Option<(Uuid, String, i32)> = sqlx::query_as(
        "SELECT id, ip, port FROM container_allocations WHERE allocation_id = $1 AND container_id = $2"
    )
        .bind(allocation_id)
        .bind(container.id)
        .fetch_optional(&state.db)
        .await?;

    if container_allocation.is_none() {
        return Err(AppError::BadRequest("Allocation not found or doesn't belong to this container".into()));
    }

    let (ca_id, ip, port) = container_allocation.unwrap();

    sqlx::query("UPDATE container_allocations SET is_primary = FALSE WHERE container_id = $1")
        .bind(container.id)
        .execute(&state.db)
        .await?;

    sqlx::query("UPDATE container_allocations SET is_primary = TRUE WHERE id = $1")
        .bind(ca_id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({
        "message": "Allocation set as primary",
        "allocationIp": ip,
        "allocationPort": port
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

    let daemon: crate::models::Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = daemon_client();
    let url = format!("{}/containers/{}/ftp", daemon.base_url(), container.id);

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

    let client = daemon_client();
    let path = query.path.unwrap_or_else(|| "/".to_string());

    let url = format!("{}/containers/{}/files?path={}", daemon.base_url(), container.id, path);

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

    let client = daemon_client();

    let url = format!("{}/containers/{}/files/read?path={}", daemon.base_url(), container.id, query.path);

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

    let client = daemon_client();

    let url = format!("{}/containers/{}/files/write", daemon.base_url(), container.id);

    let resp = client
        .post(&url)
        .header("X-API-Key", &daemon.api_key)
        .json(&req)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Daemon error: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(AppError::BadRequest(format!("Daemon returned {}: {}", status, error_text)));
    }

    let result: serde_json::Value = resp.json().await
        .map_err(|e| AppError::BadRequest(format!("Parse error: {}", e)))?;

    Ok(Json(result))
}

pub async fn upload_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    mut multipart: axum::extract::Multipart,
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

    let mut path: Option<String> = None;
    let mut file_content: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))? {
        let field_name = field.name().unwrap_or("").to_string();

        if field_name == "path" {
            path = Some(field.text().await.map_err(|e| AppError::BadRequest(format!("Read error: {}", e)))?);
        } else if field_name == "file" {
            file_content = Some(field.bytes().await.map_err(|e| AppError::BadRequest(format!("Read error: {}", e)))?.to_vec());
        }
    }

    let path = path.ok_or_else(|| AppError::BadRequest("Missing 'path' field".to_string()))?;
    let content = file_content.ok_or_else(|| AppError::BadRequest("Missing 'file' field".to_string()))?;

    let content_base64 = general_purpose::STANDARD.encode(&content);

    let client = daemon_client();
    let url = format!("{}/containers/{}/files/write", daemon.base_url(), container.id);

    let resp = client
        .post(&url)
        .header("X-API-Key", &daemon.api_key)
        .json(&serde_json::json!({
            "path": path,
            "content": content_base64,
            "encoding": "base64"
        }))
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Daemon error: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(AppError::BadRequest(format!("Daemon returned {}: {}", status, error_text)));
    }

    Ok(Json(serde_json::json!({ "message": "File uploaded successfully" })))
}

pub async fn upload_file_chunk(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    mut multipart: axum::extract::Multipart,
) -> AppResult<Json<serde_json::Value>> {
    tracing::info!("upload_file_chunk: Starting chunk upload for container {}", id);

    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if !can_access_container(&claims, &container) {
        return Err(AppError::Unauthorized);
    }

    let mut upload_id: Option<String> = None;
    let mut chunk_index: Option<u32> = None;
    let mut total_chunks: Option<u32> = None;
    let mut path: Option<String> = None;
    let mut chunk_data: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::error!("upload_file_chunk: Multipart next_field error: {}", e);
        AppError::BadRequest(format!("Multipart error: {}", e))
    })? {
        let field_name = field.name().unwrap_or("").to_string();
        tracing::debug!("upload_file_chunk: Processing field: {}", field_name);

        match field_name.as_str() {
            "uploadId" => upload_id = Some(field.text().await.map_err(|e| {
                tracing::error!("upload_file_chunk: Error reading uploadId: {}", e);
                AppError::BadRequest(format!("Read error: {}", e))
            })?),
            "chunkIndex" => chunk_index = Some(field.text().await.map_err(|e| {
                tracing::error!("upload_file_chunk: Error reading chunkIndex: {}", e);
                AppError::BadRequest(format!("Read error: {}", e))
            })?.parse().map_err(|_| AppError::BadRequest("Invalid chunk index".to_string()))?),
            "totalChunks" => total_chunks = Some(field.text().await.map_err(|e| {
                tracing::error!("upload_file_chunk: Error reading totalChunks: {}", e);
                AppError::BadRequest(format!("Read error: {}", e))
            })?.parse().map_err(|_| AppError::BadRequest("Invalid total chunks".to_string()))?),
            "path" => path = Some(field.text().await.map_err(|e| {
                tracing::error!("upload_file_chunk: Error reading path: {}", e);
                AppError::BadRequest(format!("Read error: {}", e))
            })?),
            "fileName" | "fileSize" => {  }
            "chunk" => {
                tracing::info!("upload_file_chunk: Reading chunk data...");
                chunk_data = Some(field.bytes().await.map_err(|e| {
                    tracing::error!("upload_file_chunk: Error reading chunk bytes: {}", e);
                    AppError::BadRequest(format!("Read error: {}", e))
                })?.to_vec());
                tracing::info!("upload_file_chunk: Chunk data read, size: {} bytes", chunk_data.as_ref().map(|d| d.len()).unwrap_or(0));
            }
            _ => {
                tracing::debug!("upload_file_chunk: Ignoring unknown field: {}", field_name);
            }
        }
    }

    tracing::info!("upload_file_chunk: Parsed - uploadId: {:?}, chunkIndex: {:?}, totalChunks: {:?}, path: {:?}, chunk_size: {:?}",
        upload_id, chunk_index, total_chunks, path, chunk_data.as_ref().map(|d| d.len()));

    let upload_id = upload_id.ok_or_else(|| AppError::BadRequest("Missing uploadId".to_string()))?;
    let chunk_index = chunk_index.ok_or_else(|| AppError::BadRequest("Missing chunkIndex".to_string()))?;
    let total_chunks = total_chunks.ok_or_else(|| AppError::BadRequest("Missing totalChunks".to_string()))?;
    let path = path.ok_or_else(|| AppError::BadRequest("Missing path".to_string()))?;
    let chunk_data = chunk_data.ok_or_else(|| AppError::BadRequest("Missing chunk data".to_string()))?;

    tracing::info!("upload_file_chunk: Getting daemon info for container {}", container.id);

    let daemon: crate::models::Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    tracing::info!("upload_file_chunk: Found daemon {}, encoding {} bytes to base64", daemon.id, chunk_data.len());

    let content_base64 = general_purpose::STANDARD.encode(&chunk_data);

    tracing::info!("upload_file_chunk: Base64 encoded size: {} bytes", content_base64.len());

    let client = daemon_client();
    let url = format!("{}/containers/{}/files/write-chunk", daemon.base_url(), container.id);

    tracing::info!("upload_file_chunk: Sending chunk {} of {} to daemon at {}", chunk_index, total_chunks, url);

    let resp = client
        .post(&url)
        .header("X-API-Key", &daemon.api_key)
        .timeout(std::time::Duration::from_secs(120))
        .json(&serde_json::json!({
            "uploadId": upload_id,
            "chunkIndex": chunk_index,
            "totalChunks": total_chunks,
            "path": path,
            "content": content_base64
        }))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("upload_file_chunk: Daemon request failed: {}", e);
            AppError::BadRequest(format!("Daemon error: {}", e))
        })?;

    tracing::info!("upload_file_chunk: Daemon responded with status {}", resp.status());

    if !resp.status().is_success() {
        let status = resp.status();
        let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        tracing::error!("upload_file_chunk: Daemon error {}: {}", status, error_text);
        return Err(AppError::BadRequest(format!("Daemon returned {}: {}", status, error_text)));
    }

    let daemon_response: serde_json::Value = resp.json().await
        .map_err(|e| {
            tracing::error!("upload_file_chunk: Failed to parse daemon response: {}", e);
            AppError::BadRequest(format!("Invalid daemon response: {}", e))
        })?;

    tracing::info!("upload_file_chunk: Chunk {} uploaded successfully", chunk_index);

    Ok(Json(daemon_response))
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

    let client = daemon_client();

    let url = format!("{}/containers/{}/files/folder", daemon.base_url(), container.id);

    let resp = client
        .post(&url)
        .header("X-API-Key", &daemon.api_key)
        .json(&req)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Daemon error: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(AppError::BadRequest(format!("Daemon returned {}: {}", status, error_text)));
    }

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

    let client = daemon_client();

    let url = format!("{}/containers/{}/files/delete?path={}", daemon.base_url(), container.id, query.path);

    let resp = client
        .delete(&url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Daemon error: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(AppError::BadRequest(format!("Daemon returned {}: {}", status, error_text)));
    }

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

    let client = daemon_client();
    let url = format!("{}/containers/{}/stats", daemon.base_url(), container.id);

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

// --- Container Variables ---

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerVariableResponse {
    pub env_variable: String,
    pub name: String,
    pub description: Option<String>,
    pub default_value: Option<String>,
    pub value: String,
    pub user_viewable: bool,
    pub user_editable: bool,
    pub rules: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerStartupResponse {
    pub startup_script: Option<String>,
    pub variables: Vec<ContainerVariableResponse>,
}

/// GET /containers/:id/startup - returns flake variables with current values + startup script
pub async fn get_container_startup(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ContainerStartupResponse>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    if container.user_id != claims.sub && !claims.has_permission("containers.view_all") && !claims.is_manager() {
        return Err(AppError::Unauthorized);
    }

    let mut variables: Vec<ContainerVariableResponse> = Vec::new();

    if let Some(flake_id) = container.flake_id {
        let flake_vars: Vec<crate::handlers::flakes::FlakeVariable> = sqlx::query_as(
            "SELECT * FROM flake_variables WHERE flake_id = $1 ORDER BY sort_order"
        )
            .bind(flake_id)
            .fetch_all(&state.db)
            .await?;

        // Get stored container variable values
        let stored_values: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT flake_variable_id, value FROM container_variables WHERE container_id = $1"
        )
            .bind(id)
            .fetch_all(&state.db)
            .await?;

        let stored_map: HashMap<Uuid, String> = stored_values.into_iter().collect();

        // If no stored values, try to get current values from daemon
        let daemon_env = if stored_map.is_empty() {
            get_daemon_environment(&state, &container).await.unwrap_or_default()
        } else {
            HashMap::new()
        };

        for var in &flake_vars {
            let value = stored_map.get(&var.id)
                .cloned()
                .or_else(|| daemon_env.get(&var.env_variable).cloned())
                .unwrap_or_else(|| var.default_value.clone().unwrap_or_default());

            variables.push(ContainerVariableResponse {
                env_variable: var.env_variable.clone(),
                name: var.name.clone(),
                description: var.description.clone(),
                default_value: var.default_value.clone(),
                value,
                user_viewable: var.user_viewable,
                user_editable: var.user_editable,
                rules: var.rules.clone(),
                sort_order: var.sort_order,
            });
        }
    }

    Ok(Json(ContainerStartupResponse {
        startup_script: container.startup_script,
        variables,
    }))
}

/// Fetch current environment from daemon for existing containers without stored variables
async fn get_daemon_environment(
    state: &AppState,
    container: &Container,
) -> Result<HashMap<String, String>, AppError> {
    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = daemon_client();
    let url = format!("{}/containers/{}", daemon.base_url(), container.id);

    let resp = client
        .get(&url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::Daemon(e.to_string()))?;

    if !resp.status().is_success() {
        return Ok(HashMap::new());
    }

    let data: serde_json::Value = resp.json().await.unwrap_or_default();
    let mut env = HashMap::new();
    if let Some(obj) = data.get("environment").and_then(|e| e.as_object()) {
        for (k, v) in obj {
            env.insert(k.clone(), v.as_str().unwrap_or("").to_string());
        }
    }
    Ok(env)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateContainerStartupRequest {
    #[serde(default)]
    pub startup_script: Option<String>,
    #[serde(default)]
    pub variables: Option<HashMap<String, String>>,
}

/// PUT /containers/:id/startup - update variables and/or startup script
pub async fn update_container_startup(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateContainerStartupRequest>,
) -> AppResult<Json<ContainerStartupResponse>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let is_owner = container.user_id == claims.sub;
    let is_manager = claims.has_permission("containers.manage") || claims.is_manager();

    if !is_owner && !is_manager {
        return Err(AppError::Unauthorized);
    }

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let mut daemon_payload = serde_json::json!({});

    // Update startup script in DB and daemon
    if let Some(ref startup) = req.startup_script {
        sqlx::query("UPDATE containers SET startup_script = $1, updated_at = NOW() WHERE id = $2")
            .bind(startup)
            .bind(id)
            .execute(&state.db)
            .await?;
        daemon_payload["startupScript"] = serde_json::json!(startup);
    }

    // Update variables
    if let Some(ref variables) = req.variables {
        if let Some(flake_id) = container.flake_id {
            let flake_vars: Vec<crate::handlers::flakes::FlakeVariable> = sqlx::query_as(
                "SELECT * FROM flake_variables WHERE flake_id = $1 ORDER BY sort_order"
            )
                .bind(flake_id)
                .fetch_all(&state.db)
                .await?;

            let mut env_updates: HashMap<String, String> = HashMap::new();

            for var in &flake_vars {
                if let Some(new_value) = variables.get(&var.env_variable) {
                    // Only allow editing user_editable vars (unless manager)
                    if !var.user_editable && !is_manager {
                        continue;
                    }

                    // Upsert into container_variables
                    sqlx::query(
                        r#"INSERT INTO container_variables (id, container_id, flake_variable_id, value, created_at, updated_at)
                           VALUES ($1, $2, $3, $4, NOW(), NOW())
                           ON CONFLICT (container_id, flake_variable_id)
                           DO UPDATE SET value = $4, updated_at = NOW()"#
                    )
                        .bind(Uuid::new_v4())
                        .bind(id)
                        .bind(var.id)
                        .bind(new_value)
                        .execute(&state.db)
                        .await?;

                    env_updates.insert(var.env_variable.clone(), new_value.clone());
                }
            }

            if !env_updates.is_empty() {
                daemon_payload["environment"] = serde_json::json!(env_updates);
            }
        }
    }

    // Send updates to daemon
    if daemon_payload.as_object().map_or(false, |o| !o.is_empty()) {
        let client = daemon_client();
        let daemon_url = format!("{}/containers/{}", daemon.base_url(), container.id);

        let res = client
            .patch(&daemon_url)
            .header("X-API-Key", &daemon.api_key)
            .json(&daemon_payload)
            .send()
            .await
            .map_err(|e| AppError::Daemon(e.to_string()))?;

        if !res.status().is_success() {
            let error_text = res.text().await.unwrap_or_default();
            tracing::warn!("Daemon startup update warning: {}", error_text);
        }
    }

    // Return updated state
    get_container_startup(State(state), Extension(claims), Path(id)).await
}

pub async fn download_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Query(query): Query<ReadFileQuery>,
) -> Result<axum::response::Response, AppError> {
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

    let client = daemon_client();
    let url = format!("{}/containers/{}/files/download?path={}", daemon.base_url(), container.id, query.path);

    let resp = client
        .get(&url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::Daemon(format!("Download error: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::BadRequest("File not found".into()));
    }

    let headers = resp.headers().clone();
    let bytes = resp.bytes().await
        .map_err(|e| AppError::Daemon(format!("Read error: {}", e)))?;

    let content_disposition = headers
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("attachment")
        .to_string();

    use axum::response::IntoResponse;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (axum::http::header::CONTENT_DISPOSITION, content_disposition),
        ],
        bytes.to_vec(),
    ).into_response())
}

pub async fn fix_permissions(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let is_owner = container.user_id == claims.sub;
    let is_manager = claims.has_permission("containers.manage") || claims.is_manager();

    if !is_owner && !is_manager {
        return Err(AppError::Unauthorized);
    }

    let daemon: crate::models::Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let client = daemon_client();
    let url = format!("{}/containers/{}/fix-permissions", daemon.base_url(), container.id);

    let resp = client
        .post(&url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::Daemon(format!("Fix permissions error: {}", e)))?;

    if !resp.status().is_success() {
        let error_text = resp.text().await.unwrap_or_default();
        return Err(AppError::Daemon(format!("Failed to fix permissions: {}", error_text)));
    }

    Ok(Json(serde_json::json!({"message": "Permissions fixed successfully"})))
}
