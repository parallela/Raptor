use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{Allocation, AppState, CreateAllocationRequest, CreateIpPoolRequest, IpPool};

pub async fn list_allocations(State(state): State<AppState>) -> AppResult<Json<Vec<Allocation>>> {
    let allocations: Vec<Allocation> =
        sqlx::query_as("SELECT * FROM allocations ORDER BY created_at DESC")
            .fetch_all(&state.db)
            .await?;
    Ok(Json(allocations))
}

pub async fn create_allocation(
    State(state): State<AppState>,
    Json(req): Json<CreateAllocationRequest>,
) -> AppResult<Json<Allocation>> {
    let now = Utc::now();

    let allocation: Allocation = sqlx::query_as(
        r#"
        INSERT INTO allocations (id, daemon_id, ip, port, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $5)
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(req.daemon_id)
    .bind(&req.ip)
    .bind(req.port)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(allocation))
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
    .bind(&req.protocol)
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
