use bollard::container::{
    AttachContainerOptions, AttachContainerResults,
    Config, CreateContainerOptions, ListContainersOptions, LogOutput, LogsOptions,
    RemoveContainerOptions, StartContainerOptions, StatsOptions, StopContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::network::CreateNetworkOptions;
use bollard::Docker;
use futures_util::StreamExt;
use std::collections::HashMap;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;

use crate::models::{ContainerInfo, ContainerResources, ContainerStats};

pub const RAPTOR_NETWORK: &str = "raptord_internal";

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

        let manager = Self { docker };

        manager.ensure_network().await?;

        Ok(manager)
    }

    async fn ensure_network(&self) -> anyhow::Result<()> {

        match self.docker.inspect_network::<String>(RAPTOR_NETWORK, None).await {
            Ok(_) => {
                tracing::info!("Network {} already exists", RAPTOR_NETWORK);
                return Ok(());
            }
            Err(_) => {
                tracing::info!("Creating network {}", RAPTOR_NETWORK);
            }
        }

        let config = CreateNetworkOptions {
            name: RAPTOR_NETWORK,
            driver: "bridge",
            ..Default::default()
        };

        self.docker.create_network(config).await?;
        tracing::info!("Created network {}", RAPTOR_NETWORK);
        Ok(())
    }

    pub async fn create_container_with_resources(
        &self,
        name: &str,
        image: &str,
        startup_script: Option<&str>,
        port_bindings: Option<HashMap<String, Vec<bollard::service::PortBinding>>>,
        resources: &ContainerResources,
        restart_policy_name: &str,
        tty: bool,
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

        let mut env_vars: Vec<String> = vec![
            "HOME=/home/container".to_string(),
            "USER=container".to_string(),
        ];

        let (entrypoint, cmd) = if let Some(s) = startup_script {

            env_vars.push(format!("STARTUP={}", s));

            (
                Some(vec!["/bin/bash".to_string()]),
                Some(vec!["-c".to_string(), s.to_string()])
            )
        } else {
            (None, None)
        };

        let memory_bytes = resources.memory_limit * 1024 * 1024;
        let swap_bytes = resources.swap_limit * 1024 * 1024;
        let cpu_period = 100000i64;
        let cpu_quota = (resources.cpu_limit * cpu_period as f64) as i64;

        let restart_policy = match restart_policy_name.to_lowercase().as_str() {
            "no" | "none" => bollard::service::RestartPolicy {
                name: Some(bollard::service::RestartPolicyNameEnum::NO),
                maximum_retry_count: None,
            },
            "always" => bollard::service::RestartPolicy {
                name: Some(bollard::service::RestartPolicyNameEnum::ALWAYS),
                maximum_retry_count: None,
            },
            "on-failure" | "onfailure" => bollard::service::RestartPolicy {
                name: Some(bollard::service::RestartPolicyNameEnum::ON_FAILURE),
                maximum_retry_count: Some(5),
            },
            _ => bollard::service::RestartPolicy {

                name: Some(bollard::service::RestartPolicyNameEnum::UNLESS_STOPPED),
                maximum_retry_count: None,
            },
        };
        tracing::debug!("Using restart policy: {} for container {}", restart_policy_name, name);

        let base_path = std::env::var("FTP_BASE_PATH")
            .unwrap_or_else(|_| std::env::var("SFTP_BASE_PATH")
                .unwrap_or_else(|_| "/data/raptor".into()));
        let volume_path = format!("{}/volumes/{}", base_path, name);

        if let Err(e) = tokio::fs::create_dir_all(&volume_path).await {
            tracing::warn!("Failed to create volume directory {}: {}", volume_path, e);
        }

        #[cfg(unix)]
        {
            let chmod_result = tokio::process::Command::new("chmod")
                .args(["-R", "777", &volume_path])
                .output()
                .await;
            match chmod_result {
                Ok(output) if output.status.success() => {
                    tracing::debug!("Set permissions 777 on {}", volume_path);
                }
                Ok(output) => {
                    tracing::warn!(
                        "Failed to chmod volume directory {}: {}",
                        volume_path,
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to run chmod on volume directory {}: {}", volume_path, e);
                }
            }
        }

        let machine_id_path = format!("{}/.machine-id", volume_path);
        if !std::path::Path::new(&machine_id_path).exists() {

            use rand::Rng;
            let machine_id: String = (0..32)
                .map(|_| format!("{:x}", rand::thread_rng().gen::<u8>() % 16))
                .collect();
            if let Err(e) = tokio::fs::write(&machine_id_path, format!("{}\n", machine_id)).await {
                tracing::warn!("Failed to create machine-id file: {}", e);
            } else {
                tracing::debug!("Created persistent machine-id file at {}", machine_id_path);
            }
        }

        let binds = vec![
            format!("{}:/home/container:rw", volume_path),
            format!("{}:/etc/machine-id:ro", machine_id_path),
        ];

        #[cfg(unix)]
        let user_spec = {
            use std::os::unix::fs::MetadataExt;

            if let Ok(metadata) = std::fs::metadata(&volume_path) {
                let uid = metadata.uid();
                let gid = metadata.gid();
                Some(format!("{}:{}", uid, gid))
            } else {

                let uid = unsafe { libc::getuid() };
                let gid = unsafe { libc::getgid() };
                Some(format!("{}:{}", uid, gid))
            }
        };
        #[cfg(not(unix))]
        let user_spec: Option<String> = None;

        let host_config = bollard::service::HostConfig {
            port_bindings: port_bindings.as_ref().map(|pb| pb.iter().map(|(k, v)| (k.clone(), Some(v.clone()))).collect()),
            memory: Some(memory_bytes),
            memory_swap: Some(memory_bytes + swap_bytes),
            cpu_period: Some(cpu_period),
            cpu_quota: Some(cpu_quota),
            blkio_weight: Some(resources.io_weight as u16),
            restart_policy: Some(restart_policy),
            binds: Some(binds),
            network_mode: Some(RAPTOR_NETWORK.to_string()),
            ..Default::default()
        };

        let exposed_port_keys: Vec<String> = port_bindings
            .as_ref()
            .map(|pb| pb.keys().cloned().collect())
            .unwrap_or_default();

        let config = Config {
            image: Some(image),

            entrypoint: entrypoint.as_ref().map(|v| v.iter().map(|s| s.as_str()).collect()),
            cmd: cmd.as_ref().map(|v| v.iter().map(|s| s.as_str()).collect()),
            env: Some(env_vars.iter().map(|s| s.as_str()).collect()),
            host_config: Some(host_config),
            working_dir: Some("/home/container"),

            user: user_spec.as_deref(),
            exposed_ports: if exposed_port_keys.is_empty() {
                None
            } else {
                Some(exposed_port_keys.iter().map(|k| (k.as_str(), HashMap::new())).collect())
            },

            tty: Some(tty),
            open_stdin: Some(true),
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            stdin_once: Some(false),
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

        let cmd_with_newline = format!("{}\n", command);
        input.write_all(cmd_with_newline.as_bytes()).await?;
        input.flush().await?;

        drop(input);

        tracing::info!("Command sent to container {}", id);
        Ok(())
    }

    pub async fn run_install_script(&self, id: &str, script: &str, env: &std::collections::HashMap<String, String>) -> anyhow::Result<()> {
        use bollard::exec::{CreateExecOptions, StartExecResults};
        use futures_util::StreamExt;

        tracing::info!("Running install script in container {}", id);

        let mut env_vars: Vec<String> = env.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        env_vars.push("PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string());

        let setup_script = r#"
            mkdir -p /home/container
            cd /home/container
            # Install curl and jq if not present (for Alpine-based images)
            if command -v apk &> /dev/null; then
                apk add --no-cache curl jq bash 2>/dev/null || true
            fi
            # For Debian/Ubuntu based images
            if command -v apt-get &> /dev/null; then
                apt-get update -qq && apt-get install -y -qq curl jq 2>/dev/null || true
            fi
        "#;

        let setup_exec = self.docker.create_exec(
            id,
            CreateExecOptions {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(vec!["sh", "-c", setup_script]),
                env: Some(env_vars.iter().map(|s| s.as_str()).collect()),
                working_dir: Some("/"),
                user: Some("root"),
                ..Default::default()
            }
        ).await?;

        let setup_result = self.docker.start_exec(&setup_exec.id, None).await?;
        if let StartExecResults::Attached { mut output, .. } = setup_result {
            while let Some(msg) = output.next().await {
                if let Ok(m) = msg {
                    let text = m.to_string();
                    if !text.trim().is_empty() {
                        tracing::debug!("[setup] {}", text.trim());
                    }
                }
            }
        }

        let exec = self.docker.create_exec(
            id,
            CreateExecOptions {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(vec!["bash", "-c", script]),
                env: Some(env_vars.iter().map(|s| s.as_str()).collect()),
                working_dir: Some("/home/container"),
                user: Some("root"),
                ..Default::default()
            }
        ).await?;

        let start_result = self.docker.start_exec(&exec.id, None).await?;

        if let StartExecResults::Attached { mut output, .. } = start_result {
            while let Some(msg) = output.next().await {
                match msg {
                    Ok(m) => {
                        let text = m.to_string();
                        if !text.trim().is_empty() {
                            tracing::info!("[install] {}", text.trim());
                        }
                    }
                    Err(e) => {
                        tracing::error!("Install script error: {}", e);
                    }
                }
            }
        }

        let exec_inspect = self.docker.inspect_exec(&exec.id).await?;
        if let Some(exit_code) = exec_inspect.exit_code {
            if exit_code != 0 {
                tracing::warn!("Install script exited with code {}", exit_code);
            } else {
                tracing::info!("Install script completed successfully");
            }
        }

        Ok(())
    }

    pub async fn run_install_in_temp_container(
        &self,
        container_name: &str,
        image: &str,
        script: &str,
        env: &std::collections::HashMap<String, String>,
    ) -> anyhow::Result<()> {
        self.run_install_in_temp_container_with_logs(container_name, image, script, env, None).await
    }

    pub async fn run_install_in_temp_container_with_logs(
        &self,
        container_name: &str,
        image: &str,
        script: &str,
        env: &std::collections::HashMap<String, String>,
        log_tx: Option<broadcast::Sender<String>>,
    ) -> anyhow::Result<()> {
        use bollard::container::{CreateContainerOptions, Config, LogsOptions, RemoveContainerOptions, WaitContainerOptions};
        use futures_util::StreamExt;

        let install_container_name = format!("{}-install", container_name);
        tracing::info!("=== Starting installation for container {} ===", container_name);
        tracing::info!("Install container name: {}", install_container_name);
        tracing::info!("Using image: {}", image);

        let env_vars: Vec<String> = env.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .chain(std::iter::once("PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string()))
            .collect();

        tracing::info!("Environment variables:");
        for (k, v) in env.iter() {
            tracing::info!("  {}={}", k, v);
        }

        let base_path = std::env::var("FTP_BASE_PATH")
            .unwrap_or_else(|_| std::env::var("SFTP_BASE_PATH")
                .unwrap_or_else(|_| "/data/raptor".into()));
        let volume_path = format!("{}/volumes/{}", base_path, container_name);
        tracing::info!("Volume path: {}", volume_path);

        if let Err(e) = tokio::fs::create_dir_all(&volume_path).await {
            tracing::warn!("Failed to create volume directory {}: {}", volume_path, e);
        }

        #[cfg(unix)]
        {
            let chmod_result = tokio::process::Command::new("chmod")
                .args(["-R", "777", &volume_path])
                .output()
                .await;
            match chmod_result {
                Ok(output) if output.status.success() => {
                    tracing::debug!("Set permissions 777 on {}", volume_path);
                }
                Ok(output) => {
                    tracing::warn!(
                        "Failed to chmod volume directory {}: {}",
                        volume_path,
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to run chmod on volume directory {}: {}", volume_path, e);
                }
            }
        }

        let binds = vec![format!("{}:/home/container:rw", volume_path)];

        #[cfg(unix)]
        let user_spec = {
            use std::os::unix::fs::MetadataExt;

            if let Ok(metadata) = std::fs::metadata(&volume_path) {
                let uid = metadata.uid();
                let gid = metadata.gid();
                Some(format!("{}:{}", uid, gid))
            } else {

                let uid = unsafe { libc::getuid() };
                let gid = unsafe { libc::getgid() };
                Some(format!("{}:{}", uid, gid))
            }
        };
        #[cfg(not(unix))]
        let install_user_spec: Option<String> = None;
        #[cfg(unix)]
        let install_user_spec = user_spec;

        let script_preview = if script.len() > 500 {
            format!("{}...(truncated)", &script[..500])
        } else {
            script.to_string()
        };
        tracing::info!("Install script:\n{}", script_preview);

        let full_script = format!(r#"
            echo "[Raptor Install] Starting installation..."
            echo "[Raptor Install] Working directory: $(pwd)"

            # Create /mnt/server symlink for Pterodactyl egg compatibility
            mkdir -p /mnt 2>/dev/null || true
            ln -sf /home/container /mnt/server 2>/dev/null || true

            cd /home/container
            echo "[Raptor Install] Running install script..."

            # Run the actual install script
            {}

            INSTALL_EXIT=$?
            echo "[Raptor Install] Install script finished with exit code: $INSTALL_EXIT"
            echo "[Raptor Install] Files in /home/container:"
            ls -la /home/container/ 2>/dev/null || echo "(empty)"
            exit $INSTALL_EXIT
        "#, script);

        let host_config = bollard::service::HostConfig {
            binds: Some(binds),
            auto_remove: Some(true),
            network_mode: Some(RAPTOR_NETWORK.to_string()),
            ..Default::default()
        };

        let config = Config {
            image: Some(image),
            entrypoint: Some(vec!["bash", "-c"]),
            cmd: Some(vec![&full_script]),
            host_config: Some(host_config),
            working_dir: Some("/home/container"),
            user: install_user_spec.as_deref(),
            env: Some(env_vars.iter().map(|s| s.as_str()).collect()),
            tty: Some(false),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        let _ = self.docker.remove_container(
            &install_container_name,
            Some(RemoveContainerOptions { force: true, ..Default::default() })
        ).await;

        let create_result = self.docker.create_container(
            Some(CreateContainerOptions { name: install_container_name.as_str(), platform: None }),
            config
        ).await?;

        tracing::info!("Created install container: {}", create_result.id);

        self.docker.start_container::<String>(&create_result.id, None).await?;
        tracing::info!("Install container started: {}", create_result.id);

        match self.docker.inspect_container(&create_result.id, None).await {
            Ok(info) => {
                let state = info.state.as_ref();
                let status = state.and_then(|s| s.status.as_ref()).map(|s| format!("{:?}", s)).unwrap_or_else(|| "unknown".to_string());
                let running = state.and_then(|s| s.running).unwrap_or(false);
                let exit_code = state.and_then(|s| s.exit_code).unwrap_or(-1);
                tracing::info!("Install container status: {}, running: {}, exit_code: {}", status, running, exit_code);

                if !running && exit_code != 0 {
                    tracing::error!("Install container exited immediately with code {}", exit_code);
                }
            }
            Err(e) => {
                tracing::warn!("Could not inspect install container: {}", e);
            }
        }

        tracing::info!("Starting log stream for install container...");

        let log_options = LogsOptions::<String> {
            follow: true,
            stdout: true,
            stderr: true,
            since: 0,
            timestamps: false,
            ..Default::default()
        };

        let mut logs = self.docker.logs(&create_result.id, Some(log_options));

        let wait_options = WaitContainerOptions { condition: "not-running" };
        let mut wait_stream = self.docker.wait_container(&create_result.id, Some(wait_options));

        let timeout = tokio::time::Duration::from_secs(300);
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed() > timeout {
                tracing::error!("Install script timed out after 5 minutes");
                if let Some(ref tx) = log_tx {
                    let _ = tx.send("\x1b[31m[Install] Installation timed out after 5 minutes\x1b[0m".to_string());
                }

                let _ = self.docker.kill_container::<String>(&create_result.id, None).await;
                break;
            }

            tokio::select! {
                log_msg = logs.next() => {
                    match log_msg {
                        Some(Ok(msg)) => {
                            let text = msg.to_string();
                            for line in text.lines() {
                                if !line.trim().is_empty() {
                                    tracing::info!("[install] {}", line.trim());

                                    if let Some(ref tx) = log_tx {
                                        let _ = tx.send(format!("\x1b[36m[Install]\x1b[0m {}", line.trim()));
                                    }
                                }
                            }
                        }
                        Some(Err(e)) => {
                            tracing::warn!("[install] Log stream error: {}", e);
                        }
                        None => {
                            tracing::info!("[install] Log stream ended");
                            break;
                        }
                    }
                }
                wait_result = wait_stream.next() => {
                    match wait_result {
                        Some(Ok(exit)) => {

                            if exit.status_code != 0 {
                                tracing::error!("=== Install script FAILED with exit code {} ===", exit.status_code);
                                if let Some(ref tx) = log_tx {
                                    let _ = tx.send(format!("\x1b[31m[Install] Installation FAILED with exit code {}\x1b[0m", exit.status_code));
                                }
                            } else {
                                tracing::info!("=== Install script completed successfully ===");
                                if let Some(ref tx) = log_tx {
                                    let _ = tx.send("\x1b[32m[Install] Installation completed successfully!\x1b[0m".to_string());
                                }
                            }
                            break;
                        }
                        Some(Err(e)) => {
                            tracing::error!("[install] Wait error: {}", e);
                            break;
                        }
                        None => break,
                    }
                }
            }
        }

        let _ = self.docker.remove_container(
            &install_container_name,
            Some(RemoveContainerOptions { force: true, ..Default::default() })
        ).await;

        Ok(())
    }
    pub async fn graceful_stop(&self, id: &str, timeout_secs: u64) -> anyhow::Result<()> {
        tracing::info!("Stopping container {} with {}s timeout", id, timeout_secs);

        let timeout: i64 = timeout_secs.try_into().unwrap_or(30);
        self.docker
            .stop_container(id, Some(StopContainerOptions { t: timeout }))
            .await?;

        tracing::info!("Container {} stopped", id);
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

    pub async fn cleanup_containers_by_name(&self, name: &str) -> anyhow::Result<u32> {
        let options = ListContainersOptions {
            all: true,
            filters: std::collections::HashMap::from([
                ("name".to_string(), vec![name.to_string()]),
            ]),
            ..Default::default()
        };

        let containers = self.docker.list_containers(Some(options)).await?;
        let mut removed = 0u32;

        for container in containers {
            if let Some(id) = container.id {

                let container_name = container.names
                    .unwrap_or_default()
                    .first()
                    .cloned()
                    .unwrap_or_default()
                    .trim_start_matches('/')
                    .to_string();

                if container_name == name {
                    tracing::info!("Cleaning up old container: {} ({})", container_name, id);

                    if let Err(e) = self.docker.remove_container(&id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await {
                        tracing::warn!("Failed to remove container {}: {}", id, e);
                    } else {
                        removed += 1;
                    }
                }
            }
        }

        if removed > 0 {
            tracing::info!("Cleaned up {} old container(s) with name '{}'", removed, name);
        }

        Ok(removed)
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

    pub fn stream_logs(&self, id: &str, tx: broadcast::Sender<String>, since: Option<String>) {
        let docker = self.docker.clone();
        let id = id.to_string();

        tokio::spawn(async move {
            tracing::info!("Starting log stream for container: {} (since: {:?})", id, since);

            // Parse since parameter (e.g., "10m" for 10 minutes, "1h" for 1 hour)
            let since_timestamp = since.and_then(|s| {
                let s = s.trim();
                if s.ends_with('m') {
                    s[..s.len()-1].parse::<i64>().ok().map(|mins| {
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64 - (mins * 60)
                    })
                } else if s.ends_with('h') {
                    s[..s.len()-1].parse::<i64>().ok().map(|hours| {
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64 - (hours * 3600)
                    })
                } else {
                    None
                }
            });

            let historical_options = LogsOptions::<String> {
                follow: false,
                stdout: true,
                stderr: true,
                tail: if since_timestamp.is_some() { "all".to_string() } else { "500".to_string() },
                timestamps: false,
                since: since_timestamp.unwrap_or(0),
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

                            LogOutput::Console { message } => {
                                String::from_utf8_lossy(&message).trim_end().to_string()
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

            tracing::debug!("Loaded {} historical log lines for container {}", log_count, id);

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

                            LogOutput::Console { message } => {
                                String::from_utf8_lossy(&message).trim_end().to_string()
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

            let memory_usage = stats.memory_stats.usage.unwrap_or(0);
            let memory_limit = stats.memory_stats.limit.unwrap_or(1);
            let memory_percent = (memory_usage as f64 / memory_limit as f64) * 100.0;

            let (network_rx, network_tx) = stats.networks.as_ref()
                .map(|networks| {
                    networks.values().fold((0u64, 0u64), |(rx, tx), net| {
                        (rx + net.rx_bytes, tx + net.tx_bytes)
                    })
                })
                .unwrap_or((0, 0));

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

                        let memory_usage = stats.memory_stats.usage.unwrap_or(0);
                        let memory_limit = stats.memory_stats.limit.unwrap_or(1);
                        let memory_percent = (memory_usage as f64 / memory_limit as f64) * 100.0;

                        let (network_rx, network_tx) = stats.networks.as_ref()
                            .map(|networks| {
                                networks.values().fold((0u64, 0u64), |(rx, tx), net| {
                                    (rx + net.rx_bytes, tx + net.tx_bytes)
                                })
                            })
                            .unwrap_or((0, 0));

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

