use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use bollard::{
    container::{Config, CreateContainerOptions, StopContainerOptions},
    Docker,
    image::CreateImageOptions,
    exec::{CreateExecOptions, StartExecResults},
};
use futures_util::StreamExt;
use rand::Rng;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    AppState, Claims, CreateDatabaseRequest, CreateDatabaseServerRequest,
    DatabaseServer, DatabaseServerAdminResponse, DatabaseServerResponse,
    UpdateDatabaseServerRequest, UserDatabase, UserDatabaseResponse
};

const MAX_REDIS_DB_NUMBER: i32 = 10000;

fn generate_password(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

fn generate_db_identifier(user_id: &Uuid, suffix: &str) -> String {
    let id_str = user_id.to_string().replace("-", "");
    format!("u{}_{}", &id_str[..8], suffix)
}

async fn get_docker() -> Result<Docker, AppError> {
    Docker::connect_with_local_defaults()
        .map_err(|e| AppError::Internal(format!("Failed to connect to Docker: {}", e)))
}

async fn ensure_database_server_running(
    state: &AppState,
    server: &DatabaseServer,
) -> Result<DatabaseServer, AppError> {
    let docker = get_docker().await?;

    // Check if container exists and is running
    if let Some(container_id) = &server.container_id {
        match docker.inspect_container(container_id, None).await {
            Ok(info) => {
                if let Some(state_info) = info.state {
                    if state_info.running.unwrap_or(false) {
                        // Already running - return a clone
                        return Ok(DatabaseServer {
                            id: server.id,
                            db_type: server.db_type.clone(),
                            container_id: server.container_id.clone(),
                            container_name: server.container_name.clone(),
                            host: server.host.clone(),
                            port: server.port,
                            root_password: server.root_password.clone(),
                            status: server.status.clone(),
                            created_at: server.created_at,
                            updated_at: server.updated_at,
                        });
                    }
                }
                // Container exists but not running - start it
                docker
                    .start_container::<String>(container_id, None)
                    .await
                    .map_err(|e| AppError::Internal(format!("Failed to start container: {}", e)))?;

                // Update status in database
                let updated = sqlx::query_as::<_, DatabaseServer>(
                    "UPDATE database_servers SET status = 'running', updated_at = NOW() WHERE id = $1 RETURNING *"
                )
                .bind(server.id)
                .fetch_one(&state.db)
                .await?;

                return Ok(updated);
            }
            Err(_) => {
                // Container doesn't exist, we need to create it
            }
        }
    }

    // Create the container
    let (image, env_vars, internal_port, cmd) = match server.db_type.as_str() {
        "postgresql" => (
            "postgres:16-alpine",
            vec![
                format!("POSTGRES_PASSWORD={}", server.root_password),
                "POSTGRES_DB=postgres".to_string(),
            ],
            5432,
            None,
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
        _ => return Err(AppError::BadRequest("Invalid database type".to_string())),
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

    let container_config = Config {
        image: Some(image.to_string()),
        env: Some(env_vars),
        cmd: cmd,
        exposed_ports: Some(
            [(format!("{}/tcp", internal_port), std::collections::HashMap::new())]
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
            restart_policy: Some(bollard::service::RestartPolicy {
                name: Some(bollard::service::RestartPolicyNameEnum::UNLESS_STOPPED),
                ..Default::default()
            }),
            memory: Some(1024 * 1024 * 1024),
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
        .map_err(|e| AppError::Internal(format!("Failed to create container: {}", e)))?;

    // Start container
    docker
        .start_container::<String>(&container.id, None)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to start container: {}", e)))?;

    // Update database with container ID
    let updated = sqlx::query_as::<_, DatabaseServer>(
        "UPDATE database_servers SET container_id = $1, status = 'running', updated_at = NOW() WHERE id = $2 RETURNING *"
    )
    .bind(&container.id)
    .bind(server.id)
    .fetch_one(&state.db)
    .await?;

    // Wait for database to be ready
    tracing::info!("Waiting for {} to be ready...", server.db_type);
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    Ok(updated)
}

async fn execute_db_command(
    docker: &Docker,
    container_id: &str,
    db_type: &str,
    root_password: &str,
    command: &str,
) -> Result<String, AppError> {
    let cmd = match db_type {
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
            format!("-p{}", root_password),
            "-e".to_string(),
            command.to_string(),
        ],
        "redis" => {
            let mut args = vec!["redis-cli".to_string()];
            for part in command.split_whitespace() {
                args.push(part.to_string());
            }
            args
        }
        _ => return Err(AppError::BadRequest("Invalid database type".to_string())),
    };

    let exec = docker
        .create_exec(
            container_id,
            CreateExecOptions {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(cmd),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create exec: {}", e)))?;

    let output = docker
        .start_exec(&exec.id, None)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to start exec: {}", e)))?;

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

pub async fn list_databases(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<UserDatabaseResponse>>, AppError> {
    let databases = sqlx::query_as::<_, UserDatabase>(
        "SELECT * FROM user_databases WHERE user_id = $1 ORDER BY created_at DESC"
    )
    .bind(claims.sub)
    .fetch_all(&state.db)
    .await?;

    let mut responses = Vec::new();
    for db in databases {
        // Get server info
        let server = sqlx::query_as::<_, DatabaseServer>(
            "SELECT * FROM database_servers WHERE id = $1"
        )
        .bind(db.server_id)
        .fetch_optional(&state.db)
        .await?;

        if let Some(server) = server {
            let connection_string = match db.db_type.as_str() {
                "postgresql" => format!(
                    "postgresql://{}:{}@{}:{}/{}",
                    db.db_user, db.db_password, server.host, server.port, db.db_name
                ),
                "mysql" => format!(
                    "mysql://{}:{}@{}:{}/{}",
                    db.db_user, db.db_password, server.host, server.port, db.db_name
                ),
                "redis" => format!(
                    "redis://{}:{}@{}:{}/{}",
                    db.db_user, db.db_password, server.host, server.port, db.db_name
                ),
                _ => String::new(),
            };

            responses.push(UserDatabaseResponse {
                id: db.id,
                db_type: db.db_type,
                db_name: db.db_name,
                db_user: db.db_user,
                db_password: db.db_password,
                host: server.host,
                port: server.port,
                status: db.status,
                connection_string,
                created_at: db.created_at,
            });
        }
    }

    Ok(Json(responses))
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableDatabaseType {
    pub db_type: String,
    pub name: String,
    pub available: bool,
}

pub async fn get_available_database_types(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<AvailableDatabaseType>>, AppError> {
    let servers = sqlx::query_as::<_, DatabaseServer>(
        "SELECT * FROM database_servers WHERE status = 'running' ORDER BY db_type"
    )
    .fetch_all(&state.db)
    .await?;

    let existing_dbs: Vec<String> = sqlx::query_scalar(
        "SELECT db_type FROM user_databases WHERE user_id = $1"
    )
    .bind(claims.sub)
    .fetch_all(&state.db)
    .await?;

    let mut types = Vec::new();
    for server in servers {
        let name = match server.db_type.as_str() {
            "postgresql" => "PostgreSQL",
            "mysql" => "MySQL",
            "redis" => "Redis",
            _ => &server.db_type,
        };
        types.push(AvailableDatabaseType {
            db_type: server.db_type.clone(),
            name: name.to_string(),
            available: !existing_dbs.contains(&server.db_type),
        });
    }

    Ok(Json(types))
}

pub async fn get_database(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<UserDatabaseResponse>, AppError> {
    let db = sqlx::query_as::<_, UserDatabase>(
        "SELECT * FROM user_databases WHERE id = $1 AND user_id = $2"
    )
    .bind(id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let server = sqlx::query_as::<_, DatabaseServer>(
        "SELECT * FROM database_servers WHERE id = $1"
    )
    .bind(db.server_id)
    .fetch_one(&state.db)
    .await?;

    let connection_string = match db.db_type.as_str() {
        "postgresql" => format!(
            "postgresql://{}:{}@{}:{}/{}",
            db.db_user, db.db_password, server.host, server.port, db.db_name
        ),
        "mysql" => format!(
            "mysql://{}:{}@{}:{}/{}",
            db.db_user, db.db_password, server.host, server.port, db.db_name
        ),
        "redis" => format!(
            "redis://{}:{}@{}:{}/{}",
            db.db_user, db.db_password, server.host, server.port, db.db_name
        ),
        _ => String::new(),
    };

    Ok(Json(UserDatabaseResponse {
        id: db.id,
        db_type: db.db_type,
        db_name: db.db_name,
        db_user: db.db_user,
        db_password: db.db_password,
        host: server.host,
        port: server.port,
        status: db.status,
        connection_string,
        created_at: db.created_at,
    }))
}

pub async fn create_database(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateDatabaseRequest>,
) -> Result<Json<UserDatabaseResponse>, AppError> {
    // Validate db_type
    if req.db_type != "postgresql" && req.db_type != "mysql" && req.db_type != "redis" {
        return Err(AppError::BadRequest("Invalid database type. Use 'postgresql', 'mysql', or 'redis'".to_string()));
    }

    // Check if user already has a database of this type
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM user_databases WHERE user_id = $1 AND db_type = $2"
    )
    .bind(claims.sub)
    .bind(&req.db_type)
    .fetch_one(&state.db)
    .await?;

    if existing > 0 {
        return Err(AppError::BadRequest(format!("You already have a {} database", req.db_type)));
    }

    // Get the shared database server for this type
    let server = sqlx::query_as::<_, DatabaseServer>(
        "SELECT * FROM database_servers WHERE db_type = $1"
    )
    .bind(&req.db_type)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Internal(format!("No {} server configured", req.db_type)))?;

    // Ensure the shared container is running
    let server = ensure_database_server_running(&state, &server).await?;

    let container_id = server.container_id.as_ref()
        .ok_or_else(|| AppError::Internal("Server container not available".to_string()))?;

    let (db_name, db_user, db_password) = if req.db_type == "redis" {
        let used_numbers: Vec<String> = sqlx::query_scalar(
            "SELECT db_name FROM user_databases WHERE server_id = $1 AND db_type = 'redis'"
        )
        .bind(server.id)
        .fetch_all(&state.db)
        .await?;

        let used_nums: Vec<i32> = used_numbers
            .iter()
            .filter_map(|n| n.parse::<i32>().ok())
            .collect();

        let mut next_db = 0;
        for i in 0..MAX_REDIS_DB_NUMBER {
            if !used_nums.contains(&i) {
                next_db = i;
                break;
            }
        }

        if used_nums.len() >= MAX_REDIS_DB_NUMBER as usize {
            return Err(AppError::BadRequest("All Redis database slots are in use".to_string()));
        }

        let db_user = generate_db_identifier(&claims.sub, "user");
        let db_password = generate_password(24);
        (next_db.to_string(), db_user, db_password)
    } else {
        let db_name = req.db_name.unwrap_or_else(|| generate_db_identifier(&claims.sub, "db"));
        let db_user = generate_db_identifier(&claims.sub, "user");
        let db_password = generate_password(24);
        (db_name, db_user, db_password)
    };

    let docker = get_docker().await?;

    // Create database and user within the shared container
    match req.db_type.as_str() {
        "postgresql" => {
            // Create user
            let create_user_cmd = format!(
                "CREATE USER {} WITH PASSWORD '{}';",
                db_user, db_password
            );
            execute_db_command(&docker, container_id, "postgresql", &server.root_password, &create_user_cmd).await?;

            // Create database
            let create_db_cmd = format!(
                "CREATE DATABASE {} OWNER {};",
                db_name, db_user
            );
            execute_db_command(&docker, container_id, "postgresql", &server.root_password, &create_db_cmd).await?;

            // Grant privileges
            let grant_cmd = format!(
                "GRANT ALL PRIVILEGES ON DATABASE {} TO {};",
                db_name, db_user
            );
            execute_db_command(&docker, container_id, "postgresql", &server.root_password, &grant_cmd).await?;
        }
        "mysql" => {
            // Create database
            let create_db_cmd = format!("CREATE DATABASE {};", db_name);
            execute_db_command(&docker, container_id, "mysql", &server.root_password, &create_db_cmd).await?;

            // Create user and grant privileges
            let create_user_cmd = format!(
                "CREATE USER '{}'@'%' IDENTIFIED BY '{}';",
                db_user, db_password
            );
            execute_db_command(&docker, container_id, "mysql", &server.root_password, &create_user_cmd).await?;

            let grant_cmd = format!(
                "GRANT ALL PRIVILEGES ON {}.* TO '{}'@'%';",
                db_name, db_user
            );
            execute_db_command(&docker, container_id, "mysql", &server.root_password, &grant_cmd).await?;

            let flush_cmd = "FLUSH PRIVILEGES;";
            execute_db_command(&docker, container_id, "mysql", &server.root_password, flush_cmd).await?;
        }
        "redis" => {
            // Create ACL user locked to specific database
            // Format: ACL SETUSER username on >password ~* +@all -select +select|N
            let acl_cmd = format!(
                "ACL SETUSER {} on >{} ~* +@all -select +select|{}",
                db_user, db_password, db_name
            );
            execute_db_command(&docker, container_id, "redis", &server.root_password, &acl_cmd).await?;

            // Save ACL to file
            execute_db_command(&docker, container_id, "redis", &server.root_password, "ACL SAVE").await?;

            tracing::info!("Created Redis user {} with access to DB {} for user {}", db_user, db_name, claims.sub);
        }
        _ => return Err(AppError::BadRequest("Invalid database type".to_string())),
    }

    // Save to database
    let db_record = sqlx::query_as::<_, UserDatabase>(
        r#"
        INSERT INTO user_databases (user_id, server_id, db_type, db_name, db_user, db_password, status)
        VALUES ($1, $2, $3, $4, $5, $6, 'active')
        RETURNING *
        "#
    )
    .bind(claims.sub)
    .bind(server.id)
    .bind(&req.db_type)
    .bind(&db_name)
    .bind(&db_user)
    .bind(&db_password)
    .fetch_one(&state.db)
    .await?;

    let connection_string = match req.db_type.as_str() {
        "postgresql" => format!(
            "postgresql://{}:{}@{}:{}/{}",
            db_record.db_user, db_record.db_password, server.host, server.port, db_record.db_name
        ),
        "mysql" => format!(
            "mysql://{}:{}@{}:{}/{}",
            db_record.db_user, db_record.db_password, server.host, server.port, db_record.db_name
        ),
        "redis" => format!(
            "redis://{}:{}@{}:{}/{}",
            db_record.db_user, db_record.db_password, server.host, server.port, db_record.db_name
        ),
        _ => String::new(),
    };

    tracing::info!("Created {} database '{}' for user {}", req.db_type, db_name, claims.sub);

    Ok(Json(UserDatabaseResponse {
        id: db_record.id,
        db_type: db_record.db_type,
        db_name: db_record.db_name,
        db_user: db_record.db_user,
        db_password: db_record.db_password,
        host: server.host,
        port: server.port,
        status: db_record.status,
        connection_string,
        created_at: db_record.created_at,
    }))
}

pub async fn delete_database(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    // Get the database record
    let db = sqlx::query_as::<_, UserDatabase>(
        "SELECT * FROM user_databases WHERE id = $1 AND user_id = $2"
    )
    .bind(id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    // Get the server
    let server = sqlx::query_as::<_, DatabaseServer>(
        "SELECT * FROM database_servers WHERE id = $1"
    )
    .bind(db.server_id)
    .fetch_one(&state.db)
    .await?;

    if let Some(container_id) = &server.container_id {
        let docker = get_docker().await?;

        match db.db_type.as_str() {
            "postgresql" => {
                let terminate_cmd = format!(
                    "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}';",
                    db.db_name
                );
                let _ = execute_db_command(&docker, container_id, "postgresql", &server.root_password, &terminate_cmd).await;

                let drop_db_cmd = format!("DROP DATABASE IF EXISTS {};", db.db_name);
                let _ = execute_db_command(&docker, container_id, "postgresql", &server.root_password, &drop_db_cmd).await;

                let drop_user_cmd = format!("DROP USER IF EXISTS {};", db.db_user);
                let _ = execute_db_command(&docker, container_id, "postgresql", &server.root_password, &drop_user_cmd).await;
            }
            "mysql" => {
                let drop_db_cmd = format!("DROP DATABASE IF EXISTS {};", db.db_name);
                let _ = execute_db_command(&docker, container_id, "mysql", &server.root_password, &drop_db_cmd).await;

                let drop_user_cmd = format!("DROP USER IF EXISTS '{}'@'%';", db.db_user);
                let _ = execute_db_command(&docker, container_id, "mysql", &server.root_password, &drop_user_cmd).await;
            }
            "redis" => {
                let del_user_cmd = format!("ACL DELUSER {}", db.db_user);
                let _ = execute_db_command(&docker, container_id, "redis", &server.root_password, &del_user_cmd).await;
                let _ = execute_db_command(&docker, container_id, "redis", &server.root_password, "ACL SAVE").await;
            }
            _ => {}
        }
    }

    // Delete from database
    sqlx::query("DELETE FROM user_databases WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    tracing::info!("Deleted {} database '{}' for user {}", db.db_type, db.db_name, claims.sub);

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_database_servers(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<DatabaseServerResponse>>, AppError> {
    if !claims.is_admin() {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let servers = sqlx::query_as::<_, DatabaseServer>(
        "SELECT * FROM database_servers ORDER BY db_type"
    )
    .fetch_all(&state.db)
    .await?;

    let mut responses = Vec::new();
    for server in servers {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_databases WHERE server_id = $1"
        )
        .bind(server.id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

        responses.push(DatabaseServerResponse {
            id: server.id,
            db_type: server.db_type,
            container_id: server.container_id,
            container_name: server.container_name,
            host: server.host,
            port: server.port,
            status: server.status,
            database_count: count,
            created_at: server.created_at,
            updated_at: server.updated_at,
        });
    }

    Ok(Json(responses))
}

pub async fn start_database_server(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<DatabaseServerResponse>, AppError> {
    if !claims.is_admin() {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let server = sqlx::query_as::<_, DatabaseServer>(
        "SELECT * FROM database_servers WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let updated = ensure_database_server_running(&state, &server).await?;

    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM user_databases WHERE server_id = $1"
    )
    .bind(updated.id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    Ok(Json(DatabaseServerResponse {
        id: updated.id,
        db_type: updated.db_type,
        container_id: updated.container_id,
        container_name: updated.container_name,
        host: updated.host,
        port: updated.port,
        status: updated.status,
        database_count: count,
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    }))
}

pub async fn stop_database_server(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<DatabaseServerResponse>, AppError> {
    if !claims.is_admin() {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let server = sqlx::query_as::<_, DatabaseServer>(
        "SELECT * FROM database_servers WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    if let Some(container_id) = &server.container_id {
        let docker = get_docker().await?;
        let _ = docker
            .stop_container(container_id, Some(StopContainerOptions { t: 30 }))
            .await;
    }

    let updated = sqlx::query_as::<_, DatabaseServer>(
        "UPDATE database_servers SET status = 'stopped', updated_at = NOW() WHERE id = $1 RETURNING *"
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM user_databases WHERE server_id = $1"
    )
    .bind(updated.id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    Ok(Json(DatabaseServerResponse {
        id: updated.id,
        db_type: updated.db_type,
        container_id: updated.container_id,
        container_name: updated.container_name,
        host: updated.host,
        port: updated.port,
        status: updated.status,
        database_count: count,
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    }))
}

pub async fn reset_database_password(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<UserDatabaseResponse>, AppError> {
    let db = sqlx::query_as::<_, UserDatabase>(
        "SELECT * FROM user_databases WHERE id = $1 AND user_id = $2"
    )
    .bind(id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let server = sqlx::query_as::<_, DatabaseServer>(
        "SELECT * FROM database_servers WHERE id = $1"
    )
    .bind(db.server_id)
    .fetch_one(&state.db)
    .await?;

    let new_password = generate_password(24);

    if let Some(container_id) = &server.container_id {
        let docker = get_docker().await?;

        match db.db_type.as_str() {
            "postgresql" => {
                let cmd = format!("ALTER USER {} WITH PASSWORD '{}';", db.db_user, new_password);
                execute_db_command(&docker, container_id, "postgresql", &server.root_password, &cmd).await?;
            }
            "mysql" => {
                let cmd = format!("ALTER USER '{}'@'%' IDENTIFIED BY '{}';", db.db_user, new_password);
                execute_db_command(&docker, container_id, "mysql", &server.root_password, &cmd).await?;
                execute_db_command(&docker, container_id, "mysql", &server.root_password, "FLUSH PRIVILEGES;").await?;
            }
            "redis" => {
                // Update Redis ACL user password
                let acl_cmd = format!(
                    "ACL SETUSER {} on >{} ~* +@all -select +select|{}",
                    db.db_user, new_password, db.db_name
                );
                execute_db_command(&docker, container_id, "redis", &server.root_password, &acl_cmd).await?;
                execute_db_command(&docker, container_id, "redis", &server.root_password, "ACL SAVE").await?;
            }
            _ => {}
        }
    }

    let updated = sqlx::query_as::<_, UserDatabase>(
        "UPDATE user_databases SET db_password = $1, updated_at = NOW() WHERE id = $2 RETURNING *"
    )
    .bind(&new_password)
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    let connection_string = match updated.db_type.as_str() {
        "postgresql" => format!(
            "postgresql://{}:{}@{}:{}/{}",
            updated.db_user, updated.db_password, server.host, server.port, updated.db_name
        ),
        "mysql" => format!(
            "mysql://{}:{}@{}:{}/{}",
            updated.db_user, updated.db_password, server.host, server.port, updated.db_name
        ),
        "redis" => format!(
            "redis://{}:{}@{}:{}/{}",
            updated.db_user, updated.db_password, server.host, server.port, updated.db_name
        ),
        _ => String::new(),
    };

    Ok(Json(UserDatabaseResponse {
        id: updated.id,
        db_type: updated.db_type,
        db_name: updated.db_name,
        db_user: updated.db_user,
        db_password: updated.db_password,
        host: server.host,
        port: server.port,
        status: updated.status,
        connection_string,
        created_at: updated.created_at,
    }))
}

pub async fn get_database_server(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<DatabaseServerAdminResponse>, AppError> {
    if !claims.is_admin() {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let server = sqlx::query_as::<_, DatabaseServer>(
        "SELECT * FROM database_servers WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM user_databases WHERE server_id = $1"
    )
    .bind(server.id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    Ok(Json(DatabaseServerAdminResponse {
        id: server.id,
        db_type: server.db_type,
        container_id: server.container_id,
        container_name: server.container_name,
        host: server.host,
        port: server.port,
        root_password: server.root_password,
        status: server.status,
        database_count: count,
        created_at: server.created_at,
        updated_at: server.updated_at,
    }))
}

pub async fn create_database_server(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateDatabaseServerRequest>,
) -> Result<Json<DatabaseServerAdminResponse>, AppError> {
    if !claims.is_admin() {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    if req.db_type != "postgresql" && req.db_type != "mysql" && req.db_type != "redis" {
        return Err(AppError::BadRequest("Invalid database type. Use 'postgresql', 'mysql', or 'redis'".to_string()));
    }

    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM database_servers WHERE db_type = $1"
    )
    .bind(&req.db_type)
    .fetch_one(&state.db)
    .await?;

    if existing > 0 {
        return Err(AppError::BadRequest(format!("A {} server already exists", req.db_type)));
    }

    let container_name = req.container_name.unwrap_or_else(|| {
        format!("raptor-{}", req.db_type)
    });

    let root_password = generate_password(32);

    let server = sqlx::query_as::<_, DatabaseServer>(
        r#"
        INSERT INTO database_servers (db_type, container_name, host, port, root_password, status)
        VALUES ($1, $2, $3, $4, $5, 'stopped')
        RETURNING *
        "#
    )
    .bind(&req.db_type)
    .bind(&container_name)
    .bind(&req.host)
    .bind(req.port)
    .bind(&root_password)
    .fetch_one(&state.db)
    .await?;

    tracing::info!("Created {} database server", req.db_type);

    Ok(Json(DatabaseServerAdminResponse {
        id: server.id,
        db_type: server.db_type,
        container_id: server.container_id,
        container_name: server.container_name,
        host: server.host,
        port: server.port,
        root_password: server.root_password,
        status: server.status,
        database_count: 0,
        created_at: server.created_at,
        updated_at: server.updated_at,
    }))
}

pub async fn update_database_server(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateDatabaseServerRequest>,
) -> Result<Json<DatabaseServerAdminResponse>, AppError> {
    if !claims.is_admin() {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let server = sqlx::query_as::<_, DatabaseServer>(
        "SELECT * FROM database_servers WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let new_host = req.host.unwrap_or(server.host.clone());
    let new_port = req.port.unwrap_or(server.port);

    // Handle password regeneration
    let new_password = if req.regenerate_password.unwrap_or(false) {
        let password = generate_password(32);

        // If the container is running, update the password in the database
        if server.status == "running" {
            if let Some(container_id) = &server.container_id {
                let docker = get_docker().await?;

                match server.db_type.as_str() {
                    "postgresql" => {
                        let cmd = format!("ALTER USER postgres WITH PASSWORD '{}';", password);
                        execute_db_command(&docker, container_id, "postgresql", &server.root_password, &cmd).await?;
                    }
                    "mysql" => {
                        let cmd = format!("ALTER USER 'root'@'%' IDENTIFIED BY '{}';", password);
                        execute_db_command(&docker, container_id, "mysql", &server.root_password, &cmd).await?;
                        execute_db_command(&docker, container_id, "mysql", &server.root_password, "FLUSH PRIVILEGES;").await?;
                    }
                    _ => {}
                }
            }
        }

        password
    } else {
        server.root_password.clone()
    };

    let updated = sqlx::query_as::<_, DatabaseServer>(
        r#"
        UPDATE database_servers
        SET host = $1, port = $2, root_password = $3, updated_at = NOW()
        WHERE id = $4
        RETURNING *
        "#
    )
    .bind(&new_host)
    .bind(new_port)
    .bind(&new_password)
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM user_databases WHERE server_id = $1"
    )
    .bind(updated.id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    tracing::info!("Updated {} database server", updated.db_type);

    Ok(Json(DatabaseServerAdminResponse {
        id: updated.id,
        db_type: updated.db_type,
        container_id: updated.container_id,
        container_name: updated.container_name,
        host: updated.host,
        port: updated.port,
        root_password: updated.root_password,
        status: updated.status,
        database_count: count,
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    }))
}

pub async fn delete_database_server(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    if !claims.is_admin() {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let server = sqlx::query_as::<_, DatabaseServer>(
        "SELECT * FROM database_servers WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    // Check if there are any user databases
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM user_databases WHERE server_id = $1"
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if count > 0 {
        return Err(AppError::BadRequest(format!(
            "Cannot delete server with {} active databases. Delete them first.",
            count
        )));
    }

    // Stop and remove the container if it exists
    if let Some(container_id) = &server.container_id {
        let docker = get_docker().await?;

        // Stop container
        let _ = docker
            .stop_container(container_id, Some(StopContainerOptions { t: 30 }))
            .await;

        // Remove container
        let _ = docker
            .remove_container(container_id, None)
            .await;
    }

    sqlx::query("DELETE FROM database_servers WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    tracing::info!("Deleted {} database server", server.db_type);

    Ok(StatusCode::NO_CONTENT)
}

