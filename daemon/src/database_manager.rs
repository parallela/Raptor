use bollard::container::{Config, CreateContainerOptions, StopContainerOptions};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::image::CreateImageOptions;
use bollard::Docker;
use futures_util::StreamExt;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const MAX_REDIS_DB_NUMBER: i32 = 10000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseServer {
    pub id: String,
    pub db_type: String,
    pub container_id: Option<String>,
    pub container_name: String,
    pub host: String,
    pub port: i32,
    pub root_password: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDatabaseServerRequest {
    pub id: String,
    pub db_type: String,
    pub container_name: String,
    pub host: String,
    pub port: i32,
    pub root_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserDatabaseRequest {
    pub server_id: String,
    pub db_type: String,
    pub db_name: String,
    pub db_user: String,
    pub db_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteUserDatabaseRequest {
    pub server_id: String,
    pub db_type: String,
    pub db_name: String,
    pub db_user: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordRequest {
    pub server_id: String,
    pub db_type: String,
    pub db_name: String,
    pub db_user: String,
    pub new_password: String,
}

pub struct DatabaseManager {
    servers: dashmap::DashMap<String, DatabaseServer>,
}

impl DatabaseManager {
    pub fn new() -> Self {
        Self {
            servers: dashmap::DashMap::new(),
        }
    }

    /// Get the path to the database servers state file
    fn get_state_file_path() -> PathBuf {
        let data_dir = std::env::var("DAEMON_DATA_DIR")
            .unwrap_or_else(|_| "/var/lib/raptor-daemon".to_string());
        PathBuf::from(data_dir).join("database_servers.json")
    }

    /// Load database servers state from disk
    pub async fn load_state(&self) {
        let state_file = Self::get_state_file_path();
        match tokio::fs::read_to_string(&state_file).await {
            Ok(json) => {
                match serde_json::from_str::<Vec<DatabaseServer>>(&json) {
                    Ok(servers) => {
                        for server in servers {
                            self.servers.insert(server.id.clone(), server);
                        }
                        tracing::info!("Loaded {} database servers from state", self.servers.len());
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse database servers state: {}", e);
                    }
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::error!("Failed to read database servers state: {}", e);
                }
            }
        }
    }

    /// Save database servers state to disk
    pub async fn save_state(&self) {
        let state_file = Self::get_state_file_path();
        let servers: Vec<DatabaseServer> = self.servers.iter().map(|r| r.value().clone()).collect();

        if let Some(parent) = state_file.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                tracing::error!("Failed to create state directory: {}", e);
                return;
            }
        }

        match serde_json::to_string_pretty(&servers) {
            Ok(json) => {
                if let Err(e) = tokio::fs::write(&state_file, json).await {
                    tracing::error!("Failed to save database servers state: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize database servers state: {}", e);
            }
        }
    }

    pub fn get_server(&self, id: &str) -> Option<DatabaseServer> {
        self.servers.get(id).map(|r| r.value().clone())
    }

    pub fn list_servers(&self) -> Vec<DatabaseServer> {
        self.servers.iter().map(|r| r.value().clone()).collect()
    }

    pub fn add_server(&self, server: DatabaseServer) {
        self.servers.insert(server.id.clone(), server);
    }

    pub fn update_server_status(&self, id: &str, status: &str, container_id: Option<String>) {
        if let Some(mut server) = self.servers.get_mut(id) {
            server.status = status.to_string();
            if let Some(cid) = container_id {
                server.container_id = Some(cid);
            }
        }
    }

    pub fn remove_server(&self, id: &str) -> Option<DatabaseServer> {
        self.servers.remove(id).map(|(_, v)| v)
    }
}

async fn get_docker() -> Result<Docker, String> {
    if let Ok(host) = std::env::var("DOCKER_HOST") {
        if host.starts_with("unix://") {
            Docker::connect_with_socket(&host[7..], 120, bollard::API_DEFAULT_VERSION)
                .map_err(|e| format!("Failed to connect to Docker: {}", e))
        } else {
            Docker::connect_with_local_defaults()
                .map_err(|e| format!("Failed to connect to Docker: {}", e))
        }
    } else {
        Docker::connect_with_local_defaults()
            .map_err(|e| format!("Failed to connect to Docker: {}", e))
    }
}

pub async fn create_and_start_database_container(
    server: &DatabaseServer,
) -> Result<String, String> {
    let docker = get_docker().await?;

    // First, check if container exists by ID
    if let Some(container_id) = &server.container_id {
        match docker.inspect_container(container_id, None).await {
            Ok(info) => {
                if let Some(state) = info.state {
                    if state.running.unwrap_or(false) {
                        tracing::info!("Container {} already running", container_id);
                        return Ok(container_id.clone());
                    }
                }
                // Container exists but not running - start it
                tracing::info!("Starting existing container {}", container_id);
                docker.start_container::<String>(container_id, None).await
                    .map_err(|e| format!("Failed to start container: {}", e))?;
                return Ok(container_id.clone());
            }
            Err(_) => {
                // Container ID not found, will check by name below
            }
        }
    }

    // Check if container exists by name
    match docker.inspect_container(&server.container_name, None).await {
        Ok(info) => {
            let container_id = info.id.unwrap_or_default();
            if let Some(state) = info.state {
                if state.running.unwrap_or(false) {
                    tracing::info!("Container {} already running (found by name)", server.container_name);
                    return Ok(container_id);
                }
            }
            // Container exists but not running - start it
            tracing::info!("Starting existing container {} (found by name)", server.container_name);
            docker.start_container::<String>(&container_id, None).await
                .map_err(|e| format!("Failed to start container: {}", e))?;
            return Ok(container_id);
        }
        Err(_) => {
            // Container doesn't exist by name either, will create new one
            tracing::info!("Container {} not found, creating new one", server.container_name);
        }
    }

    // Configure container based on database type
    let (image, env_vars, internal_port, cmd) = match server.db_type.as_str() {
        "postgresql" => (
            "postgres:16-alpine",
            vec![
                format!("POSTGRES_PASSWORD={}", server.root_password),
                "POSTGRES_DB=postgres".to_string(),
            ],
            5432,
            None::<Vec<String>>,
        ),
        "mysql" => (
            "mysql:8.0",
            vec![
                format!("MYSQL_ROOT_PASSWORD={}", server.root_password),
            ],
            3306,
            None,
        ),
        "redis" => (
            "redis:7-alpine",
            vec![],
            6379,
            Some(vec![
                "redis-server".to_string(),
                "--databases".to_string(),
                MAX_REDIS_DB_NUMBER.to_string(),
                "--aclfile".to_string(),
                "/data/users.acl".to_string(),
            ]),
        ),
        _ => return Err("Invalid database type".to_string()),
    };

    // Pull image
    tracing::info!("Pulling database image: {}", image);
    let mut pull_stream = docker.create_image(
        Some(CreateImageOptions {
            from_image: image,
            ..Default::default()
        }),
        None,
        None,
    );

    while let Some(result) = pull_stream.next().await {
        if let Err(e) = result {
            tracing::warn!("Image pull warning: {}", e);
        }
    }

    // Create volume directory for persistence
    let data_dir = std::env::var("DAEMON_DATA_DIR")
        .unwrap_or_else(|_| "/var/lib/raptor-daemon".to_string());
    let volume_path = format!("{}/database_volumes/{}", data_dir, server.container_name);
    tokio::fs::create_dir_all(&volume_path).await
        .map_err(|e| format!("Failed to create volume directory: {}", e))?;

    // For Redis, create ACL file that disables default user and sets up admin
    if server.db_type == "redis" {
        let acl_path = format!("{}/users.acl", volume_path);
        // Disable default user (no anonymous access) and create admin user with root password
        let acl_content = format!(
            "user default off\nuser admin on >{} ~* +@all\n",
            server.root_password
        );
        tokio::fs::write(&acl_path, acl_content).await
            .map_err(|e| format!("Failed to create Redis ACL file: {}", e))?;
    }

    let container_config = Config {
        image: Some(image.to_string()),
        env: Some(env_vars),
        cmd: cmd,
        exposed_ports: Some(
            [(format!("{}/tcp", internal_port), HashMap::new())]
                .into_iter()
                .collect(),
        ),
        host_config: Some(bollard::service::HostConfig {
            port_bindings: Some(
                [(
                    format!("{}/tcp", internal_port),
                    Some(vec![bollard::service::PortBinding {
                        host_ip: Some("0.0.0.0".to_string()),
                        host_port: Some(server.port.to_string()),
                    }]),
                )]
                .into_iter()
                .collect(),
            ),
            binds: Some(vec![format!("{}:/data", volume_path)]),
            restart_policy: Some(bollard::service::RestartPolicy {
                name: Some(bollard::service::RestartPolicyNameEnum::UNLESS_STOPPED),
                ..Default::default()
            }),
            memory: Some(1024 * 1024 * 1024), // 1GB
            ..Default::default()
        }),
        ..Default::default()
    };

    // Create container
    let container = docker
        .create_container::<String, String>(
            Some(CreateContainerOptions {
                name: server.container_name.clone(),
                platform: None,
            }),
            container_config,
        )
        .await
        .map_err(|e| format!("Failed to create container: {}", e))?;

    // Start container
    docker
        .start_container::<String>(&container.id, None)
        .await
        .map_err(|e| format!("Failed to start container: {}", e))?;

    tracing::info!("Created and started database container: {} ({})", server.container_name, container.id);

    // Wait for database to be ready
    tracing::info!("Waiting for {} to be ready...", server.db_type);
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    Ok(container.id)
}

pub async fn stop_database_container(server: &DatabaseServer) -> Result<(), String> {
    let docker = get_docker().await?;

    // Try to stop by container_id first
    if let Some(container_id) = &server.container_id {
        match docker.stop_container(container_id, Some(StopContainerOptions { t: 30 })).await {
            Ok(_) => {
                tracing::info!("Stopped container by ID: {}", container_id);
                return Ok(());
            }
            Err(e) => {
                tracing::warn!("Failed to stop by ID {}: {}, trying by name", container_id, e);
            }
        }
    }

    // Try to stop by name
    match docker.stop_container(&server.container_name, Some(StopContainerOptions { t: 30 })).await {
        Ok(_) => {
            tracing::info!("Stopped container by name: {}", server.container_name);
            Ok(())
        }
        Err(e) => {
            // Check if container is already stopped or doesn't exist
            if let Ok(info) = docker.inspect_container(&server.container_name, None).await {
                if let Some(state) = info.state {
                    if !state.running.unwrap_or(false) {
                        tracing::info!("Container {} is already stopped", server.container_name);
                        return Ok(());
                    }
                }
            }
            Err(format!("Failed to stop container: {}", e))
        }
    }
}

pub async fn delete_database_container(server: &DatabaseServer) -> Result<(), String> {
    let docker = get_docker().await?;

    // Determine which identifier to use
    let container_ref = server.container_id.as_ref()
        .map(|s| s.as_str())
        .unwrap_or(&server.container_name);

    // Stop first (ignore errors)
    let _ = docker.stop_container(container_ref, Some(StopContainerOptions { t: 10 })).await;

    // Remove
    docker
        .remove_container(container_ref, None)
        .await
        .map_err(|e| format!("Failed to remove container: {}", e))?;

    tracing::info!("Deleted container: {}", container_ref);
    Ok(())
}

pub async fn execute_db_command(
    server: &DatabaseServer,
    command: &str,
) -> Result<String, String> {
    let docker = get_docker().await?;

    // Use container_id if available, otherwise use container_name
    let container_ref = server.container_id.as_ref()
        .map(|s| s.as_str())
        .unwrap_or(&server.container_name);

    let cmd = match server.db_type.as_str() {
        "postgresql" => vec![
            "psql".to_string(),
            "-U".to_string(),
            "postgres".to_string(),
            "-c".to_string(),
            command.to_string(),
        ],
        "mysql" => vec![
            "mysql".to_string(),
            "-u".to_string(),
            "root".to_string(),
            format!("-p{}", server.root_password),
            "-e".to_string(),
            command.to_string(),
        ],
        "redis" => {
            // Authenticate as admin user to run commands
            let mut args = vec![
                "redis-cli".to_string(),
                "--user".to_string(),
                "admin".to_string(),
                "--pass".to_string(),
                server.root_password.clone(),
            ];
            for part in command.split_whitespace() {
                args.push(part.to_string());
            }
            args
        }
        _ => return Err("Invalid database type".to_string()),
    };

    let exec = docker
        .create_exec(
            container_ref,
            CreateExecOptions {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(cmd),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("Failed to create exec: {}", e))?;

    let output = docker
        .start_exec(&exec.id, None)
        .await
        .map_err(|e| format!("Failed to start exec: {}", e))?;

    let mut result = String::new();
    if let StartExecResults::Attached { mut output, .. } = output {
        while let Some(chunk) = output.next().await {
            if let Ok(log) = chunk {
                result.push_str(&log.to_string());
            }
        }
    }

    Ok(result)
}

pub async fn create_user_database(
    server: &DatabaseServer,
    db_name: &str,
    db_user: &str,
    db_password: &str,
) -> Result<(), String> {
    match server.db_type.as_str() {
        "postgresql" => {
            // Create user (quote identifier with double quotes for PostgreSQL)
            let create_user_cmd = format!(
                "CREATE USER \"{}\" WITH PASSWORD '{}';",
                db_user, db_password
            );
            execute_db_command(server, &create_user_cmd).await?;

            // Create database (quote identifiers)
            let create_db_cmd = format!(
                "CREATE DATABASE \"{}\" OWNER \"{}\";",
                db_name, db_user
            );
            execute_db_command(server, &create_db_cmd).await?;

            // Grant privileges (quote identifiers)
            let grant_cmd = format!(
                "GRANT ALL PRIVILEGES ON DATABASE \"{}\" TO \"{}\";",
                db_name, db_user
            );
            execute_db_command(server, &grant_cmd).await?;
        }
        "mysql" => {
            // Create database (backticks for MySQL)
            let create_db_cmd = format!("CREATE DATABASE `{}`;", db_name);
            execute_db_command(server, &create_db_cmd).await?;

            // Create user and grant privileges
            let create_user_cmd = format!(
                "CREATE USER '{}'@'%' IDENTIFIED BY '{}';",
                db_user, db_password
            );
            execute_db_command(server, &create_user_cmd).await?;

            let grant_cmd = format!(
                "GRANT ALL PRIVILEGES ON `{}`.* TO '{}'@'%';",
                db_name, db_user
            );
            execute_db_command(server, &grant_cmd).await?;

            execute_db_command(server, "FLUSH PRIVILEGES;").await?;
        }
        "redis" => {
            let acl_cmd = format!(
                "ACL SETUSER {} on >{} resetkeys ~{}:* +@all",
                db_user, db_password, db_name
            );
            execute_db_command(server, &acl_cmd).await?;

            // Save ACL to file
            execute_db_command(server, "ACL SAVE").await?;

            tracing::info!("Created Redis user {} with access to keys prefixed with {}:", db_user, db_name);
        }
        _ => return Err("Invalid database type".to_string()),
    }

    Ok(())
}

pub async fn delete_user_database(
    server: &DatabaseServer,
    db_name: &str,
    db_user: &str,
) -> Result<(), String> {
    match server.db_type.as_str() {
        "postgresql" => {
            let terminate_cmd = format!(
                "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}';",
                db_name
            );
            let _ = execute_db_command(server, &terminate_cmd).await;

            let drop_db_cmd = format!("DROP DATABASE IF EXISTS \"{}\";", db_name);
            let _ = execute_db_command(server, &drop_db_cmd).await;

            let drop_user_cmd = format!("DROP USER IF EXISTS \"{}\";", db_user);
            let _ = execute_db_command(server, &drop_user_cmd).await;
        }
        "mysql" => {
            let drop_db_cmd = format!("DROP DATABASE IF EXISTS `{}`;", db_name);
            let _ = execute_db_command(server, &drop_db_cmd).await;

            let drop_user_cmd = format!("DROP USER IF EXISTS '{}'@'%';", db_user);
            let _ = execute_db_command(server, &drop_user_cmd).await;
        }
        "redis" => {
            let del_user_cmd = format!("ACL DELUSER {}", db_user);
            let _ = execute_db_command(server, &del_user_cmd).await;
            let _ = execute_db_command(server, "ACL SAVE").await;
        }
        _ => {}
    }

    Ok(())
}

pub async fn reset_user_database_password(
    server: &DatabaseServer,
    db_name: &str,
    db_user: &str,
    new_password: &str,
) -> Result<(), String> {
    match server.db_type.as_str() {
        "postgresql" => {
            let cmd = format!("ALTER USER \"{}\" WITH PASSWORD '{}';", db_user, new_password);
            execute_db_command(server, &cmd).await?;
        }
        "mysql" => {
            let cmd = format!("ALTER USER '{}'@'%' IDENTIFIED BY '{}';", db_user, new_password);
            execute_db_command(server, &cmd).await?;
            execute_db_command(server, "FLUSH PRIVILEGES;").await?;
        }
        "redis" => {
            // Update password while keeping key prefix isolation
            let acl_cmd = format!(
                "ACL SETUSER {} on >{} resetkeys ~{}:* +@all",
                db_user, new_password, db_name
            );
            execute_db_command(server, &acl_cmd).await?;
            execute_db_command(server, "ACL SAVE").await?;
        }
        _ => return Err("Invalid database type".to_string()),
    }

    Ok(())
}

pub fn generate_password(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
