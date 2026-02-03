use axum::{extract::State, Json};
use chrono::Utc;
use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;
use rand::Rng;
use bcrypt::{hash, verify};

use crate::error::{AppError, AppResult};
use crate::middleware::AuthUser;
use crate::models::AppState;

const TOTP_ISSUER: &str = "Raptor Panel";
const BACKUP_CODE_COUNT: usize = 10;
const BACKUP_CODE_LENGTH: usize = 8;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Enable2FARequest {
    pub password: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Setup2FAResponse {
    pub secret: String,
    pub qr_code: String,
    pub otpauth_url: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Verify2FARequest {
    pub code: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verify2FAResponse {
    pub success: bool,
    pub backup_codes: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Disable2FARequest {
    pub password: String,
    pub code: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TwoFactorStatusResponse {
    pub enabled: bool,
    pub verified_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Validate2FALoginRequest {
    pub user_id: String,
    pub code: String,
    pub is_backup_code: Option<bool>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Validate2FALoginResponse {
    pub valid: bool,
    pub token: Option<String>,
    pub user: Option<crate::models::UserResponse>,
}

fn generate_totp_secret() -> String {
    let secret = Secret::generate_secret();
    secret.to_encoded().to_string()
}

fn create_totp(secret: &str, username: &str) -> Result<TOTP, AppError> {
    let secret_bytes = Secret::Encoded(secret.to_string())
        .to_bytes()
        .map_err(|e| AppError::Internal(format!("Invalid TOTP secret: {}", e)))?;

    TOTP::new(
        Algorithm::SHA1,
        6,
        3,  // Allow 3 steps tolerance (90 seconds before/after) for clock drift
        30,
        secret_bytes,
        Some(TOTP_ISSUER.to_string()),
        username.to_string(),
    )
    .map_err(|e| AppError::Internal(format!("Failed to create TOTP: {}", e)))
}

fn generate_backup_codes() -> Vec<String> {
    let mut rng = rand::thread_rng();
    (0..BACKUP_CODE_COUNT)
        .map(|_| {
            (0..BACKUP_CODE_LENGTH)
                .map(|_| {
                    let idx = rng.gen_range(0..36);
                    if idx < 10 {
                        (b'0' + idx) as char
                    } else {
                        (b'A' + idx - 10) as char
                    }
                })
                .collect::<String>()
        })
        .map(|code| format!("{}-{}", &code[..4], &code[4..]))
        .collect()
}

pub async fn get_2fa_status(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> AppResult<Json<TwoFactorStatusResponse>> {
    let result: Option<(bool, Option<chrono::DateTime<Utc>>)> = sqlx::query_as(
        "SELECT totp_enabled, totp_verified_at FROM users WHERE id = $1"
    )
    .bind(auth_user.id)
    .fetch_optional(&state.db)
    .await?;

    match result {
        Some((enabled, verified_at)) => Ok(Json(TwoFactorStatusResponse {
            enabled,
            verified_at,
        })),
        None => Err(AppError::NotFound),
    }
}

pub async fn setup_2fa(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(req): Json<Enable2FARequest>,
) -> AppResult<Json<Setup2FAResponse>> {
    let user: Option<(String, bool)> = sqlx::query_as(
        "SELECT password_hash, totp_enabled FROM users WHERE id = $1"
    )
    .bind(auth_user.id)
    .fetch_optional(&state.db)
    .await?;

    let (password_hash, totp_enabled) = user.ok_or(AppError::NotFound)?;

    if totp_enabled {
        return Err(AppError::BadRequest("2FA is already enabled".into()));
    }

    if !verify(&req.password, &password_hash).unwrap_or(false) {
        return Err(AppError::BadRequest("Invalid password".into()));
    }

    let secret = generate_totp_secret();
    let totp = create_totp(&secret, &auth_user.username)?;

    let qr_code = totp
        .get_qr_base64()
        .map_err(|e| AppError::Internal(format!("Failed to generate QR code: {}", e)))?;

    // Get the base otpauth URL and add image parameter for authenticator apps that support it
    let base_url = totp.get_url();
    let icon_url = format!("{}/favicon.png", state.config.app_url);
    let otpauth_url = format!("{}&image={}", base_url, urlencoding::encode(&icon_url));

    // Store secret (not yet enabled)
    sqlx::query("UPDATE users SET totp_secret = $1, updated_at = NOW() WHERE id = $2")
        .bind(&secret)
        .bind(auth_user.id)
        .execute(&state.db)
        .await?;

    Ok(Json(Setup2FAResponse {
        secret,
        qr_code: format!("data:image/png;base64,{}", qr_code),
        otpauth_url,
    }))
}

pub async fn verify_2fa(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(req): Json<Verify2FARequest>,
) -> AppResult<Json<Verify2FAResponse>> {
    // Get the stored secret
    let user: Option<(Option<String>, bool)> = sqlx::query_as(
        "SELECT totp_secret, totp_enabled FROM users WHERE id = $1"
    )
    .bind(auth_user.id)
    .fetch_optional(&state.db)
    .await?;

    let (secret, totp_enabled) = user.ok_or(AppError::NotFound)?;

    if totp_enabled {
        return Err(AppError::BadRequest("2FA is already enabled".into()));
    }

    let secret = secret.ok_or(AppError::BadRequest("2FA setup not started".into()))?;

    // Clean the code - remove any spaces or dashes and trim
    let clean_code = req.code.trim().replace(" ", "").replace("-", "");

    // Verify the code
    let totp = create_totp(&secret, &auth_user.username)?;

    tracing::info!("Verifying 2FA code for user {}: code='{}', length={}",
        auth_user.username, clean_code, clean_code.len());

    // Generate what the current code should be for debugging
    let expected_code = totp.generate_current()
        .map_err(|e| AppError::Internal(format!("Failed to generate TOTP: {}", e)))?;
    tracing::info!("Expected TOTP code: {}", expected_code);

    // Use check_current which handles time window automatically
    let is_valid = match totp.check_current(&clean_code) {
        Ok(valid) => valid,
        Err(e) => {
            tracing::error!("TOTP check error: {}", e);
            false
        }
    };

    tracing::info!("TOTP validation result: {}", is_valid);

    if !is_valid {
        tracing::warn!("Invalid 2FA code for user {}. Got: {}, Expected: {}",
            auth_user.username, clean_code, expected_code);
        return Ok(Json(Verify2FAResponse {
            success: false,
            backup_codes: None,
        }));
    }

    tracing::info!("2FA verification successful for user {}", auth_user.username);

    // Generate backup codes
    let backup_codes = generate_backup_codes();

    // Hash backup codes before storing
    let mut tx = state.db.begin().await?;

    // Enable 2FA
    sqlx::query(
        "UPDATE users SET totp_enabled = TRUE, totp_verified_at = NOW(), updated_at = NOW() WHERE id = $1"
    )
    .bind(auth_user.id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM totp_backup_codes WHERE user_id = $1")
        .bind(auth_user.id)
        .execute(&mut *tx)
        .await?;

    for code in &backup_codes {
        let code_hash = hash(code.replace("-", "").as_str(), 4)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        sqlx::query(
            "INSERT INTO totp_backup_codes (id, user_id, code_hash, created_at) VALUES ($1, $2, $3, NOW())"
        )
        .bind(Uuid::new_v4())
        .bind(auth_user.id)
        .bind(&code_hash)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(Json(Verify2FAResponse {
        success: true,
        backup_codes: Some(backup_codes),
    }))
}

pub async fn disable_2fa(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(req): Json<Disable2FARequest>,
) -> AppResult<Json<serde_json::Value>> {
    let user: Option<(String, Option<String>, bool)> = sqlx::query_as(
        "SELECT password_hash, totp_secret, totp_enabled FROM users WHERE id = $1"
    )
    .bind(auth_user.id)
    .fetch_optional(&state.db)
    .await?;

    let (password_hash, secret, totp_enabled) = user.ok_or(AppError::NotFound)?;

    if !totp_enabled {
        return Err(AppError::BadRequest("2FA is not enabled".into()));
    }

    if !verify(&req.password, &password_hash).unwrap_or(false) {
        return Err(AppError::BadRequest("Invalid password".into()));
    }

    let secret = secret.ok_or(AppError::Internal("TOTP secret not found".into()))?;
    let totp = create_totp(&secret, &auth_user.username)?;

    if !totp.check_current(&req.code).unwrap_or(false) {
        return Err(AppError::BadRequest("Invalid 2FA code".into()));
    }

    let mut tx = state.db.begin().await?;

    sqlx::query(
        "UPDATE users SET totp_enabled = FALSE, totp_secret = NULL, totp_verified_at = NULL, updated_at = NOW() WHERE id = $1"
    )
    .bind(auth_user.id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM totp_backup_codes WHERE user_id = $1")
        .bind(auth_user.id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(Json(serde_json::json!({ "message": "2FA disabled successfully" })))
}

pub async fn regenerate_backup_codes(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(req): Json<Verify2FARequest>,
) -> AppResult<Json<Vec<String>>> {
    // Verify the user has 2FA enabled and the code is valid
    let user: Option<(Option<String>, bool)> = sqlx::query_as(
        "SELECT totp_secret, totp_enabled FROM users WHERE id = $1"
    )
    .bind(auth_user.id)
    .fetch_optional(&state.db)
    .await?;

    let (secret, totp_enabled) = user.ok_or(AppError::NotFound)?;

    if !totp_enabled {
        return Err(AppError::BadRequest("2FA is not enabled".into()));
    }

    let secret = secret.ok_or(AppError::Internal("TOTP secret not found".into()))?;
    let totp = create_totp(&secret, &auth_user.username)?;

    if !totp.check_current(&req.code).unwrap_or(false) {
        return Err(AppError::BadRequest("Invalid 2FA code".into()));
    }

    // Generate new backup codes
    let backup_codes = generate_backup_codes();

    let mut tx = state.db.begin().await?;

    sqlx::query("DELETE FROM totp_backup_codes WHERE user_id = $1")
        .bind(auth_user.id)
        .execute(&mut *tx)
        .await?;

    for code in &backup_codes {
        let code_hash = hash(code.replace("-", "").as_str(), 4)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        sqlx::query(
            "INSERT INTO totp_backup_codes (id, user_id, code_hash, created_at) VALUES ($1, $2, $3, NOW())"
        )
        .bind(Uuid::new_v4())
        .bind(auth_user.id)
        .bind(&code_hash)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(Json(backup_codes))
}

pub async fn validate_2fa_login(
    State(state): State<AppState>,
    Json(req): Json<Validate2FALoginRequest>,
) -> AppResult<Json<Validate2FALoginResponse>> {
    let user_id: Uuid = req.user_id.parse()
        .map_err(|_| AppError::BadRequest("Invalid user ID".into()))?;

    let user: Option<(String, Option<String>, bool, Option<String>, Option<Uuid>)> = sqlx::query_as(
        "SELECT username, totp_secret, totp_enabled, email, role_id FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    let (username, secret, totp_enabled, email, role_id) = user.ok_or(AppError::NotFound)?;

    if !totp_enabled {
        return Err(AppError::BadRequest("2FA is not enabled for this user".into()));
    }

    let secret = secret.ok_or(AppError::Internal("TOTP secret not found".into()))?;

    let is_valid = if req.is_backup_code.unwrap_or(false) {
        // Validate backup code
        let backup_codes: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT id, code_hash FROM totp_backup_codes WHERE user_id = $1 AND used = FALSE"
        )
        .bind(user_id)
        .fetch_all(&state.db)
        .await?;

        let code_clean = req.code.replace("-", "");
        let mut matched_code_id: Option<Uuid> = None;

        for (id, code_hash) in backup_codes {
            if verify(&code_clean, &code_hash).unwrap_or(false) {
                matched_code_id = Some(id);
                break;
            }
        }

        if let Some(code_id) = matched_code_id {
            sqlx::query("UPDATE totp_backup_codes SET used = TRUE, used_at = NOW() WHERE id = $1")
                .bind(code_id)
                .execute(&state.db)
                .await?;
            true
        } else {
            false
        }
    } else {
        let totp = create_totp(&secret, &username)?;
        totp.check_current(&req.code).unwrap_or(false)
    };

    // Log attempt
    sqlx::query(
        "INSERT INTO totp_attempts (id, user_id, success, created_at) VALUES ($1, $2, $3, NOW())"
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(is_valid)
    .execute(&state.db)
    .await?;

    if !is_valid {
        return Ok(Json(Validate2FALoginResponse {
            valid: false,
            token: None,
            user: None,
        }));
    }

    let (role_name, _) = crate::handlers::auth::fetch_user_role(&state, role_id).await?;
    let permission_list = crate::handlers::auth::fetch_user_permissions(&state.db, user_id, role_id).await?;
    let permissions = serde_json::json!(permission_list.iter().map(|p| (p.clone(), true)).collect::<std::collections::HashMap<_, _>>());

    let exp = (Utc::now().timestamp() + 86400 * state.config.jwt_expiry_days) as usize;
    let claims = crate::models::Claims {
        sub: user_id,
        username: username.clone(),
        role_id,
        role_name: role_name.clone(),
        permissions: permissions.clone(),
        exp,
    };

    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let avatar_url = crate::models::UserResponse::gravatar_url(email.as_deref());

    Ok(Json(Validate2FALoginResponse {
        valid: true,
        token: Some(token),
        user: Some(crate::models::UserResponse {
            id: user_id,
            username,
            email,
            avatar_url,
            role_id,
            role_name,
            permissions,
        }),
    }))
}

pub async fn check_2fa_required(
    db: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result: Option<(bool,)> = sqlx::query_as(
        "SELECT totp_enabled FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;

    Ok(result.map(|(enabled,)| enabled).unwrap_or(false))
}
