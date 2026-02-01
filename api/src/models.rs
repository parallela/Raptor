use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::config::Config;

fn default_protocol() -> String {
    "tcp".to_string()
}

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Config,
}

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
    pub secure: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Daemon {
    pub fn base_url(&self) -> String {
        let scheme = if self.secure { "https" } else { "http" };
        format!("{}://{}:{}", scheme, self.host, self.port)
    }
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
    #[serde(default = "default_tcp")]
    pub protocol: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn default_tcp() -> String {
    "tcp".to_string()
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateContainerAllocationRequest {
    pub container_id: Uuid,
    pub allocation_id: Uuid,
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

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

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
    #[serde(default)]
    pub secure: bool,
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
    pub flake_id: Option<Uuid>,
    pub image: Option<String>,
    pub startup_script: Option<String>,
    pub stop_command: Option<String>,
    pub allocation_id: Option<Uuid>,
    #[serde(default)]
    pub additional_allocations: Vec<Uuid>,
    #[serde(default = "default_memory")]
    pub memory_limit: i64,
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
    pub user_id: Option<Uuid>,
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
    pub protocol: Option<String>,
}

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

        if let Some(all) = self.permissions.get("*") {
            if all.as_bool().unwrap_or(false) {
                return true;
            }
        }

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

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseServer {
    pub id: Uuid,
    pub daemon_id: Option<Uuid>,
    pub db_type: String,
    pub container_id: Option<String>,
    pub container_name: String,
    pub host: String,
    pub port: i32,
    pub root_password: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserDatabase {
    pub id: Uuid,
    pub user_id: Uuid,
    pub server_id: Uuid,
    pub db_type: String,
    pub db_name: String,
    pub db_user: String,
    pub db_password: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDatabaseRequest {
    pub db_type: String, // "postgresql" or "mysql"
    pub db_name: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserDatabaseResponse {
    pub id: Uuid,
    pub db_type: String,
    pub db_name: String,
    pub db_user: String,
    pub db_password: String,
    pub host: String,
    pub port: i32,
    pub status: String,
    pub connection_string: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseServerResponse {
    pub id: Uuid,
    pub db_type: String,
    pub container_id: Option<String>,
    pub container_name: String,
    pub host: String,
    pub port: i32,
    pub status: String,
    pub database_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseServerAdminResponse {
    pub id: Uuid,
    pub daemon_id: Option<Uuid>,
    pub db_type: String,
    pub container_id: Option<String>,
    pub container_name: String,
    pub host: String,
    pub port: i32,
    pub root_password: String,
    pub status: String,
    pub database_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDatabaseServerRequest {
    pub daemon_id: Uuid,
    pub db_type: String, // "postgresql", "mysql", or "redis"
    pub port: i32,
    pub container_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDatabaseServerRequest {
    pub host: Option<String>,
    pub port: Option<i32>,
    pub regenerate_password: Option<bool>,
}
