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
}

async fn check_daemon_status(host: &str, port: i32, api_key: &str) -> (String, Option<SystemResources>) {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return ("offline".to_string(), None),
    };

    let health_url = format!("http://{}:{}/health", host, port);
    let health_ok = match client.get(&health_url).header("X-API-Key", api_key).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    };

    if !health_ok {
        return ("offline".to_string(), None);
    }

    let system_url = format!("http://{}:{}/system", host, port);
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

    let (status, system) = check_daemon_status(&daemon.host, daemon.port, &daemon.api_key).await;

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
        INSERT INTO daemons (id, name, host, port, api_key, location, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
        RETURNING *
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(&req.name)
    .bind(&req.host)
    .bind(req.port)
    .bind(&api_key)
    .bind(&req.location)
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
        let ws_url = format!(
            "ws://{}:{}/ws/system?api_key={}",
            daemon.host, daemon.port, daemon.api_key
        );

        if let Ok((ws_stream, _)) = tokio_tungstenite::connect_async(&ws_url).await {
            daemon_sockets.insert(daemon.id, ws_stream);
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

