use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{AppState, Role, RoleResponse};

pub async fn list_roles(State(state): State<AppState>) -> AppResult<Json<Vec<RoleResponse>>> {
    let roles: Vec<Role> = sqlx::query_as("SELECT * FROM roles ORDER BY name")
        .fetch_all(&state.db)
        .await?;

    Ok(Json(
        roles
            .into_iter()
            .map(|r| RoleResponse {
                id: r.id,
                name: r.name,
                permissions: r.permissions,
            })
            .collect(),
    ))
}

pub async fn get_role(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<RoleResponse>> {
    let role: Role = sqlx::query_as("SELECT * FROM roles WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(RoleResponse {
        id: role.id,
        name: role.name,
        permissions: role.permissions,
    }))
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    pub permissions: serde_json::Value,
}

pub async fn create_role(
    State(state): State<AppState>,
    Json(req): Json<CreateRoleRequest>,
) -> AppResult<Json<RoleResponse>> {
    let now = Utc::now();
    let role: Role = sqlx::query_as(
        r#"
        INSERT INTO roles (id, name, permissions, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4)
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(&req.name)
    .bind(&req.permissions)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(RoleResponse {
        id: role.id,
        name: role.name,
        permissions: role.permissions,
    }))
}

pub async fn update_role(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateRoleRequest>,
) -> AppResult<Json<RoleResponse>> {
    let role: Role = sqlx::query_as(
        r#"
        UPDATE roles SET name = $2, permissions = $3, updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(&req.name)
    .bind(&req.permissions)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(RoleResponse {
        id: role.id,
        name: role.name,
        permissions: role.permissions,
    }))
}

pub async fn delete_role(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<()>> {
    let builtin_ids = [
        "00000000-0000-0000-0000-000000000001",
        "00000000-0000-0000-0000-000000000002",
        "00000000-0000-0000-0000-000000000003",
    ];

    if builtin_ids.contains(&id.to_string().as_str()) {
        return Err(AppError::BadRequest("Cannot delete built-in roles".into()));
    }

    sqlx::query("DELETE FROM roles WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(()))
}
