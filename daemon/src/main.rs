mod config;
mod docker;
mod ftp;
mod handlers;
mod models;

use axum::{
    routing::{get, post, delete, patch},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::docker::DockerManager;
use crate::models::{AppState, ContainerLocks};
use crate::ftp::FtpServerState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    tracing::info!("Loaded config: {:?}", config);

    let docker = DockerManager::new().await?;

    // Create FTP server state
    let ftp_base_path = std::env::var("FTP_BASE_PATH")
        .unwrap_or_else(|_| std::env::var("SFTP_BASE_PATH")
            .unwrap_or_else(|_| "/Users/lubomirstankov/Development/me/raptor/daemon".into()));
    let ftp_state = Arc::new(FtpServerState::new(&ftp_base_path));

    // Load saved container state from disk
    let saved_containers = handlers::load_container_state().await;
    let containers_map = dashmap::DashMap::new();
    for container in saved_containers {
        containers_map.insert(container.name.clone(), container);
    }
    tracing::info!("Loaded {} containers from saved state", containers_map.len());

    let app_state = Arc::new(AppState {
        docker,
        api_key: config.daemon_api_key.clone(),
        containers: containers_map,
        ftp_state: ftp_state.clone(),
        container_locks: ContainerLocks::new(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Container management
        .route("/containers", get(handlers::list_containers))
        .route("/containers", post(handlers::create_container))
        .route("/containers/:id", get(handlers::get_container))
        .route("/containers/:id", delete(handlers::delete_container))
        .route("/containers/:id", patch(handlers::update_container))
        .route("/containers/:id/start", post(handlers::start_container))
        .route("/containers/:id/stop", post(handlers::stop_container))
        .route("/containers/:id/restart", post(handlers::restart_container))
        .route("/containers/:id/recreate", post(handlers::recreate_container))
        .route("/containers/:id/kill", post(handlers::kill_container))
        .route("/containers/:id/command", post(handlers::send_command))
        .route("/containers/:id/graceful-stop", post(handlers::graceful_stop_container))
        .route("/containers/:id/ftp", post(handlers::create_ftp))
        .route("/containers/:id/stats", get(handlers::get_container_stats))
        .route("/containers/:id/status", get(handlers::get_container_status))
        // Allocations
        .route("/allocations", get(handlers::list_allocations))
        .route("/allocations/assign", post(handlers::assign_allocation))
        // WebSocket for logs
        .route("/ws/containers/:id/logs", get(handlers::ws_logs))
        // WebSocket for container stats
        .route("/ws/containers/:id/stats", get(handlers::ws_container_stats))
        // WebSocket for system stats
        .route("/ws/system", get(handlers::ws_system_stats))
        // File management
        .route("/containers/:name/files", get(handlers::list_files))
        .route("/containers/:name/files/read", get(handlers::read_file))
        .route("/containers/:name/files/write", post(handlers::write_file))
        .route("/containers/:name/files/folder", post(handlers::create_folder))
        .route("/containers/:name/files/delete", delete(handlers::delete_file))
        // Health check
        .route("/health", get(|| async { "OK" }))
        .route("/system", get(handlers::get_system_resources))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    // Start FTP server in background
    let ftp_host = std::env::var("FTP_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let ftp_port: u16 = std::env::var("FTP_PORT")
        .unwrap_or_else(|_| "2121".into())
        .parse()
        .unwrap_or(2121);

    let ftp_state_clone = ftp_state.clone();
    tokio::spawn(async move {
        if let Err(e) = ftp::start_ftp_server(ftp_state_clone, &ftp_host, ftp_port).await {
            tracing::error!("FTP server error: {}", e);
        }
    });

    let listener = tokio::net::TcpListener::bind(&config.daemon_addr).await?;
    tracing::info!("Daemon listening on {}", config.daemon_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
