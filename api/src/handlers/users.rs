use axum::{
    extract::{Path, Query, State},
    Extension,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{AppState, Claims, UserResponse};

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    #[serde(default)]
    pub q: String,
    #[serde(default = "default_search_limit")]
    pub limit: i64,
}

fn default_page() -> i64 { 1 }
fn default_per_page() -> i64 { 20 }
fn default_search_limit() -> i64 { 10 }

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct UserWithRole {
    pub id: Uuid,
    pub username: String,
    pub email: Option<String>,
    pub role_id: Option<Uuid>,
    pub role_name: Option<String>,
    pub permissions: Option<serde_json::Value>,
}

pub async fn list_users(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> AppResult<Json<PaginatedResponse<UserResponse>>> {
    let per_page = params.per_page.min(100).max(1);
    let page = params.page.max(1);
    let offset = (page - 1) * per_page;

    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?;

    let users: Vec<UserWithRole> = sqlx::query_as(
        r#"
        SELECT
            u.id, u.username, u.email, u.role_id,
            r.name as role_name, r.permissions
        FROM users u
        LEFT JOIN roles r ON u.role_id = r.id
        ORDER BY u.created_at DESC
        LIMIT $1 OFFSET $2
        "#
    )
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let data: Vec<UserResponse> = users
        .into_iter()
        .map(|u| {
            let avatar_url = UserResponse::gravatar_url(u.email.as_deref());
            UserResponse {
                id: u.id,
                username: u.username,
                email: u.email,
                avatar_url,
                role_id: u.role_id,
                role_name: u.role_name,
                permissions: u.permissions.unwrap_or(serde_json::json!({})),
            }
        })
        .collect();

    let total_pages = (total as f64 / per_page as f64).ceil() as i64;

    Ok(Json(PaginatedResponse {
        data,
        total,
        page,
        per_page,
        total_pages,
    }))
}

pub async fn search_users(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> AppResult<Json<Vec<UserResponse>>> {
    let limit = params.limit.min(50).max(1);
    let search_term = format!("%{}%", params.q.to_lowercase());

    let users: Vec<UserWithRole> = sqlx::query_as(
        r#"
        SELECT
            u.id, u.username, u.email, u.role_id,
            r.name as role_name, r.permissions
        FROM users u
        LEFT JOIN roles r ON u.role_id = r.id
        WHERE LOWER(u.username) LIKE $1 OR LOWER(u.email) LIKE $1
        ORDER BY u.username ASC
        LIMIT $2
        "#
    )
    .bind(&search_term)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    let data: Vec<UserResponse> = users
        .into_iter()
        .map(|u| {
            let avatar_url = UserResponse::gravatar_url(u.email.as_deref());
            UserResponse {
                id: u.id,
                username: u.username,
                email: u.email,
                avatar_url,
                role_id: u.role_id,
                role_name: u.role_name,
                permissions: u.permissions.unwrap_or(serde_json::json!({})),
            }
        })
        .collect();

    Ok(Json(data))
}

pub async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserResponse>> {

    let user: UserWithRole = sqlx::query_as(
        r#"
        SELECT
            u.id, u.username, u.email, u.role_id,
            r.name as role_name, r.permissions
        FROM users u
        LEFT JOIN roles r ON u.role_id = r.id
        WHERE u.id = $1
        "#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let avatar_url = UserResponse::gravatar_url(user.email.as_deref());

    Ok(Json(UserResponse {
        id: user.id,
        username: user.username,
        email: user.email,
        avatar_url,
        role_id: user.role_id,
        role_name: user.role_name,
        permissions: user.permissions.unwrap_or(serde_json::json!({})),
    }))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub email: Option<String>,
    pub role_id: Option<Uuid>,
}

pub async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> AppResult<Json<UserResponse>> {

    sqlx::query(
        r#"
        UPDATE users SET
            username = COALESCE($2, username),
            email = COALESCE($3, email),
            role_id = COALESCE($4, role_id),
            updated_at = NOW()
        WHERE id = $1
        "#
    )
    .bind(id)
    .bind(&req.username)
    .bind(&req.email)
    .bind(req.role_id)
    .execute(&state.db)
    .await?;

    let user: UserWithRole = sqlx::query_as(
        r#"
        SELECT
            u.id, u.username, u.email, u.role_id,
            r.name as role_name, r.permissions
        FROM users u
        LEFT JOIN roles r ON u.role_id = r.id
        WHERE u.id = $1
        "#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let avatar_url = UserResponse::gravatar_url(user.email.as_deref());

    Ok(Json(UserResponse {
        id: user.id,
        username: user.username,
        email: user.email,
        avatar_url,
        role_id: user.role_id,
        role_name: user.role_name,
        permissions: user.permissions.unwrap_or(serde_json::json!({})),
    }))
}

pub async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<()>> {
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(()))
}

pub async fn get_me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> AppResult<Json<UserResponse>> {
    let user: UserWithRole = sqlx::query_as(
        r#"
        SELECT
            u.id, u.username, u.email, u.role_id,
            r.name as role_name, r.permissions
        FROM users u
        LEFT JOIN roles r ON u.role_id = r.id
        WHERE u.id = $1
        "#
    )
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let avatar_url = UserResponse::gravatar_url(user.email.as_deref());

    Ok(Json(UserResponse {
        id: user.id,
        username: user.username,
        email: user.email,
        avatar_url,
        role_id: user.role_id,
        role_name: user.role_name,
        permissions: user.permissions.unwrap_or(serde_json::json!({})),
    }))
}

pub async fn invite_user(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<crate::models::InviteUserRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let existing: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&req.email)
        .fetch_optional(&state.db)
        .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("User with this email already exists".into()));
    }

    let token = crate::email::generate_reset_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::days(7);

    sqlx::query(
        r#"
        INSERT INTO user_invites (id, email, token, role_id, invited_by, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#
    )
    .bind(Uuid::new_v4())
    .bind(&req.email)
    .bind(&token)
    .bind(req.role_id)
    .bind(claims.sub)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    if let Some(ref smtp_config) = state.config.smtp {
        match crate::email::EmailService::new(smtp_config, &state.config.app_url) {
            Ok(email_service) => {
                match email_service.send_invite_email(
                    &req.email,
                    &token,
                    &claims.username,
                ).await {
                    Ok(_) => tracing::info!("Invite email sent to {}", req.email),
                    Err(e) => tracing::error!("Failed to send invite email to {}: {}", req.email, e),
                }
            },
            Err(e) => tracing::error!("Failed to create email service: {}", e),
        }
    } else {
        tracing::info!("SMTP not configured. Invite token for {}: {}", req.email, token);
    }

    Ok(Json(serde_json::json!({
        "message": "Invitation sent successfully",
        "token": token
    })))
}

pub async fn accept_invite(
    State(state): State<AppState>,
    Json(req): Json<crate::models::AcceptInviteRequest>,
) -> AppResult<Json<UserResponse>> {
    let invite: Option<crate::models::UserInvite> = sqlx::query_as(
        "SELECT * FROM user_invites WHERE token = $1 AND used = false AND expires_at > NOW()"
    )
    .bind(&req.token)
    .fetch_optional(&state.db)
    .await?;

    let invite = invite.ok_or(AppError::BadRequest("Invalid or expired invitation".into()))?;

    let existing: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE username = $1")
        .bind(&req.username)
        .fetch_optional(&state.db)
        .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Username already taken".into()));
    }

    let password_hash = bcrypt::hash(&req.password, state.config.bcrypt_cost)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let user_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO users (id, username, email, password_hash, role_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
        "#
    )
    .bind(user_id)
    .bind(&req.username)
    .bind(&invite.email)
    .bind(&password_hash)
    .bind(invite.role_id)
    .execute(&state.db)
    .await?;

    sqlx::query("UPDATE user_invites SET used = true WHERE id = $1")
        .bind(invite.id)
        .execute(&state.db)
        .await?;

    let user: UserWithRole = sqlx::query_as(
        r#"
        SELECT
            u.id, u.username, u.email, u.role_id,
            r.name as role_name, r.permissions
        FROM users u
        LEFT JOIN roles r ON u.role_id = r.id
        WHERE u.id = $1
        "#
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    let avatar_url = UserResponse::gravatar_url(user.email.as_deref());

    Ok(Json(UserResponse {
        id: user.id,
        username: user.username,
        email: user.email,
        avatar_url,
        role_id: user.role_id,
        role_name: user.role_name,
        permissions: user.permissions.unwrap_or(serde_json::json!({})),
    }))
}

