use bollard::container::{
    AttachContainerOptions, AttachContainerResults,
    Config, CreateContainerOptions, ListContainersOptions, LogOutput, LogsOptions,
    RemoveContainerOptions, StartContainerOptions, StatsOptions, StopContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::Docker;
use futures_util::StreamExt;
use std::collections::HashMap;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;

use crate::models::{ContainerInfo, ContainerResources, ContainerStats};

pub struct DockerManager {
    docker: Docker,
}

impl DockerManager {
    pub async fn new() -> anyhow::Result<Self> {
        let docker = if let Ok(host) = std::env::var("DOCKER_HOST") {
            if host.starts_with("unix://") {
                Docker::connect_with_socket(&host[7..], 120, bollard::API_DEFAULT_VERSION)?
            } else {
                Docker::connect_with_local_defaults()?
            }
        } else {
            Docker::connect_with_local_defaults()?
        };
        docker.ping().await?;
        tracing::info!("Connected to Docker daemon");
        Ok(Self { docker })
    }


    pub async fn create_container_with_resources(
        &self,
        name: &str,
        image: &str,
        startup_script: Option<&str>,
        port_bindings: Option<HashMap<String, Vec<bollard::service::PortBinding>>>,
        resources: &ContainerResources,
    ) -> anyhow::Result<String> {
        let mut stream = self.docker.create_image(
            Some(CreateImageOptions {
                from_image: image,
                ..Default::default()
            }),
            None,
            None,
        );

        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => tracing::debug!("Pulling image: {:?}", info),
                Err(e) => tracing::warn!("Image pull warning: {}", e),
            }
        }

        // Use exec to replace the shell with the actual process, so stdin goes directly to it
        // This makes PID 1 be the actual java process instead of /bin/sh
        let cmd_strings: Option<Vec<String>> = startup_script.map(|s| {
            vec!["/bin/sh".to_string(), "-c".to_string(), format!("exec {}", s)]
        });
        let cmd: Option<Vec<&str>> = cmd_strings.as_ref().map(|v| v.iter().map(|s| s.as_str()).collect());

        let memory_bytes = resources.memory_limit * 1024 * 1024;
        let swap_bytes = resources.swap_limit * 1024 * 1024;
        let cpu_period = 100000i64;
        let cpu_quota = (resources.cpu_limit * cpu_period as f64) as i64;

        // Restart policy: always restart on failure (no retry limit)
        let restart_policy = bollard::service::RestartPolicy {
            name: Some(bollard::service::RestartPolicyNameEnum::ON_FAILURE),
            maximum_retry_count: None,
        };

        // Volume binding: mount {FTP_BASE_PATH}/volumes/{container_name} to /home/container
        let base_path = std::env::var("FTP_BASE_PATH")
            .unwrap_or_else(|_| std::env::var("SFTP_BASE_PATH")
                .unwrap_or_else(|_| "/data/raptor".into()));
        let volume_path = format!("{}/volumes/{}", base_path, name);

        // Create the volume directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&volume_path) {
            tracing::warn!("Failed to create volume directory {}: {}", volume_path, e);
        }

        let binds = vec![format!("{}:/home/container:rw", volume_path)];

        let host_config = bollard::service::HostConfig {
            port_bindings: port_bindings.as_ref().map(|pb| pb.iter().map(|(k, v)| (k.clone(), Some(v.clone()))).collect()),
            memory: Some(memory_bytes),
            memory_swap: Some(memory_bytes + swap_bytes),
            cpu_period: Some(cpu_period),
            cpu_quota: Some(cpu_quota),
            blkio_weight: Some(resources.io_weight as u16),
            restart_policy: Some(restart_policy),
            binds: Some(binds),
            ..Default::default()
        };

        // Build exposed_ports from port_bindings keys - store as Vec to own the strings
        let exposed_port_keys: Vec<String> = port_bindings
            .as_ref()
            .map(|pb| pb.keys().cloned().collect())
            .unwrap_or_default();

        let config = Config {
            image: Some(image),
            cmd,
            host_config: Some(host_config),
            working_dir: Some("/home/container"),
            exposed_ports: if exposed_port_keys.is_empty() {
                None
            } else {
                Some(exposed_port_keys.iter().map(|k| (k.as_str(), HashMap::new())).collect())
            },
            // Interactive mode WITHOUT TTY - this allows:
            // 1. docker logs to work normally (no TTY escape sequences/prompts)
            // 2. stdin commands to still work via docker attach
            // Note: tty=false means no pseudo-terminal, which is cleaner for logs
            tty: Some(false),             // No TTY = clean logs without escape sequences
            open_stdin: Some(true),       // Keeps stdin open for commands
            attach_stdin: Some(true),     // Allows docker attach to send input
            attach_stdout: Some(true),    // Receive server output
            attach_stderr: Some(true),    // Receive errors
            stdin_once: Some(false),      // Prevents stdin closing after first attach
            ..Default::default()
        };

        let container = self
            .docker
            .create_container(
                Some(CreateContainerOptions { name, platform: None }),
                config,
            )
            .await?;

        tracing::info!("Created container {} with id {}", name, container.id);
        Ok(container.id)
    }

    pub async fn start_container(&self, id: &str) -> anyhow::Result<()> {
        self.docker
            .start_container(id, None::<StartContainerOptions<String>>)
            .await?;
        tracing::info!("Started container {}", id);
        Ok(())
    }

    pub async fn stop_container(&self, id: &str) -> anyhow::Result<()> {
        self.docker
            .stop_container(id, Some(StopContainerOptions { t: 10 }))
            .await?;
        tracing::info!("Stopped container {}", id);
        Ok(())
    }

    pub async fn restart_container(&self, id: &str) -> anyhow::Result<()> {
        self.docker.restart_container(id, None).await?;
        tracing::info!("Restarted container {}", id);
        Ok(())
    }

    pub async fn kill_container(&self, id: &str) -> anyhow::Result<()> {
        self.docker.kill_container::<String>(id, None).await?;
        tracing::info!("Killed container {}", id);
        Ok(())
    }

    /// Update container resource limits (memory, cpu, etc.)
    pub async fn update_container_resources(&self, id: &str, resources: &ContainerResources) -> anyhow::Result<()> {
        use bollard::container::UpdateContainerOptions;

        let memory_bytes = resources.memory_limit * 1024 * 1024;
        let swap_bytes = resources.swap_limit * 1024 * 1024;
        let cpu_period = 100000i64;
        let cpu_quota = (resources.cpu_limit * cpu_period as f64) as i64;

        let update_options = UpdateContainerOptions::<String> {
            memory: Some(memory_bytes),
            memory_swap: Some(memory_bytes + swap_bytes),
            cpu_period: Some(cpu_period),
            cpu_quota: Some(cpu_quota),
            blkio_weight: Some(resources.io_weight as u16),
            ..Default::default()
        };

        self.docker.update_container(id, update_options).await?;
        tracing::info!("Updated container {} resources", id);
        Ok(())
    }

    /// Send a command to the container's stdin using docker attach
    /// Requires container to be created with: tty=true, open_stdin=true, stdin_once=false
    pub async fn send_command(&self, id: &str, command: &str) -> anyhow::Result<()> {
        tracing::info!("Sending command to container {}: {}", id, command);

        let options = AttachContainerOptions::<String> {
            stdin: Some(true),
            stdout: Some(false),
            stderr: Some(false),
            stream: Some(true),
            ..Default::default()
        };

        let AttachContainerResults { mut input, .. } = self
            .docker
            .attach_container(id, Some(options))
            .await?;

        // Send command with newline
        let cmd_with_newline = format!("{}\n", command);
        input.write_all(cmd_with_newline.as_bytes()).await?;
        input.flush().await?;

        // Close the input to detach cleanly
        drop(input);

        tracing::info!("Command sent to container {}", id);
        Ok(())
    }

    /// Gracefully stop container by sending a stop command first, then force kill after timeout
    pub async fn graceful_stop(&self, id: &str, stop_command: Option<&str>, timeout_secs: u64) -> anyhow::Result<()> {
        // If a stop command is provided, try to send it first
        if let Some(cmd) = stop_command {
            tracing::info!("Sending stop command '{}' to container {}", cmd, id);

            // Try to send the stop command
            if let Err(e) = self.send_command(id, cmd).await {
                tracing::warn!("Failed to send stop command: {}, proceeding with docker stop", e);
            } else {
                // Wait for container to stop gracefully
                let start = std::time::Instant::now();
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                    // Check if container is still running
                    match self.get_container(id).await {
                        Ok(info) => {
                            let state = info.state.to_lowercase();
                            if state != "running" {
                                tracing::info!("Container {} stopped gracefully", id);
                                return Ok(());
                            }
                        }
                        Err(_) => {
                            // Container might be gone
                            tracing::info!("Container {} no longer exists", id);
                            return Ok(());
                        }
                    }

                    // Check timeout
                    if start.elapsed().as_secs() >= timeout_secs {
                        tracing::warn!("Container {} did not stop within {} seconds, force killing", id, timeout_secs);
                        break;
                    }
                }
            }
        }

        // Force stop/kill the container
        if let Err(e) = self.docker.stop_container(id, Some(StopContainerOptions { t: 5 })).await {
            tracing::warn!("Stop failed, trying kill: {}", e);
            self.docker.kill_container::<String>(id, None).await?;
        }

        tracing::info!("Stopped container {}", id);
        Ok(())
    }

    pub async fn remove_container(&self, id: &str) -> anyhow::Result<()> {
        self.docker
            .remove_container(
                id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await?;
        tracing::info!("Removed container {}", id);
        Ok(())
    }

    pub async fn list_containers(&self) -> anyhow::Result<Vec<ContainerInfo>> {
        let options = ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        };

        let containers = self.docker.list_containers(Some(options)).await?;

        Ok(containers
            .into_iter()
            .map(|c| ContainerInfo {
                id: c.id.unwrap_or_default(),
                name: c
                    .names
                    .unwrap_or_default()
                    .first()
                    .cloned()
                    .unwrap_or_default()
                    .trim_start_matches('/')
                    .to_string(),
                image: c.image.unwrap_or_default(),
                status: c.status.unwrap_or_default(),
                state: c.state.unwrap_or_default(),
            })
            .collect())
    }

    pub async fn get_container(&self, id: &str) -> anyhow::Result<ContainerInfo> {
        let info = self.docker.inspect_container(id, None).await?;

        Ok(ContainerInfo {
            id: info.id.unwrap_or_default(),
            name: info
                .name
                .unwrap_or_default()
                .trim_start_matches('/')
                .to_string(),
            image: info.config.and_then(|c| c.image).unwrap_or_default(),
            status: info
                .state
                .as_ref()
                .and_then(|s| s.status)
                .map(|s| format!("{:?}", s))
                .unwrap_or_default(),
            state: info
                .state
                .and_then(|s| s.status)
                .map(|s| format!("{:?}", s))
                .unwrap_or_default(),
        })
    }


    pub fn stream_logs(&self, id: &str, tx: broadcast::Sender<String>) {
        let docker = self.docker.clone();
        let id = id.to_string();

        tokio::spawn(async move {
            tracing::info!("Starting log stream for container: {}", id);

            // Get historical logs first (last 500 lines)
            let historical_options = LogsOptions::<String> {
                follow: false,
                stdout: true,
                stderr: true,
                tail: "500".to_string(),
                timestamps: false,
                ..Default::default()
            };

            let mut historical_stream = docker.logs(&id, Some(historical_options));
            let mut log_count = 0;

            while let Some(result) = historical_stream.next().await {
                match result {
                    Ok(log) => {
                        let text = match log {
                            LogOutput::StdOut { message } => {
                                String::from_utf8_lossy(&message).trim_end().to_string()
                            }
                            LogOutput::StdErr { message } => {
                                format!("\x1b[31m{}\x1b[0m", String::from_utf8_lossy(&message).trim_end())
                            }
                            _ => continue,
                        };

                        if text.is_empty() {
                            continue;
                        }

                        log_count += 1;
                        if tx.send(text).is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(format!("\x1b[31m[Error] Failed to get logs: {}\x1b[0m", e));
                        return;
                    }
                }
            }

            tracing::info!("Sent {} historical log lines for container {}", log_count, id);
            let _ = tx.send(format!("\x1b[33m--- {} historical log lines loaded ---\x1b[0m", log_count));

            // Now follow new logs (stream in real-time)
            let follow_options = LogsOptions::<String> {
                follow: true,
                stdout: true,
                stderr: true,
                tail: "0".to_string(),
                timestamps: false,
                ..Default::default()
            };

            let mut stream = docker.logs(&id, Some(follow_options));

            while let Some(result) = stream.next().await {
                match result {
                    Ok(log) => {
                        let text = match log {
                            LogOutput::StdOut { message } => {
                                String::from_utf8_lossy(&message).trim_end().to_string()
                            }
                            LogOutput::StdErr { message } => {
                                format!("\x1b[31m{}\x1b[0m", String::from_utf8_lossy(&message).trim_end())
                            }
                            _ => continue,
                        };

                        if text.is_empty() {
                            continue;
                        }

                        if tx.send(text).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Log stream error: {}", e);
                        let _ = tx.send(format!("\x1b[33m[System] Container stopped or log stream ended\x1b[0m"));
                        break;
                    }
                }
            }
        });
    }


    pub async fn get_container_stats(&self, id: &str) -> anyhow::Result<ContainerStats> {
        let options = StatsOptions {
            stream: false,
            one_shot: true,
        };

        let mut stream = self.docker.stats(id, Some(options));

        if let Some(result) = stream.next().await {
            let stats = result?;

            // Calculate CPU percentage
            let cpu_delta = stats.cpu_stats.cpu_usage.total_usage
                .saturating_sub(stats.precpu_stats.cpu_usage.total_usage);
            let system_delta = stats.cpu_stats.system_cpu_usage
                .unwrap_or(0)
                .saturating_sub(stats.precpu_stats.system_cpu_usage.unwrap_or(0));
            let num_cpus = stats.cpu_stats.online_cpus.unwrap_or(1) as f64;

            let cpu_percent = if system_delta > 0 && cpu_delta > 0 {
                (cpu_delta as f64 / system_delta as f64) * num_cpus * 100.0
            } else {
                0.0
            };

            // Memory usage
            let memory_usage = stats.memory_stats.usage.unwrap_or(0);
            let memory_limit = stats.memory_stats.limit.unwrap_or(1);
            let memory_percent = (memory_usage as f64 / memory_limit as f64) * 100.0;

            // Network stats
            let (network_rx, network_tx) = stats.networks.as_ref()
                .map(|networks| {
                    networks.values().fold((0u64, 0u64), |(rx, tx), net| {
                        (rx + net.rx_bytes, tx + net.tx_bytes)
                    })
                })
                .unwrap_or((0, 0));

            // Block I/O stats
            let (block_read, block_write) = stats.blkio_stats.io_service_bytes_recursive
                .as_ref()
                .map(|entries| {
                    let read = entries.iter()
                        .filter(|e| e.op.as_str() == "read" || e.op.as_str() == "Read")
                        .map(|e| e.value)
                        .sum();
                    let write = entries.iter()
                        .filter(|e| e.op.as_str() == "write" || e.op.as_str() == "Write")
                        .map(|e| e.value)
                        .sum();
                    (read, write)
                })
                .unwrap_or((0, 0));

            return Ok(ContainerStats {
                cpu_percent,
                memory_usage,
                memory_limit,
                memory_percent,
                network_rx,
                network_tx,
                block_read,
                block_write,
            });
        }

        Err(anyhow::anyhow!("No stats available"))
    }

    pub fn stream_container_stats(&self, id: &str, tx: broadcast::Sender<String>) {
        let docker = self.docker.clone();
        let id = id.to_string();

        tokio::spawn(async move {
            let options = StatsOptions {
                stream: true,
                one_shot: false,
            };

            let mut stream = docker.stats(&id, Some(options));

            while let Some(result) = stream.next().await {
                match result {
                    Ok(stats) => {
                        // Calculate CPU percentage
                        let cpu_delta = stats.cpu_stats.cpu_usage.total_usage
                            .saturating_sub(stats.precpu_stats.cpu_usage.total_usage);
                        let system_delta = stats.cpu_stats.system_cpu_usage
                            .unwrap_or(0)
                            .saturating_sub(stats.precpu_stats.system_cpu_usage.unwrap_or(0));
                        let num_cpus = stats.cpu_stats.online_cpus.unwrap_or(1) as f64;

                        let cpu_percent = if system_delta > 0 && cpu_delta > 0 {
                            (cpu_delta as f64 / system_delta as f64) * num_cpus * 100.0
                        } else {
                            0.0
                        };

                        // Memory usage
                        let memory_usage = stats.memory_stats.usage.unwrap_or(0);
                        let memory_limit = stats.memory_stats.limit.unwrap_or(1);
                        let memory_percent = (memory_usage as f64 / memory_limit as f64) * 100.0;

                        // Network stats
                        let (network_rx, network_tx) = stats.networks.as_ref()
                            .map(|networks| {
                                networks.values().fold((0u64, 0u64), |(rx, tx), net| {
                                    (rx + net.rx_bytes, tx + net.tx_bytes)
                                })
                            })
                            .unwrap_or((0, 0));

                        // Block I/O stats
                        let (block_read, block_write) = stats.blkio_stats.io_service_bytes_recursive
                            .as_ref()
                            .map(|entries| {
                                let read = entries.iter()
                                    .filter(|e| e.op.as_str() == "read" || e.op.as_str() == "Read")
                                    .map(|e| e.value)
                                    .sum();
                                let write = entries.iter()
                                    .filter(|e| e.op.as_str() == "write" || e.op.as_str() == "Write")
                                    .map(|e| e.value)
                                    .sum();
                                (read, write)
                            })
                            .unwrap_or((0, 0));

                        let container_stats = ContainerStats {
                            cpu_percent,
                            memory_usage,
                            memory_limit,
                            memory_percent,
                            network_rx,
                            network_tx,
                            block_read,
                            block_write,
                        };

                        let json = serde_json::to_string(&container_stats).unwrap_or_default();
                        if tx.send(json).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Stats stream error: {}", e);
                        break;
                    }
                }
            }
        });
    }
}

