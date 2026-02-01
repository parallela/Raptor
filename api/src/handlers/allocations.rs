use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{Allocation, AppState, CreateAllocationRequest, CreateIpPoolRequest, IpPool};

/// List all allocations (for admin page) - includes both assigned and unassigned
pub async fn list_all_allocations(State(state): State<AppState>) -> AppResult<Json<Vec<Allocation>>> {
    let allocations: Vec<Allocation> = sqlx::query_as(
        r#"SELECT * FROM allocations ORDER BY created_at DESC"#
    )
        .fetch_all(&state.db)
        .await?;
    Ok(Json(allocations))
}

/// List only available (unassigned) allocations - for container creation dropdowns
pub async fn list_allocations(State(state): State<AppState>) -> AppResult<Json<Vec<Allocation>>> {
    // Only return allocations that are NOT already assigned to a container
    let allocations: Vec<Allocation> = sqlx::query_as(
        r#"SELECT a.* FROM allocations a
           WHERE NOT EXISTS (
               SELECT 1 FROM container_allocations ca
               WHERE ca.allocation_id = a.id
           )
           ORDER BY a.created_at DESC"#
    )
        .fetch_all(&state.db)
        .await?;
    Ok(Json(allocations))
}

pub async fn create_allocation(
    State(state): State<AppState>,
    Json(req): Json<CreateAllocationRequest>,
) -> AppResult<Json<Allocation>> {
    let now = Utc::now();
    let protocol = req.protocol.as_deref().unwrap_or("tcp");

    let allocation: Allocation = sqlx::query_as(
        r#"
        INSERT INTO allocations (id, daemon_id, ip, port, protocol, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $6)
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(req.daemon_id)
    .bind(&req.ip)
    .bind(req.port)
    .bind(protocol)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(allocation))
}

pub async fn delete_allocation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM container_allocations WHERE allocation_id = $1"
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .ok();

    if existing.map(|c| c.0 > 0).unwrap_or(false) {
        return Err(AppError::BadRequest("Cannot delete allocation that is in use by a container".into()));
    }

    let result = sqlx::query("DELETE FROM allocations WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({"message": "Allocation deleted successfully"})))
}

pub async fn list_ip_pools(State(state): State<AppState>) -> AppResult<Json<Vec<IpPool>>> {
    let pools: Vec<IpPool> = sqlx::query_as("SELECT * FROM ip_pools ORDER BY created_at DESC")
        .fetch_all(&state.db)
        .await?;
    Ok(Json(pools))
}

pub async fn list_daemon_ip_pools(
    State(state): State<AppState>,
    Path(daemon_id): Path<Uuid>,
) -> AppResult<Json<Vec<IpPool>>> {
    let pools: Vec<IpPool> = sqlx::query_as("SELECT * FROM ip_pools WHERE daemon_id = $1 ORDER BY is_primary DESC, created_at")
        .bind(daemon_id)
        .fetch_all(&state.db)
        .await?;
    Ok(Json(pools))
}

pub async fn create_ip_pool(
    State(state): State<AppState>,
    Json(req): Json<CreateIpPoolRequest>,
) -> AppResult<Json<IpPool>> {
    let pool: IpPool = sqlx::query_as(
        r#"
        INSERT INTO ip_pools (id, daemon_id, ip_address, cidr, description, is_primary)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(req.daemon_id)
    .bind(&req.ip_address)
    .bind(req.cidr.unwrap_or(32))
    .bind(&req.description)
    .bind(req.is_primary.unwrap_or(false))
    .fetch_one(&state.db)
    .await?;

    Ok(Json(pool))
}

pub async fn delete_ip_pool(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let result = sqlx::query("DELETE FROM ip_pools WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({"message": "IP pool deleted successfully"})))
}

pub async fn list_container_allocations(
    State(state): State<AppState>,
    Path(container_id): Path<Uuid>,
) -> AppResult<Json<Vec<crate::models::ContainerAllocation>>> {
    let allocations: Vec<crate::models::ContainerAllocation> = sqlx::query_as(
        "SELECT * FROM container_allocations WHERE container_id = $1 ORDER BY is_primary DESC, created_at"
    )
    .bind(container_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(allocations))
}

pub async fn create_container_allocation(
    State(state): State<AppState>,
    Json(req): Json<crate::models::CreateContainerAllocationRequest>,
) -> AppResult<Json<crate::models::ContainerAllocation>> {
    // If setting as primary, remove existing primary first
    if req.is_primary.unwrap_or(false) {
        sqlx::query("UPDATE container_allocations SET is_primary = FALSE WHERE container_id = $1 AND is_primary = TRUE")
            .bind(req.container_id)
            .execute(&state.db)
            .await?;
    }

    // Get allocation details (ip and port)
    let allocation: crate::models::Allocation = sqlx::query_as(
        "SELECT * FROM allocations WHERE id = $1"
    )
        .bind(req.allocation_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    // Use protocol from the allocation table
    let container_allocation: crate::models::ContainerAllocation = sqlx::query_as(
        r#"
        INSERT INTO container_allocations (id, container_id, allocation_id, ip, port, internal_port, protocol, is_primary)
        VALUES ($1, $2, $3, $4, $5, $5, $6, $7)
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(req.container_id)
    .bind(req.allocation_id)
    .bind(&allocation.ip)
    .bind(allocation.port)
    .bind(&allocation.protocol)
    .bind(req.is_primary.unwrap_or(false))
    .fetch_one(&state.db)
    .await?;

    Ok(Json(container_allocation))
}

pub async fn delete_container_allocation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let result = sqlx::query("DELETE FROM container_allocations WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({"message": "Container allocation deleted successfully"})))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAllocationRequest {
    pub protocol: Option<String>,
}

pub async fn update_allocation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateAllocationRequest>,
) -> AppResult<Json<crate::models::Allocation>> {
    let existing: crate::models::Allocation = sqlx::query_as(
        "SELECT * FROM allocations WHERE id = $1"
    )
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let protocol = req.protocol.unwrap_or(existing.protocol);

    let updated: crate::models::Allocation = sqlx::query_as(
        r#"
        UPDATE allocations SET protocol = $1, updated_at = NOW()
        WHERE id = $2
        RETURNING *
        "#,
    )
    .bind(&protocol)
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    sqlx::query(
        "UPDATE container_allocations SET protocol = $1 WHERE allocation_id = $2"
    )
    .bind(&protocol)
    .bind(id)
    .execute(&state.db)
    .await?;

    #[derive(sqlx::FromRow)]
    struct ContainerDaemonInfo {
        container_id: Uuid,
        daemon_id: Uuid,
    }

    let containers: Vec<ContainerDaemonInfo> = sqlx::query_as(
        r#"
        SELECT ca.container_id, c.daemon_id
        FROM container_allocations ca
        JOIN containers c ON c.id = ca.container_id
        WHERE ca.allocation_id = $1
        "#
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    for container_info in containers {
        #[derive(sqlx::FromRow)]
        struct AllocationRow {
            id: Uuid,
            allocation_id: Option<Uuid>,
            ip: String,
            port: i32,
            internal_port: i32,
            protocol: String,
            is_primary: Option<bool>,
        }

        let allocs: Vec<AllocationRow> = sqlx::query_as(
            "SELECT id, allocation_id, ip, port, internal_port, protocol, is_primary FROM container_allocations WHERE container_id = $1"
        )
        .bind(container_info.container_id)
        .fetch_all(&state.db)
        .await?;

        let daemon: crate::models::Daemon = sqlx::query_as(
            "SELECT * FROM daemons WHERE id = $1"
        )
        .bind(container_info.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

        let allocations_for_daemon: Vec<serde_json::Value> = allocs.iter().map(|a| {
            serde_json::json!({
                "id": a.id.to_string(),
                "allocationId": a.allocation_id.map(|id| id.to_string()),
                "ip": a.ip,
                "port": a.port,
                "internalPort": a.internal_port,
                "protocol": a.protocol,
                "isPrimary": a.is_primary.unwrap_or(false)
            })
        }).collect();

        let client = crate::handlers::containers::daemon_client();
        let update_url = format!("{}/containers/{}", daemon.base_url(), container_info.container_id);

        let _ = client
            .patch(&update_url)
            .header("X-API-Key", &daemon.api_key)
            .json(&serde_json::json!({
                "allocations": allocations_for_daemon
            }))
            .send()
            .await;
    }

    Ok(Json(updated))
}

