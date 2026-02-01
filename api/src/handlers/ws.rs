use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use uuid::Uuid;
use std::collections::HashMap;

use crate::error::AppError;
use crate::models::{AppState, Container, Daemon};

pub async fn container_logs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> Result<Response, AppError> {

    let token = params.get("token").ok_or(AppError::Unauthorized)?;
    validate_token(token, &state.config.jwt_secret)?;

    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(ws.on_upgrade(move |socket| handle_logs_ws(socket, daemon, container)))
}

fn validate_token(token: &str, secret: &str) -> Result<(), AppError> {
    use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};

    let validation = Validation::new(Algorithm::HS256);
    decode::<serde_json::Value>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    ).map_err(|_| AppError::Unauthorized)?;

    Ok(())
}

async fn handle_logs_ws(socket: WebSocket, daemon: Daemon, container: Container) {
    let (mut sender, mut receiver) = socket.split();

    let ws_protocol = if daemon.secure { "wss" } else { "ws" };
    let daemon_ws_url = format!(
        "{}://{}:{}/ws/containers/{}/logs?api_key={}",
        ws_protocol, daemon.host, daemon.port, container.id, daemon.api_key
    );

    tracing::info!("Connecting to daemon WebSocket: {}", daemon_ws_url);

    let ws_stream = match tokio_tungstenite::connect_async(&daemon_ws_url).await {
        Ok((stream, _)) => {
            tracing::info!("Connected to daemon WebSocket for container {}", container.id);
            stream
        }
        Err(e) => {
            tracing::error!("Failed to connect to daemon WebSocket: {}", e);
            let _ = sender
                .send(Message::Text(format!("Error connecting to daemon: {}", e)))
                .await;
            return;
        }
    };

    let (mut daemon_sender, mut daemon_receiver) = ws_stream.split();

    let forward_to_client = async {
        while let Some(msg) = daemon_receiver.next().await {
            match msg {
                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                    if sender.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Binary(data)) => {
                    if sender.send(Message::Binary(data)).await.is_err() {
                        break;
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => break,
                Err(e) => {
                    tracing::error!("Daemon WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    };

    let forward_to_daemon = async {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if daemon_sender
                        .send(tokio_tungstenite::tungstenite::Message::Text(text))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = forward_to_client => {},
        _ = forward_to_daemon => {},
    }
}

pub async fn container_stats(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> Result<Response, AppError> {

    let token = params.get("token").ok_or(AppError::Unauthorized)?;
    validate_token(token, &state.config.jwt_secret)?;

    let container: Container = sqlx::query_as("SELECT * FROM containers WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let daemon: Daemon = sqlx::query_as("SELECT * FROM daemons WHERE id = $1")
        .bind(container.daemon_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(ws.on_upgrade(move |socket| handle_stats_ws(socket, daemon, container)))
}

async fn handle_stats_ws(socket: WebSocket, daemon: Daemon, container: Container) {
    let (mut sender, mut receiver) = socket.split();

    let ws_protocol = if daemon.secure { "wss" } else { "ws" };
    let daemon_ws_url = format!(
        "{}://{}:{}/ws/containers/{}/stats?api_key={}",
        ws_protocol, daemon.host, daemon.port, container.id, daemon.api_key
    );

    tracing::info!("Connecting to daemon stats WebSocket: {}", daemon_ws_url);

    let ws_stream = match tokio_tungstenite::connect_async(&daemon_ws_url).await {
        Ok((stream, _)) => {
            tracing::info!("Connected to daemon stats WebSocket for container {}", container.id);
            stream
        }
        Err(e) => {
            tracing::error!("Failed to connect to daemon stats WebSocket: {}", e);
            let _ = sender
                .send(Message::Text(format!("Error connecting to daemon: {}", e)))
                .await;
            return;
        }
    };

    let (mut daemon_sender, mut daemon_receiver) = ws_stream.split();

    let forward_to_client = async {
        while let Some(msg) = daemon_receiver.next().await {
            match msg {
                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                    if sender.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => break,
                Err(e) => {
                    tracing::error!("Daemon stats WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    };

    let recv_task = async {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = forward_to_client => {},
        _ = recv_task => {},
    }
}

