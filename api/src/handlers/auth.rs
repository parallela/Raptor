use axum::{extract::State, Json};
use bcrypt::{hash, verify};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use uuid::Uuid;

use crate::email::{generate_reset_token, EmailService};
use crate::error::{AppError, AppResult};
use crate::models::{
    AppState, Claims, ForgotPasswordRequest, LoginRequest, LoginResponse,
    RegisterRequest, ResetPasswordRequest, Role, User, UserResponse,
};

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    let user: User = sqlx::query_as("SELECT * FROM users WHERE username = $1 OR email = $1")
        .bind(&req.username)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !verify(&req.password, &user.password_hash).unwrap_or(false) {
        return Err(AppError::Unauthorized);
    }

    let (role_name, _) = fetch_user_role(&state, user.role_id).await?;
    let permission_list = fetch_user_permissions(&state.db, user.id, user.role_id).await?;
    let permissions = serde_json::json!(permission_list.iter().map(|p| (p.clone(), true)).collect::<std::collections::HashMap<_, _>>());

    let exp = (Utc::now().timestamp() + 86400 * state.config.jwt_expiry_days) as usize;
    let claims = Claims {
        sub: user.id,
        username: user.username.clone(),
        role_id: user.role_id,
        role_name: role_name.clone(),
        permissions: permissions.clone(),
        exp,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let avatar_url = UserResponse::gravatar_url(user.email.as_deref());

    Ok(Json(LoginResponse {
        token,
        user: UserResponse {
            id: user.id,
            username: user.username,
            email: user.email,
            avatar_url,
            role_id: user.role_id,
            role_name,
            permissions,
        },
    }))
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<Json<UserResponse>> {
    if !req.email.contains('@') {
        return Err(AppError::BadRequest("Invalid email format".into()));
    }

    let existing: Option<User> = sqlx::query_as(
        "SELECT * FROM users WHERE username = $1 OR email = $2"
    )
    .bind(&req.username)
    .bind(&req.email)
    .fetch_optional(&state.db)
    .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Username or email already exists".into()));
    }

    let password_hash = hash(&req.password, state.config.bcrypt_cost)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let default_role: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM roles WHERE name = 'user'")
        .fetch_optional(&state.db)
        .await?;

    let default_role_id = default_role.map(|(id,)| id);

    let user: User = sqlx::query_as(
        r#"
        INSERT INTO users (id, username, email, password_hash, role_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $6)
        RETURNING *
        "#
    )
    .bind(Uuid::new_v4())
    .bind(&req.username)
    .bind(&req.email)
    .bind(&password_hash)
    .bind(default_role_id)
    .bind(Utc::now())
    .fetch_one(&state.db)
    .await?;

    if let Some(ref smtp_config) = state.config.smtp {
        if let Ok(email_service) = EmailService::new(smtp_config, &state.config.app_url) {
            let _ = email_service.send_welcome_email(&req.email, &req.username).await;
        }
    }

    let (role_name, _) = fetch_user_role(&state, user.role_id).await?;
    let permission_list = fetch_user_permissions(&state.db, user.id, user.role_id).await?;
    let permissions = serde_json::json!(permission_list.iter().map(|p| (p.clone(), true)).collect::<std::collections::HashMap<_, _>>());
    let avatar_url = UserResponse::gravatar_url(user.email.as_deref());

    Ok(Json(UserResponse {
        id: user.id,
        username: user.username,
        email: user.email,
        avatar_url,
        role_id: user.role_id,
        role_name,
        permissions,
    }))
}

pub async fn forgot_password(
    State(state): State<AppState>,
    Json(req): Json<ForgotPasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE email = $1")
        .bind(&req.email)
        .fetch_optional(&state.db)
        .await?;

    let response = serde_json::json!({
        "message": "If an account with that email exists, a password reset link has been sent."
    });

    if let Some(user) = user {
        let token = generate_reset_token();
        let expires_at = Utc::now() + Duration::hours(1);

        sqlx::query(
            r#"
            INSERT INTO password_reset_tokens (id, user_id, token, expires_at, created_at)
            VALUES ($1, $2, $3, $4, $5)
            "#
        )
        .bind(Uuid::new_v4())
        .bind(user.id)
        .bind(&token)
        .bind(expires_at)
        .bind(Utc::now())
        .execute(&state.db)
        .await?;

        if let Some(ref smtp_config) = state.config.smtp {
            if let Ok(email_service) = EmailService::new(smtp_config, &state.config.app_url) {
                let _ = email_service.send_password_reset_email(
                    &req.email,
                    &user.username,
                    &token,
                ).await;
            }
        } else {
            tracing::info!("Password reset token for {}: {}", req.email, token);
        }
    }

    Ok(Json(response))
}

pub async fn reset_password(
    State(state): State<AppState>,
    Json(req): Json<ResetPasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let token_record: Option<crate::models::PasswordResetToken> = sqlx::query_as(
        r#"
        SELECT * FROM password_reset_tokens
        WHERE token = $1 AND used = false AND expires_at > NOW()
        "#
    )
    .bind(&req.token)
    .fetch_optional(&state.db)
    .await?;

    let token_record = token_record
        .ok_or(AppError::BadRequest("Invalid or expired reset token".into()))?;

    let password_hash = hash(&req.password, state.config.bcrypt_cost)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
        .bind(&password_hash)
        .bind(token_record.user_id)
        .execute(&state.db)
        .await?;

    sqlx::query("UPDATE password_reset_tokens SET used = true WHERE id = $1")
        .bind(token_record.id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({"message": "Password has been reset successfully"})))
}

async fn fetch_user_role(
    state: &AppState,
    role_id: Option<Uuid>,
) -> Result<(Option<String>, serde_json::Value), AppError> {
    match role_id {
        Some(id) => {
            let role: Option<Role> = sqlx::query_as("SELECT * FROM roles WHERE id = $1")
                .bind(id)
                .fetch_optional(&state.db)
                .await?;

            match role {
                Some(r) => Ok((Some(r.name), r.permissions)),
                None => Ok((None, serde_json::json!({}))),
            }
        }
        None => Ok((None, serde_json::json!({}))),
    }
}

pub async fn fetch_user_permissions(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    role_id: Option<Uuid>,
) -> Result<Vec<String>, AppError> {
    let mut permissions = Vec::new();

    if let Some(rid) = role_id {
        let role_perms: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT p.name FROM permissions p
            INNER JOIN role_permissions rp ON rp.permission_id = p.id
            WHERE rp.role_id = $1
            "#
        )
        .bind(rid)
        .fetch_all(pool)
        .await?;

        permissions.extend(role_perms.into_iter().map(|(name,)| name));
    }

    let user_perms: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT p.name FROM permissions p
        INNER JOIN user_permissions up ON up.permission_id = p.id
        WHERE up.user_id = $1
        "#
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    permissions.extend(user_perms.into_iter().map(|(name,)| name));
    permissions.sort();
    permissions.dedup();

    Ok(permissions)
}

