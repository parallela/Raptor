use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub daemon_addr: String,
    pub daemon_api_key: String,
    pub docker_socket: String,
    pub sftp_base_path: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            daemon_addr: std::env::var("DAEMON_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:8080".into()),
            daemon_api_key: std::env::var("DAEMON_API_KEY")
                .unwrap_or_else(|_| "daemon-secret".into()),
            docker_socket: std::env::var("DOCKER_SOCKET")
                .unwrap_or_else(|_| "/var/run/docker.sock".into()),
            sftp_base_path: std::env::var("SFTP_BASE_PATH")
                .unwrap_or_else(|_| "/data/containers".into()),
        }
    }
}
