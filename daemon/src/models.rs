use std::sync::Arc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::docker::DockerManager;
use crate::ftp::FtpServerState;

pub struct AppState {
    pub docker: DockerManager,
    pub api_key: String,
    pub containers: DashMap<String, ManagedContainer>,
    pub ftp_state: Arc<FtpServerState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedContainer {
    pub name: String,
    pub docker_id: String,
    pub image: String,
    pub startup_script: Option<String>,
    /// Command to execute for graceful stop (e.g., "stop" for Minecraft)
    pub stop_command: Option<String>,
    /// Legacy single allocation (kept for backward compatibility)
    pub allocation: Option<AllocationInfo>,
    /// Multiple allocations support
    #[serde(default)]
    pub allocations: Vec<ContainerAllocation>,
    pub ports: Vec<PortMapping>,
    pub resources: ContainerResources,
    pub sftp_user: Option<String>,
    pub sftp_pass: Option<String>,
}

/// Container allocation with full details
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerAllocation {
    pub id: String,
    pub allocation_id: Option<String>,
    pub ip: String,
    pub port: i32,
    pub internal_port: i32,
    pub protocol: String,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContainerResources {
    pub memory_limit: i64,
    pub cpu_limit: f64,
    pub disk_limit: i64,
    pub swap_limit: i64,
    pub io_weight: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortMapping {
    pub host_port: i32,
    pub container_port: i32,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllocationInfo {
    pub ip: String,
    pub port: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateContainerRequest {
    pub name: String,
    pub image: String,
    pub startup_script: Option<String>,
    /// Command to execute for graceful stop (e.g., "stop" for Minecraft)
    pub stop_command: Option<String>,
    /// Legacy single allocation (kept for backward compatibility)
    pub allocation: Option<AllocationInfo>,
    /// Multiple allocations support
    #[serde(default)]
    pub allocations: Vec<ContainerAllocation>,
    #[serde(default)]
    pub ports: Vec<PortMapping>,
    #[serde(default = "default_memory")]
    pub memory_limit: i64,
    #[serde(default = "default_cpu")]
    pub cpu_limit: f64,
    #[serde(default = "default_disk")]
    pub disk_limit: i64,
    #[serde(default)]
    pub swap_limit: i64,
    #[serde(default = "default_io")]
    pub io_weight: i32,
}

fn default_memory() -> i64 { 512 }
fn default_cpu() -> f64 { 1.0 }
fn default_disk() -> i64 { 5120 }
fn default_io() -> i32 { 500 }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignAllocationRequest {
    pub container_name: String,
    pub ip: String,
    pub port: i32,
}

#[derive(Debug, Deserialize)]
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
    /// Legacy single allocation
    #[serde(default)]
    pub allocation: Option<AllocationInfo>,
    /// Multiple allocations - if provided, replaces all allocations
    #[serde(default)]
    pub allocations: Option<Vec<ContainerAllocation>>,
    #[serde(default)]
    pub ports: Option<Vec<PortMapping>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableAllocation {
    pub ip: String,
    pub ports: Vec<i32>,
}


#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemResources {
    pub total_memory: u64,
    pub available_memory: u64,
    pub cpu_cores: usize,
    pub cpu_usage: f64,
    pub total_disk: u64,
    pub available_disk: u64,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

