use axum::{
    extract::{Extension, Path, State},
    Json,
};
use rand::Rng;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::{
        AppState, Claims, CreateDatabaseRequest, CreateDatabaseServerRequest,
        DatabaseServer, DatabaseServerAdminResponse, Daemon, UpdateDatabaseServerRequest,
        UserDatabase, UserDatabaseResponse,
    },
};

/// Create HTTP client for daemon communication
/// Accepts self-signed certificates for internal daemon communication
fn daemon_client() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

// ============ User Database Handlers ============

/// List all databases for the current user
pub async fn list_databases(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> AppResult<Json<Vec<UserDatabaseResponse>>> {
    let databases: Vec<UserDatabaseResponse> = sqlx::query_as(
        r#"
        SELECT
            ud.id,
            ud.db_type,
            ud.db_name,
            ud.db_user,
            ud.db_password,
            ds.host,
            ds.port,
            ud.status,
            CASE
                WHEN ud.db_type = 'postgresql' THEN 'postgresql://' || ud.db_user || ':' || ud.db_password || '@' || ds.host || ':' || ds.port || '/' || ud.db_name
                WHEN ud.db_type = 'mysql' THEN 'mysql://' || ud.db_user || ':' || ud.db_password || '@' || ds.host || ':' || ds.port || '/' || ud.db_name
                WHEN ud.db_type = 'redis' THEN 'redis://' || ud.db_user || ':' || ud.db_password || '@' || ds.host || ':' || ds.port
                ELSE ''
            END as connection_string,
            ud.created_at
        FROM user_databases ud
        JOIN database_servers ds ON ds.id = ud.server_id
        WHERE ud.user_id = $1
        ORDER BY ud.created_at DESC
        "#
    )
    .bind(claims.sub)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(databases))
}

/// Create a new database for the current user
pub async fn create_database(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateDatabaseRequest>,
) -> AppResult<Json<UserDatabaseResponse>> {
    // Validate db_type
    if !["postgresql", "mysql", "redis"].contains(&payload.db_type.as_str()) {
        return Err(AppError::BadRequest(
            "Invalid database type. Must be 'postgresql', 'mysql', or 'redis'".to_string(),
        ));
    }

    // Find an active database server for this type
    let server: DatabaseServer = sqlx::query_as(
        r#"SELECT * FROM database_servers WHERE db_type = $1 AND status = 'running' LIMIT 1"#
    )
    .bind(&payload.db_type)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest(format!(
            "No running {} server available",
            payload.db_type
        ))
    })?;

    // Get the daemon for this server
    let daemon: Daemon = sqlx::query_as(
        r#"SELECT * FROM daemons WHERE id = $1"#
    )
    .bind(server.daemon_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Internal("Database server has no daemon assigned".to_string()))?;

    // Generate database name if not provided (alphanumeric only, no hyphens)
    let db_name = payload.db_name.unwrap_or_else(|| {
        format!(
            "db_{}_{}",
            claims.username.chars().filter(|c| c.is_alphanumeric()).take(8).collect::<String>(),
            &Uuid::new_v4().to_string().replace("-", "")[..8]
        )
    });

    // Generate random username and password (alphanumeric only, no hyphens for SQL compatibility)
    let db_user = format!("u_{}", &Uuid::new_v4().to_string().replace("-", "")[..12]);
    let db_password = generate_password(24);

    // Call daemon to create the database
    let client = daemon_client();
    let daemon_url = format!("{}/database-servers/{}/databases", daemon.base_url(), server.id);

    let daemon_req = serde_json::json!({
        "serverId": server.id.to_string(),
        "dbType": payload.db_type,
        "dbName": db_name,
        "dbUser": db_user,
        "dbPassword": db_password
    });

    let res = client
        .post(&daemon_url)
        .header("X-API-Key", &daemon.api_key)
        .json(&daemon_req)
        .send()
        .await
        .map_err(|e| AppError::Daemon(format!("Failed to connect to daemon: {}", e)))?;

    if !res.status().is_success() {
        let error_text = res.text().await.unwrap_or_default();
        return Err(AppError::Daemon(format!("Failed to create database: {}", error_text)));
    }

    // Create the user_database record
    let id = Uuid::new_v4();
    let database: UserDatabase = sqlx::query_as(
        r#"
        INSERT INTO user_databases (id, user_id, server_id, db_type, db_name, db_user, db_password, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'active')
        RETURNING *
        "#
    )
    .bind(id)
    .bind(claims.sub)
    .bind(server.id)
    .bind(&payload.db_type)
    .bind(&db_name)
    .bind(&db_user)
    .bind(&db_password)
    .fetch_one(&state.db)
    .await?;

    // Build response
    let connection_string = build_connection_string(
        &database.db_type,
        &database.db_user,
        &database.db_password,
        &server.host,
        server.port,
        &database.db_name,
    );

    Ok(Json(UserDatabaseResponse {
        id: database.id,
        db_type: database.db_type,
        db_name: database.db_name,
        db_user: database.db_user,
        db_password: database.db_password,
        host: server.host,
        port: server.port,
        status: database.status,
        connection_string,
        created_at: database.created_at,
    }))
}

/// Get available database types (servers that are running)
pub async fn get_available_database_types(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> AppResult<Json<Vec<serde_json::Value>>> {
    // Get all possible database types
    let all_types = vec!["postgresql", "mysql", "redis"];

    // Get running database types
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT DISTINCT db_type FROM database_servers WHERE status = 'running'"#
    )
    .fetch_all(&state.db)
    .await?;

    let running_types: Vec<String> = rows.into_iter().map(|(t,)| t).collect();

    // Build response with availability info
    let result: Vec<serde_json::Value> = all_types
        .iter()
        .map(|t| {
            let name = match *t {
                "postgresql" => "PostgreSQL",
                "mysql" => "MySQL",
                "redis" => "Redis",
                _ => *t,
            };
            serde_json::json!({
                "dbType": t,
                "name": name,
                "available": running_types.contains(&t.to_string())
            })
        })
        .collect();

    Ok(Json(result))
}

/// Get a specific database by ID
pub async fn get_database(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserDatabaseResponse>> {
    let database: UserDatabaseResponse = sqlx::query_as(
        r#"
        SELECT
            ud.id,
            ud.db_type,
            ud.db_name,
            ud.db_user,
            ud.db_password,
            ds.host,
            ds.port,
            ud.status,
            CASE
                WHEN ud.db_type = 'postgresql' THEN 'postgresql://' || ud.db_user || ':' || ud.db_password || '@' || ds.host || ':' || ds.port || '/' || ud.db_name
                WHEN ud.db_type = 'mysql' THEN 'mysql://' || ud.db_user || ':' || ud.db_password || '@' || ds.host || ':' || ds.port || '/' || ud.db_name
                WHEN ud.db_type = 'redis' THEN 'redis://' || ud.db_user || ':' || ud.db_password || '@' || ds.host || ':' || ds.port
                ELSE ''
            END as connection_string,
            ud.created_at
        FROM user_databases ud
        JOIN database_servers ds ON ds.id = ud.server_id
        WHERE ud.id = $1 AND ud.user_id = $2
        "#
    )
    .bind(id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(database))
}

/// Delete a database
pub async fn delete_database(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    // Verify ownership and get database info
    let database: UserDatabase = sqlx::query_as(
        r#"SELECT * FROM user_databases WHERE id = $1 AND user_id = $2"#
    )
    .bind(id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    // Get server and daemon info
    let server: DatabaseServer = sqlx::query_as(
        r#"SELECT * FROM database_servers WHERE id = $1"#
    )
    .bind(database.server_id)
    .fetch_one(&state.db)
    .await?;

    if let Some(daemon_id) = server.daemon_id {
        let daemon: Option<Daemon> = sqlx::query_as(
            r#"SELECT * FROM daemons WHERE id = $1"#
        )
        .bind(daemon_id)
        .fetch_optional(&state.db)
        .await?;

        // Call daemon to delete the database
        if let Some(daemon) = daemon {
            let client = daemon_client();
            let daemon_url = format!("{}/database-servers/{}/databases", daemon.base_url(), server.id);

            let daemon_req = serde_json::json!({
                "serverId": server.id.to_string(),
                "dbType": database.db_type,
                "dbName": database.db_name,
                "dbUser": database.db_user
            });

            let res = client
                .delete(&daemon_url)
                .header("X-API-Key", &daemon.api_key)
                .json(&daemon_req)
                .send()
                .await;

            if let Err(e) = res {
                tracing::warn!("Failed to delete database from daemon: {}", e);
            }
        }
    }

    // Delete from database
    sqlx::query("DELETE FROM user_databases WHERE id = $1")
        .bind(database.id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "message": "Database deleted" })))
}

/// Reset database password
pub async fn reset_database_password(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserDatabaseResponse>> {
    // Verify ownership
    let database: UserDatabase = sqlx::query_as(
        r#"SELECT * FROM user_databases WHERE id = $1 AND user_id = $2"#
    )
    .bind(id)
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    // Get server info
    let server: DatabaseServer = sqlx::query_as(
        r#"SELECT * FROM database_servers WHERE id = $1"#
    )
    .bind(database.server_id)
    .fetch_one(&state.db)
    .await?;

    // Generate new password
    let new_password = generate_password(24);

    // Call daemon to reset password if server is running
    if server.status == "running" {
        if let Some(daemon_id) = server.daemon_id {
            let daemon: Option<Daemon> = sqlx::query_as(
                r#"SELECT * FROM daemons WHERE id = $1"#
            )
            .bind(daemon_id)
            .fetch_optional(&state.db)
            .await?;

            if let Some(daemon) = daemon {
                let client = daemon_client();
                let daemon_url = format!("{}/database-servers/{}/databases/reset-password", daemon.base_url(), server.id);

                let daemon_req = serde_json::json!({
                    "serverId": server.id.to_string(),
                    "dbType": database.db_type,
                    "dbName": database.db_name,
                    "dbUser": database.db_user,
                    "newPassword": new_password
                });

                let res = client
                    .post(&daemon_url)
                    .header("X-API-Key", &daemon.api_key)
                    .json(&daemon_req)
                    .send()
                    .await
                    .map_err(|e| AppError::Daemon(format!("Failed to connect to daemon: {}", e)))?;

                if !res.status().is_success() {
                    let error_text = res.text().await.unwrap_or_default();
                    return Err(AppError::Daemon(format!("Failed to reset password: {}", error_text)));
                }
            }
        }
    }

    // Update the password in API database
    let updated: UserDatabase = sqlx::query_as(
        r#"
        UPDATE user_databases
        SET db_password = $1, updated_at = NOW()
        WHERE id = $2
        RETURNING *
        "#
    )
    .bind(&new_password)
    .bind(database.id)
    .fetch_one(&state.db)
    .await?;

    let connection_string = build_connection_string(
        &updated.db_type,
        &updated.db_user,
        &updated.db_password,
        &server.host,
        server.port,
        &updated.db_name,
    );

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

// ============ Admin Database Server Handlers ============

/// List all database servers (admin only)
pub async fn list_database_servers(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> AppResult<Json<Vec<DatabaseServerAdminResponse>>> {
    let servers: Vec<DatabaseServerAdminResponse> = sqlx::query_as(
        r#"
        SELECT
            ds.id,
            ds.daemon_id,
            ds.db_type,
            ds.container_id,
            ds.container_name,
            ds.host,
            ds.port,
            ds.root_password,
            ds.status,
            COALESCE((SELECT COUNT(*) FROM user_databases WHERE server_id = ds.id), 0) as database_count,
            ds.created_at,
            ds.updated_at
        FROM database_servers ds
        ORDER BY ds.created_at DESC
        "#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(servers))
}

/// Create a new database server (admin only)
pub async fn create_database_server(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(payload): Json<CreateDatabaseServerRequest>,
) -> AppResult<Json<DatabaseServerAdminResponse>> {
    // Validate db_type
    if !["postgresql", "mysql", "redis"].contains(&payload.db_type.as_str()) {
        return Err(AppError::BadRequest(
            "Invalid database type. Must be 'postgresql', 'mysql', or 'redis'".to_string(),
        ));
    }

    // Verify daemon exists
    let daemon: Daemon = sqlx::query_as(
        r#"SELECT * FROM daemons WHERE id = $1"#
    )
    .bind(payload.daemon_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("Daemon not found".to_string()))?;

    // Generate container name if not provided
    let container_name = payload.container_name.unwrap_or_else(|| {
        format!("raptor-{}-{}", payload.db_type, &Uuid::new_v4().to_string()[..8])
    });

    // Generate root password
    let root_password = generate_password(32);

    // Use daemon's host as the database host (where users will connect to)
    let db_host = daemon.host.clone();

    let id = Uuid::new_v4();

    // Create the server in daemon
    let client = daemon_client();
    let daemon_url = format!("{}/database-servers", daemon.base_url());

    let daemon_req = serde_json::json!({
        "id": id.to_string(),
        "dbType": payload.db_type,
        "containerName": container_name,
        "host": db_host,
        "port": payload.port,
        "rootPassword": root_password
    });

    let res = client
        .post(&daemon_url)
        .header("X-API-Key", &daemon.api_key)
        .json(&daemon_req)
        .send()
        .await
        .map_err(|e| AppError::Daemon(format!("Failed to connect to daemon: {}", e)))?;

    if !res.status().is_success() {
        let error_text = res.text().await.unwrap_or_default();
        return Err(AppError::Daemon(format!("Failed to create database server: {}", error_text)));
    }

    // Save to API database
    let server: DatabaseServer = sqlx::query_as(
        r#"
        INSERT INTO database_servers (id, daemon_id, db_type, container_name, host, port, root_password, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'stopped')
        RETURNING *
        "#
    )
    .bind(id)
    .bind(payload.daemon_id)
    .bind(&payload.db_type)
    .bind(&container_name)
    .bind(&db_host)
    .bind(payload.port)
    .bind(&root_password)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(DatabaseServerAdminResponse {
        id: server.id,
        daemon_id: server.daemon_id,
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

/// Get a specific database server (admin only)
pub async fn get_database_server(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DatabaseServerAdminResponse>> {
    let server: DatabaseServerAdminResponse = sqlx::query_as(
        r#"
        SELECT
            ds.id,
            ds.daemon_id,
            ds.db_type,
            ds.container_id,
            ds.container_name,
            ds.host,
            ds.port,
            ds.root_password,
            ds.status,
            COALESCE((SELECT COUNT(*) FROM user_databases WHERE server_id = ds.id), 0) as database_count,
            ds.created_at,
            ds.updated_at
        FROM database_servers ds
        WHERE ds.id = $1
        "#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(server))
}

/// Update a database server (admin only)
pub async fn update_database_server(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateDatabaseServerRequest>,
) -> AppResult<Json<DatabaseServerAdminResponse>> {
    // Check if server exists
    let existing: DatabaseServer = sqlx::query_as(
        r#"SELECT * FROM database_servers WHERE id = $1"#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let new_host = payload.host.unwrap_or(existing.host);
    let new_port = payload.port.unwrap_or(existing.port);
    let new_password = if payload.regenerate_password.unwrap_or(false) {
        generate_password(32)
    } else {
        existing.root_password
    };

    let server: DatabaseServer = sqlx::query_as(
        r#"
        UPDATE database_servers
        SET host = $2, port = $3, root_password = $4, updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#
    )
    .bind(id)
    .bind(&new_host)
    .bind(new_port)
    .bind(&new_password)
    .fetch_one(&state.db)
    .await?;

    let database_count: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM user_databases WHERE server_id = $1"#
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(DatabaseServerAdminResponse {
        id: server.id,
        daemon_id: server.daemon_id,
        db_type: server.db_type,
        container_id: server.container_id,
        container_name: server.container_name,
        host: server.host,
        port: server.port,
        root_password: server.root_password,
        status: server.status,
        database_count: database_count.0,
        created_at: server.created_at,
        updated_at: server.updated_at,
    }))
}

/// Delete a database server (admin only)
pub async fn delete_database_server(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    // Check if server has databases
    let count: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM user_databases WHERE server_id = $1"#
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    if count.0 > 0 {
        return Err(AppError::BadRequest(format!(
            "Cannot delete server with {} existing databases",
            count.0
        )));
    }

    // Get server to find daemon
    let server: DatabaseServer = sqlx::query_as(
        r#"SELECT * FROM database_servers WHERE id = $1"#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    // Delete from daemon
    if let Some(daemon_id) = server.daemon_id {
        let daemon: Option<Daemon> = sqlx::query_as(
            r#"SELECT * FROM daemons WHERE id = $1"#
        )
        .bind(daemon_id)
        .fetch_optional(&state.db)
        .await?;

        if let Some(daemon) = daemon {
            let client = daemon_client();
            let daemon_url = format!("{}/database-servers/{}", daemon.base_url(), id);

            let res = client
                .delete(&daemon_url)
                .header("X-API-Key", &daemon.api_key)
                .send()
                .await;

            if let Err(e) = res {
                tracing::warn!("Failed to delete database server from daemon: {}", e);
            }
        }
    }

    let result = sqlx::query("DELETE FROM database_servers WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "message": "Database server deleted" })))
}

/// Start a database server (admin only)
pub async fn start_database_server(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DatabaseServerAdminResponse>> {
    // Get the server
    let server: DatabaseServer = sqlx::query_as(
        r#"SELECT * FROM database_servers WHERE id = $1"#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    // Get daemon
    let daemon: Daemon = sqlx::query_as(
        r#"SELECT * FROM daemons WHERE id = $1"#
    )
    .bind(server.daemon_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("Database server has no daemon assigned".to_string()))?;

    let client = daemon_client();

    // First, ensure the server exists in the daemon by trying to create it
    // (the daemon will handle if it already exists)
    let create_url = format!("{}/database-servers", daemon.base_url());
    let create_req = serde_json::json!({
        "id": server.id.to_string(),
        "dbType": server.db_type,
        "containerName": server.container_name,
        "host": server.host,
        "port": server.port,
        "rootPassword": server.root_password
    });

    // Try to create - ignore errors if already exists
    let _ = client
        .post(&create_url)
        .header("X-API-Key", &daemon.api_key)
        .json(&create_req)
        .send()
        .await;

    // Now call daemon to start the container
    let daemon_url = format!("{}/database-servers/{}/start", daemon.base_url(), id);

    let res = client
        .post(&daemon_url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::Daemon(format!("Failed to connect to daemon: {}", e)))?;

    if !res.status().is_success() {
        let error_text = res.text().await.unwrap_or_default();
        return Err(AppError::Daemon(format!("Failed to start database server: {}", error_text)));
    }

    // Get updated server info from daemon response
    let daemon_response: serde_json::Value = res.json().await
        .map_err(|e| AppError::Daemon(format!("Failed to parse daemon response: {}", e)))?;

    let container_id = daemon_response.get("containerId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Update status in API database
    let updated: DatabaseServer = sqlx::query_as(
        r#"
        UPDATE database_servers
        SET status = 'running', container_id = COALESCE($2, container_id), updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#
    )
    .bind(id)
    .bind(container_id)
    .fetch_one(&state.db)
    .await?;

    let database_count: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM user_databases WHERE server_id = $1"#
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(DatabaseServerAdminResponse {
        id: updated.id,
        daemon_id: updated.daemon_id,
        db_type: updated.db_type,
        container_id: updated.container_id,
        container_name: updated.container_name,
        host: updated.host,
        port: updated.port,
        root_password: updated.root_password,
        status: updated.status,
        database_count: database_count.0,
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    }))
}

/// Stop a database server (admin only)
pub async fn stop_database_server(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DatabaseServerAdminResponse>> {
    // Get the server
    let server: DatabaseServer = sqlx::query_as(
        r#"SELECT * FROM database_servers WHERE id = $1"#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    // Get daemon
    let daemon: Daemon = sqlx::query_as(
        r#"SELECT * FROM daemons WHERE id = $1"#
    )
    .bind(server.daemon_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("Database server has no daemon assigned".to_string()))?;

    let client = daemon_client();

    // First, ensure the server exists in the daemon by trying to create it
    let create_url = format!("{}/database-servers", daemon.base_url());
    let create_req = serde_json::json!({
        "id": server.id.to_string(),
        "dbType": server.db_type,
        "containerName": server.container_name,
        "host": server.host,
        "port": server.port,
        "rootPassword": server.root_password
    });

    // Try to create - ignore errors if already exists
    let _ = client
        .post(&create_url)
        .header("X-API-Key", &daemon.api_key)
        .json(&create_req)
        .send()
        .await;

    // Call daemon to stop the container
    let daemon_url = format!("{}/database-servers/{}/stop", daemon.base_url(), id);

    let res = client
        .post(&daemon_url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::Daemon(format!("Failed to connect to daemon: {}", e)))?;

    if !res.status().is_success() {
        let error_text = res.text().await.unwrap_or_default();
        return Err(AppError::Daemon(format!("Failed to stop database server: {}", error_text)));
    }

    // Update status in API database
    let updated: DatabaseServer = sqlx::query_as(
        r#"
        UPDATE database_servers
        SET status = 'stopped', updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    let database_count: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM user_databases WHERE server_id = $1"#
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(DatabaseServerAdminResponse {
        id: updated.id,
        daemon_id: updated.daemon_id,
        db_type: updated.db_type,
        container_id: updated.container_id,
        container_name: updated.container_name,
        host: updated.host,
        port: updated.port,
        root_password: updated.root_password,
        status: updated.status,
        database_count: database_count.0,
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    }))
}

/// Restart a database server (admin only)
pub async fn restart_database_server(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DatabaseServerAdminResponse>> {
    // Get the server
    let server: DatabaseServer = sqlx::query_as(
        r#"SELECT * FROM database_servers WHERE id = $1"#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    // Get daemon
    let daemon: Daemon = sqlx::query_as(
        r#"SELECT * FROM daemons WHERE id = $1"#
    )
    .bind(server.daemon_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("Database server has no daemon assigned".to_string()))?;

    // Update status to restarting
    sqlx::query(
        r#"UPDATE database_servers SET status = 'restarting', updated_at = NOW() WHERE id = $1"#
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    let client = daemon_client();

    // First, ensure the server exists in the daemon by trying to create it
    let create_url = format!("{}/database-servers", daemon.base_url());
    let create_req = serde_json::json!({
        "id": server.id.to_string(),
        "dbType": server.db_type,
        "containerName": server.container_name,
        "host": server.host,
        "port": server.port,
        "rootPassword": server.root_password
    });

    // Try to create - ignore errors if already exists
    let _ = client
        .post(&create_url)
        .header("X-API-Key", &daemon.api_key)
        .json(&create_req)
        .send()
        .await;

    // Call daemon to restart the container
    let daemon_url = format!("{}/database-servers/{}/restart", daemon.base_url(), id);

    let res = client
        .post(&daemon_url)
        .header("X-API-Key", &daemon.api_key)
        .send()
        .await
        .map_err(|e| AppError::Daemon(format!("Failed to connect to daemon: {}", e)))?;

    if !res.status().is_success() {
        let error_text = res.text().await.unwrap_or_default();
        // Revert status on failure
        sqlx::query(
            r#"UPDATE database_servers SET status = 'error', updated_at = NOW() WHERE id = $1"#
        )
        .bind(id)
        .execute(&state.db)
        .await?;
        return Err(AppError::Daemon(format!("Failed to restart database server: {}", error_text)));
    }

    // Get updated server info from daemon response
    let daemon_response: serde_json::Value = res.json().await
        .map_err(|e| AppError::Daemon(format!("Failed to parse daemon response: {}", e)))?;

    let container_id = daemon_response.get("containerId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Update status in API database
    let updated: DatabaseServer = sqlx::query_as(
        r#"
        UPDATE database_servers
        SET status = 'running', container_id = COALESCE($2, container_id), updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#
    )
    .bind(id)
    .bind(container_id)
    .fetch_one(&state.db)
    .await?;

    let database_count: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM user_databases WHERE server_id = $1"#
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(DatabaseServerAdminResponse {
        id: updated.id,
        daemon_id: updated.daemon_id,
        db_type: updated.db_type,
        container_id: updated.container_id,
        container_name: updated.container_name,
        host: updated.host,
        port: updated.port,
        root_password: updated.root_password,
        status: updated.status,
        database_count: database_count.0,
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    }))
}

// ============ Helper Functions ============

fn generate_password(length: usize) -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

fn build_connection_string(
    db_type: &str,
    db_user: &str,
    db_password: &str,
    host: &str,
    port: i32,
    db_name: &str,
) -> String {
    match db_type {
        "postgresql" => format!(
            "postgresql://{}:{}@{}:{}/{}",
            db_user, db_password, host, port, db_name
        ),
        "mysql" => format!(
            "mysql://{}:{}@{}:{}/{}",
            db_user, db_password, host, port, db_name
        ),
        // Redis uses ACL with key prefix isolation: all keys must be prefixed with "db_name:"
        // Connection: redis://username:password@host:port
        "redis" => format!("redis://{}:{}@{}:{}", db_user, db_password, host, port),
        _ => String::new(),
    }
}
