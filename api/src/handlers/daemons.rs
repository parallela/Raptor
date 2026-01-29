use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{AppState, CreateDaemonRequest, Daemon};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonResponse {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub api_key: String,
    pub location: Option<String>,
    pub secure: bool,
    pub status: String,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonStatusResponse {
    pub id: Uuid,
    pub status: String,
    pub system: Option<SystemResources>,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDaemonRequest {
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub location: Option<String>,
    pub secure: Option<bool>,
}

async fn check_daemon_status(host: &str, port: i32, api_key: &str, secure: bool) -> (String, Option<SystemResources>) {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .danger_accept_invalid_certs(true) // Accept self-signed certs for daemon communication
        .build()
    {
        Ok(c) => c,
        Err(_) => return ("offline".to_string(), None),
    };

    let scheme = if secure { "https" } else { "http" };
    let health_url = format!("{}://{}:{}/health", scheme, host, port);
    let health_ok = match client.get(&health_url).header("X-API-Key", api_key).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    };

    if !health_ok {
        return ("offline".to_string(), None);
    }

    let system_url = format!("{}://{}:{}/system", scheme, host, port);
    let system = match client.get(&system_url).header("X-API-Key", api_key).send().await {
        Ok(resp) if resp.status().is_success() => resp.json::<SystemResources>().await.ok(),
        _ => None,
    };

    ("online".to_string(), system)
}

pub async fn list_daemons(State(state): State<AppState>) -> AppResult<Json<Vec<DaemonResponse>>> {
    let daemons: Vec<Daemon> = sqlx::query_as("SELECT * FROM daemons ORDER BY created_at DESC")
        .fetch_all(&state.db)
        .await?;

    let responses: Vec<DaemonResponse> = daemons
        .into_iter()
        .map(|daemon| DaemonResponse {
            id: daemon.id,
            name: daemon.name,
            host: daemon.host,
            port: daemon.port,
            api_key: daemon.api_key,
            location: daemon.location,
            secure: daemon.secure,
            status: "unknown".to_string(),
            created_at: daemon.created_at,
            updated_at: daemon.updated_at,
        })
        .collect();

    Ok(Json(responses))
}

pub async fn get_daemon_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DaemonStatusResponse>> {
    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let (status, system) = check_daemon_status(&daemon.host, daemon.port, &daemon.api_key, daemon.secure).await;

    Ok(Json(DaemonStatusResponse { id: daemon.id, status, system }))
}

pub async fn create_daemon(
    State(state): State<AppState>,
    Json(req): Json<CreateDaemonRequest>,
) -> AppResult<Json<DaemonResponse>> {
    let api_key = Uuid::new_v4().to_string();
    let now = Utc::now();

    let daemon: Daemon = sqlx::query_as(
        r#"
        INSERT INTO daemons (id, name, host, port, api_key, location, secure, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(&req.name)
    .bind(&req.host)
    .bind(req.port)
    .bind(&api_key)
    .bind(&req.location)
    .bind(req.secure)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(DaemonResponse {
        id: daemon.id,
        name: daemon.name,
        host: daemon.host,
        port: daemon.port,
        api_key: daemon.api_key,
        location: daemon.location,
        secure: daemon.secure,
        status: "unknown".to_string(),
        created_at: daemon.created_at,
        updated_at: daemon.updated_at,
    }))
}

pub async fn get_daemon(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DaemonResponse>> {
    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(DaemonResponse {
        id: daemon.id,
        name: daemon.name,
        host: daemon.host,
        port: daemon.port,
        api_key: daemon.api_key,
        location: daemon.location,
        secure: daemon.secure,
        status: "unknown".to_string(),
        created_at: daemon.created_at,
        updated_at: daemon.updated_at,
    }))
}

pub async fn update_daemon(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateDaemonRequest>,
) -> AppResult<Json<DaemonResponse>> {
    let daemon: Daemon = sqlx::query_as(
        r#"
        UPDATE daemons SET
            name = COALESCE($2, name),
            host = COALESCE($3, host),
            port = COALESCE($4, port),
            location = COALESCE($5, location),
            secure = COALESCE($6, secure),
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(&req.name)
    .bind(&req.host)
    .bind(req.port)
    .bind(&req.location)
    .bind(req.secure)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(DaemonResponse {
        id: daemon.id,
        name: daemon.name,
        host: daemon.host,
        port: daemon.port,
        api_key: daemon.api_key,
        location: daemon.location,
        secure: daemon.secure,
        status: "unknown".to_string(),
        created_at: daemon.created_at,
        updated_at: daemon.updated_at,
    }))
}

pub async fn delete_daemon(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let result = sqlx::query("DELETE FROM daemons WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({"message": "Daemon deleted successfully"})))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingDaemonRequest {
    pub host: String,
    pub port: i32,
    pub api_key: String,
    #[serde(default)]
    pub secure: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingDaemonResponse {
    pub online: bool,
    pub latency_ms: Option<u64>,
    pub version: Option<String>,
    pub system: Option<SystemResources>,
    pub error: Option<String>,
}

pub async fn ping_daemon(
    Json(req): Json<PingDaemonRequest>,
) -> AppResult<Json<PingDaemonResponse>> {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .build()
    {
        Ok(c) => c,
        Err(e) => return Ok(Json(PingDaemonResponse {
            online: false,
            latency_ms: None,
            version: None,
            system: None,
            error: Some(format!("Failed to create HTTP client: {}", e)),
        })),
    };

    let scheme = if req.secure { "https" } else { "http" };
    let health_url = format!("{}://{}:{}/health", scheme, req.host, req.port);

    let start = std::time::Instant::now();
    let health_result = client
        .get(&health_url)
        .header("X-API-Key", &req.api_key)
        .send()
        .await;
    let latency = start.elapsed().as_millis() as u64;

    match health_result {
        Ok(resp) if resp.status().is_success() => {
            // Try to get system info
            let system_url = format!("{}://{}:{}/system", scheme, req.host, req.port);
            let system = match client
                .get(&system_url)
                .header("X-API-Key", &req.api_key)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => resp.json::<SystemResources>().await.ok(),
                _ => None,
            };

            Ok(Json(PingDaemonResponse {
                online: true,
                latency_ms: Some(latency),
                version: Some("1.0.0".to_string()),
                system,
                error: None,
            }))
        }
        Ok(resp) => Ok(Json(PingDaemonResponse {
            online: false,
            latency_ms: Some(latency),
            version: None,
            system: None,
            error: Some(format!("Daemon returned status: {}", resp.status())),
        })),
        Err(e) => Ok(Json(PingDaemonResponse {
            online: false,
            latency_ms: None,
            version: None,
            system: None,
            error: Some(format!("Connection failed: {}", e)),
        })),
    }
}

pub async fn ws_daemon_stats(
    State(state): State<AppState>,
    ws: axum::extract::ws::WebSocketUpgrade,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|socket| handle_daemon_stats_socket(socket, state))
}

async fn handle_daemon_stats_socket(socket: axum::extract::ws::WebSocket, state: AppState) {
    use axum::extract::ws::Message;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message as TungMessage;

    let (mut sender, mut receiver) = socket.split();

    let daemons: Vec<Daemon> = match sqlx::query_as("SELECT * FROM daemons")
        .fetch_all(&state.db)
        .await
    {
        Ok(d) => d,
        Err(_) => return,
    };

    let mut daemon_sockets: std::collections::HashMap<Uuid, tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>> = std::collections::HashMap::new();

    for daemon in &daemons {
        let ws_protocol = if daemon.secure { "wss" } else { "ws" };
        let ws_url = format!(
            "{}://{}:{}/ws/system?api_key={}",
            ws_protocol, daemon.host, daemon.port, daemon.api_key
        );

        match tokio_tungstenite::connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                daemon_sockets.insert(daemon.id, ws_stream);
            }
            Err(e) => {
                tracing::warn!("Failed to connect to daemon {} WebSocket: {}", daemon.id, e);
            }
        }
    }

    let daemon_ids: Vec<Uuid> = daemon_sockets.keys().cloned().collect();

    let send_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));

        loop {
            interval.tick().await;

            for daemon_id in &daemon_ids {
                if let Some(ws) = daemon_sockets.get_mut(daemon_id) {
                    if let Some(Ok(TungMessage::Text(text))) = ws.next().await {
                        let response = serde_json::json!({
                            "daemonId": daemon_id.to_string(),
                            "system": serde_json::from_str::<serde_json::Value>(&text).unwrap_or_default()
                        });

                        if sender.send(Message::Text(response.to_string())).await.is_err() {
                            return;
                        }
                    }
                }
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

