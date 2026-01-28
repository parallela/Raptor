use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Config,
}

// Database models
#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: Uuid,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub email: Option<String>,
    pub role_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PasswordResetToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    pub id: Uuid,
    pub name: String,
    pub permissions: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Daemon {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub api_key: String,
    pub location: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Container {
    pub id: Uuid,
    pub user_id: Uuid,
    pub daemon_id: Uuid,
    pub name: String,
    pub image: String,
    pub startup_script: Option<String>,
    pub stop_command: Option<String>,
    pub status: String,
    pub sftp_user: Option<String>,
    pub sftp_pass: Option<String>,
    pub memory_limit: Option<i64>,
    pub cpu_limit: Option<rust_decimal::Decimal>,
    pub disk_limit: Option<i64>,
    pub swap_limit: Option<i64>,
    pub io_weight: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ContainerUser {
    pub id: Uuid,
    pub container_id: Uuid,
    pub user_id: Uuid,
    pub permission_level: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ContainerUserResponse {
    pub id: Uuid,
    pub container_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub permission_level: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ContainerPort {
    pub id: Uuid,
    pub container_id: Uuid,
    pub host_port: i32,
    pub container_port: i32,
    pub protocol: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Allocation {
    pub id: Uuid,
    pub daemon_id: Uuid,
    pub ip: String,
    pub port: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct IpPool {
    pub id: Uuid,
    pub daemon_id: Uuid,
    pub ip_address: String,
    pub cidr: Option<i32>,
    pub description: Option<String>,
    pub is_primary: Option<bool>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ContainerAllocation {
    pub id: Uuid,
    pub container_id: Uuid,
    pub allocation_id: Option<Uuid>,
    pub ip: String,
    pub port: i32,
    pub internal_port: i32,
    pub protocol: String,
    pub is_primary: Option<bool>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateIpPoolRequest {
    pub daemon_id: Uuid,
    pub ip_address: String,
    pub cidr: Option<i32>,
    pub description: Option<String>,
    pub is_primary: Option<bool>,
}

fn default_protocol() -> String {
    "tcp".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateContainerAllocationRequest {
    pub container_id: Uuid,
    pub allocation_id: Uuid,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    pub is_primary: Option<bool>,
}


#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserInvite {
    pub id: Uuid,
    pub email: String,
    pub token: String,
    pub role_id: Option<Uuid>,
    pub invited_by: Uuid,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteUserRequest {
    pub email: String,
    pub role_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptInviteRequest {
    pub token: String,
    pub username: String,
    pub password: String,
}

// Request/Response DTOs
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: Option<String>,
    pub avatar_url: String,
    pub role_id: Option<Uuid>,
    pub role_name: Option<String>,
    pub permissions: serde_json::Value,
}

impl UserResponse {
    pub fn gravatar_url(email: Option<&str>) -> String {
        let email = email.unwrap_or("").trim().to_lowercase();
        let hash = md5_hash(&email);
        format!("https://www.gravatar.com/avatar/{}?d=identicon&s=200", hash)
    }
}

fn md5_hash(input: &str) -> String {
    use std::fmt::Write;
    let digest = md5::compute(input.as_bytes());
    let mut s = String::with_capacity(32);
    for byte in digest.iter() {
        write!(&mut s, "{:02x}", byte).unwrap();
    }
    s
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleResponse {
    pub id: Uuid,
    pub name: String,
    pub permissions: serde_json::Value,
}

// Registration with email
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

// Password reset requests
#[derive(Debug, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub password: String,
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDaemonRequest {
    pub name: String,
    pub host: String,
    pub port: i32,
    pub location: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortMapping {
    pub host_port: i32,
    pub container_port: i32,
    #[serde(default = "default_protocol")]
    pub protocol: String,
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateContainerRequest {
    pub daemon_id: Uuid,
    pub name: String,
    /// Flake ID - if provided, image and startup_script will be taken from the flake
    pub flake_id: Option<Uuid>,
    /// Docker image - required if flake_id is not provided
    pub image: Option<String>,
    /// Startup script - optional, will use flake's startupCommand if flake_id is provided
    pub startup_script: Option<String>,
    /// Command to execute for graceful stop (e.g., "stop" for Minecraft)
    pub stop_command: Option<String>,
    /// Primary allocation ID (will be marked as is_primary=true)
    pub allocation_id: Option<Uuid>,
    /// Additional allocation IDs (is_primary=false)
    #[serde(default)]
    pub additional_allocations: Vec<Uuid>,
    /// Docker container memory limit in MB (should be higher than server_memory for JVM overhead)
    #[serde(default = "default_memory")]
    pub memory_limit: i64,
    /// Server/JVM heap memory in MB (used for -Xmx via {{SERVER_MEMORY}})
    /// If not set, defaults to memory_limit for backward compatibility
    pub server_memory: Option<i64>,
    #[serde(default = "default_cpu")]
    pub cpu_limit: f64,
    #[serde(default = "default_disk")]
    pub disk_limit: i64,
    #[serde(default)]
    pub swap_limit: i64,
    #[serde(default = "default_io")]
    pub io_weight: i32,
    #[serde(default)]
    pub ports: Vec<PortMapping>,
    /// User ID to assign the container to (admin only, defaults to current user)
    pub user_id: Option<Uuid>,
    /// Variable values for the flake (envVariable -> value)
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,
}

fn default_memory() -> i64 { 512 }
fn default_cpu() -> f64 { 1.0 }
fn default_disk() -> i64 { 5120 }
fn default_io() -> i32 { 500 }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAllocationRequest {
    pub daemon_id: Uuid,
    pub ip: String,
    pub port: i32,
}

// JWT Claims
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,
    pub username: String,
    pub role_id: Option<Uuid>,
    pub role_name: Option<String>,
    pub permissions: serde_json::Value,
    pub exp: usize,
}

impl Claims {
    pub fn has_permission(&self, permission: &str) -> bool {
        // Check for wildcard permission
        if let Some(all) = self.permissions.get("*") {
            if all.as_bool().unwrap_or(false) {
                return true;
            }
        }
        // Check for specific permission
        if let Some(perm) = self.permissions.get(permission) {
            return perm.as_bool().unwrap_or(false);
        }
        false
    }

    pub fn is_admin(&self) -> bool {
        self.role_name.as_deref() == Some("admin") || self.has_permission("*")
    }

    pub fn is_manager(&self) -> bool {
        self.role_name.as_deref() == Some("manager") || self.is_admin()
    }
}

