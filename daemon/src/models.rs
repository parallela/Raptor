use std::sync::Arc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::database_manager::DatabaseManager;
use crate::docker::DockerManager;
use crate::ftp::FtpServerState;

pub struct ContainerLocks {
    locks: DashMap<String, Arc<Mutex<()>>>,
}

impl ContainerLocks {
    pub fn new() -> Self {
        Self {
            locks: DashMap::new(),
        }
    }

    pub fn get_lock(&self, container_id: &str) -> Arc<Mutex<()>> {
        self.locks
            .entry(container_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    pub fn remove_lock(&self, container_id: &str) {
        self.locks.remove(container_id);
    }
}

impl Default for ContainerLocks {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AppState {
    pub docker: DockerManager,
    pub api_key: String,
    pub containers: DashMap<String, ManagedContainer>,
    pub ftp_state: Arc<FtpServerState>,
    pub container_locks: ContainerLocks,
    pub database_manager: DatabaseManager,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedContainer {
    pub name: String,
    pub docker_id: String,
    pub image: String,
    pub startup_script: Option<String>,
    pub stop_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allocation: Option<AllocationInfo>,
    #[serde(default)]
    pub allocations: Vec<ContainerAllocation>,
    pub resources: ContainerResources,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_script: Option<String>,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub environment: std::collections::HashMap<String, String>,
    #[serde(default = "default_restart_policy")]
    pub restart_policy: String,
    #[serde(default)]
    pub tty: bool,
}

fn default_restart_policy() -> String {
    "unless-stopped".to_string()
}

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
    #[serde(default)]
    pub server_memory: i64,
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
    pub stop_command: Option<String>,
    pub allocation: Option<AllocationInfo>,
    #[serde(default)]
    pub allocations: Vec<ContainerAllocation>,
    #[serde(default)]
    pub ports: Vec<PortMapping>,
    #[serde(default = "default_memory")]
    pub memory_limit: i64,
    #[serde(default)]
    pub server_memory: Option<i64>,
    #[serde(default = "default_cpu")]
    pub cpu_limit: f64,
    #[serde(default = "default_disk")]
    pub disk_limit: i64,
    #[serde(default)]
    pub swap_limit: i64,
    #[serde(default = "default_io")]
    pub io_weight: i32,
    pub install_script: Option<String>,
    #[serde(default)]
    pub environment: std::collections::HashMap<String, String>,
    #[serde(default = "default_restart_policy")]
    pub restart_policy: String,
    #[serde(default)]
    pub tty: bool,
}

fn default_memory() -> i64 { 512 }
fn default_cpu() -> f64 { 1.0 }
fn default_disk() -> i64 { 5120 }
fn default_io() -> i32 { 500 }

fn deserialize_string_or_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct StringOrF64Visitor;

    impl<'de> Visitor<'de> for StringOrF64Visitor {
        type Value = Option<f64>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or a number")
        }

        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v))
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v as f64))
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v as f64))
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {

            let normalized = v.replace(',', ".");
            normalized.parse::<f64>().map(Some).map_err(de::Error::custom)
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_any(StringOrF64Visitor)
}

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
    pub server_memory: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_string_or_f64")]
    pub cpu_limit: Option<f64>,
    #[serde(default)]
    pub disk_limit: Option<i64>,
    #[serde(default)]
    pub swap_limit: Option<i64>,
    #[serde(default)]
    pub io_weight: Option<i32>,
    #[serde(default)]
    pub allocation: Option<AllocationInfo>,
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

