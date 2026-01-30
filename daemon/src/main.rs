mod config;
mod database_manager;
mod docker;
mod ftp;
mod handlers;
mod models;

use axum::{
    routing::{get, post, delete, patch},
    Router,
    extract::DefaultBodyLimit,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::database_manager::DatabaseManager;
use crate::docker::DockerManager;
use crate::models::{AppState, ContainerLocks};
use crate::ftp::FtpServerState;

/// Chunk size for large file uploads (55MB)
pub const UPLOAD_CHUNK_SIZE: usize = 55 * 1024 * 1024;

/// Maximum body size for chunk uploads
/// Base64 adds ~33% overhead: 55MB * 1.33 = ~73MB, plus JSON structure = ~80MB
pub const UPLOAD_CHUNK_BODY_LIMIT: usize = 80 * 1024 * 1024; // 80MB

/// Maximum body size for full file writes (500MB)
pub const MAX_FILE_WRITE_SIZE: usize = 500 * 1024 * 1024;

/// Load TLS configuration from certificate and key files
fn load_tls_config(cert_path: &str, key_path: &str) -> anyhow::Result<axum_server::tls_rustls::RustlsConfig> {
    use std::fs::File;
    use std::io::BufReader;
    use rustls_pemfile::{certs, private_key};

    // Read certificate chain
    let cert_file = File::open(cert_path)
        .map_err(|e| anyhow::anyhow!("Failed to open certificate file '{}': {}", cert_path, e))?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<_> = certs(&mut cert_reader)
        .filter_map(|r| r.ok())
        .collect();

    if certs.is_empty() {
        anyhow::bail!("No certificates found in '{}'", cert_path);
    }

    // Read private key
    let key_file = File::open(key_path)
        .map_err(|e| anyhow::anyhow!("Failed to open key file '{}': {}", key_path, e))?;
    let mut key_reader = BufReader::new(key_file);
    let key = private_key(&mut key_reader)
        .map_err(|e| anyhow::anyhow!("Failed to parse private key: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("No private key found in '{}'", key_path))?;

    // Build rustls config
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs.into_iter().map(|c| c.into()).collect(), key.into())
        .map_err(|e| anyhow::anyhow!("Failed to build TLS config: {}", e))?;

    Ok(axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(config)))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // Install the ring crypto provider for rustls before any TLS operations
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

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

    // Initialize database manager and load state
    let database_manager = DatabaseManager::new();
    database_manager.load_state().await;
    tracing::info!("Database manager initialized");

    let app_state = Arc::new(AppState {
        docker,
        api_key: config.daemon_api_key.clone(),
        containers: containers_map,
        ftp_state: ftp_state.clone(),
        container_locks: ContainerLocks::new(),
        database_manager,
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
        .route("/containers/:name/files/write", post(handlers::write_file)
            .layer(DefaultBodyLimit::max(MAX_FILE_WRITE_SIZE))) // 500MB for file writes
        .route("/containers/:name/files/write-chunk", post(handlers::write_file_chunk)
            .layer(DefaultBodyLimit::max(UPLOAD_CHUNK_BODY_LIMIT))) // 60MB per chunk (55MB + overhead)
        .route("/containers/:name/files/folder", post(handlers::create_folder))
        .route("/containers/:name/files/delete", delete(handlers::delete_file))
        // Database server management
        .route("/database-servers", get(handlers::list_database_servers))
        .route("/database-servers", post(handlers::create_database_server))
        .route("/database-servers/:id", get(handlers::get_database_server))
        .route("/database-servers/:id", delete(handlers::delete_database_server))
        .route("/database-servers/:id/start", post(handlers::start_database_server))
        .route("/database-servers/:id/stop", post(handlers::stop_database_server))
        .route("/database-servers/:id/restart", post(handlers::restart_database_server))
        // User database operations (called by API)
        .route("/database-servers/:id/databases", post(handlers::create_user_database))
        .route("/database-servers/:id/databases", delete(handlers::delete_user_database))
        .route("/database-servers/:id/databases/reset-password", post(handlers::reset_user_database_password))
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

    // Check for TLS configuration
    let tls_cert_path = std::env::var("TLS_CERT_PATH").ok();
    let tls_key_path = std::env::var("TLS_KEY_PATH").ok();

    let addr: std::net::SocketAddr = config.daemon_addr.parse()?;

    match (tls_cert_path, tls_key_path) {
        (Some(cert_path), Some(key_path)) => {
            // HTTPS mode
            tracing::info!("Loading TLS certificates from {} and {}", cert_path, key_path);
            let tls_config = load_tls_config(&cert_path, &key_path)?;
            tracing::info!("Daemon listening on {} (HTTPS)", addr);
            axum_server::bind_rustls(addr, tls_config)
                .serve(app.into_make_service())
                .await?;
        }
        (Some(_), None) | (None, Some(_)) => {
            anyhow::bail!("Both TLS_CERT_PATH and TLS_KEY_PATH must be set for HTTPS");
        }
        (None, None) => {
            // HTTP mode (default)
            let listener = tokio::net::TcpListener::bind(&config.daemon_addr).await?;
            tracing::info!("Daemon listening on {} (HTTP)", config.daemon_addr);
            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}
