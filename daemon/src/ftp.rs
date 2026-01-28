use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use async_trait::async_trait;
use libunftp::auth::{AuthenticationError, Authenticator, Credentials, UserDetail};
use libunftp::storage::{StorageBackend, Fileinfo, Metadata, Result as StorageResult, Error as StorageError, ErrorKind as StorageErrorKind};
use tokio::io::AsyncSeekExt;

/// Stored FTP credentials (for persistence) - password is bcrypt hashed
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredFtpCredentials {
    pub username: String,
    pub password_hash: String,
    pub container_id: String,
}

/// All FTP credentials stored in a single file
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct FtpCredentialsStore {
    pub credentials: Vec<StoredFtpCredentials>,
}

/// Custom user type that includes home directory info
#[derive(Debug, Clone)]
pub struct RaptorUser {
    pub username: String,
    pub home_path: PathBuf,
    pub is_admin: bool,
}

impl UserDetail for RaptorUser {
    fn account_enabled(&self) -> bool {
        true
    }
}

impl std::fmt::Display for RaptorUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.username)
    }
}

/// FTP user credentials stored in memory (includes plain password for current session)
#[derive(Clone, Debug)]
pub struct FtpUser {
    pub username: String,
    pub password_hash: String,
    pub container_id: String,
    pub home_path: PathBuf,
    pub is_admin: bool,
}

/// Get the secure path for FTP credentials storage
fn get_ftp_credentials_path() -> PathBuf {
    let data_dir = std::env::var("DAEMON_DATA_DIR")
        .unwrap_or_else(|_| "/var/lib/raptor-daemon".to_string());
    PathBuf::from(data_dir).join("ftp_credentials.json")
}

/// FTP Server state
#[derive(Debug)]
pub struct FtpServerState {
    pub users: Arc<dashmap::DashMap<String, FtpUser>>,
    pub base_path: PathBuf,
}

impl FtpServerState {
    pub fn new(base_path: &str) -> Self {
        let state = Self {
            users: Arc::new(dashmap::DashMap::new()),
            base_path: PathBuf::from(base_path),
        };

        // Load persisted credentials on startup
        state.load_all_credentials();

        state
    }

    /// Load all FTP credentials from secure storage file
    pub fn load_all_credentials(&self) {
        let creds_path = get_ftp_credentials_path();

        if !creds_path.exists() {
            tracing::info!("No FTP credentials file found at {:?}, starting fresh", creds_path);
            return;
        }

        match std::fs::read_to_string(&creds_path) {
            Ok(contents) => {
                match serde_json::from_str::<FtpCredentialsStore>(&contents) {
                    Ok(store) => {
                        let mut loaded = 0;
                        for creds in store.credentials {
                            self.add_user_from_stored(&creds);
                            loaded += 1;
                        }
                        tracing::info!("Loaded FTP credentials for {} containers from {:?}", loaded, creds_path);
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse FTP credentials file: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to read FTP credentials file: {}", e);
            }
        }
    }

    /// Add user from stored credentials (password is already hashed)
    fn add_user_from_stored(&self, creds: &StoredFtpCredentials) {
        let home_path = self.base_path.join("volumes").join(&creds.container_id);

        // Create the home directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&home_path) {
            tracing::warn!("Failed to create FTP home directory {:?}: {}", home_path, e);
        }

        self.users.insert(
            creds.username.clone(),
            FtpUser {
                username: creds.username.clone(),
                password_hash: creds.password_hash.clone(),
                container_id: creds.container_id.clone(),
                home_path,
                is_admin: false,
            },
        );

        tracing::debug!("Loaded FTP user: {} for container: {}", creds.username, creds.container_id);
    }

    /// Save all FTP credentials to secure storage
    fn save_all_credentials(&self) {
        let creds_path = get_ftp_credentials_path();

        // Collect all non-admin users
        let credentials: Vec<StoredFtpCredentials> = self.users
            .iter()
            .filter(|u| !u.is_admin)
            .map(|u| StoredFtpCredentials {
                username: u.username.clone(),
                password_hash: u.password_hash.clone(),
                container_id: u.container_id.clone(),
            })
            .collect();

        let store = FtpCredentialsStore { credentials };

        // Ensure parent directory exists
        if let Some(parent) = creds_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::error!("Failed to create FTP credentials directory: {}", e);
                return;
            }
        }

        match serde_json::to_string_pretty(&store) {
            Ok(json) => {
                // Write with restricted permissions (owner read/write only)
                match std::fs::write(&creds_path, &json) {
                    Ok(_) => {
                        // Try to set file permissions to 0600 (Unix only)
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let _ = std::fs::set_permissions(&creds_path, std::fs::Permissions::from_mode(0o600));
                        }
                        tracing::debug!("Saved FTP credentials to {:?}", creds_path);
                    }
                    Err(e) => tracing::error!("Failed to write FTP credentials file: {}", e),
                }
            }
            Err(e) => tracing::error!("Failed to serialize FTP credentials: {}", e),
        }
    }

    /// Hash a password using bcrypt
    fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
        bcrypt::hash(password, bcrypt::DEFAULT_COST)
    }

    /// Verify a password against a bcrypt hash
    fn verify_password(password: &str, hash: &str) -> bool {
        bcrypt::verify(password, hash).unwrap_or(false)
    }

    /// Add a new FTP user with password hashing
    pub fn add_user(&self, username: &str, password: &str, container_id: &str) {
        let password_hash = match Self::hash_password(password) {
            Ok(hash) => hash,
            Err(e) => {
                tracing::error!("Failed to hash password for user {}: {}", username, e);
                return;
            }
        };

        let home_path = self.base_path.join("volumes").join(container_id);

        // Create the home directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&home_path) {
            tracing::warn!("Failed to create FTP home directory {:?}: {}", home_path, e);
        }

        self.users.insert(
            username.to_string(),
            FtpUser {
                username: username.to_string(),
                password_hash,
                container_id: container_id.to_string(),
                home_path,
                is_admin: false,
            },
        );

        // Save all credentials to disk
        self.save_all_credentials();

        tracing::info!("Added FTP user: {} for container: {} (jailed, password hashed)", username, container_id);
    }

    pub fn add_admin_user(&self, username: &str, password: &str) {
        let password_hash = match Self::hash_password(password) {
            Ok(hash) => hash,
            Err(e) => {
                tracing::error!("Failed to hash password for admin user {}: {}", username, e);
                return;
            }
        };

        let home_path = self.base_path.join("volumes");

        // Create the home directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&home_path) {
            tracing::warn!("Failed to create FTP admin home directory {:?}: {}", home_path, e);
        }

        self.users.insert(
            username.to_string(),
            FtpUser {
                username: username.to_string(),
                password_hash,
                container_id: "admin".to_string(),
                home_path,
                is_admin: true,
            },
        );

        tracing::info!("Added FTP admin user: {} (access to all volumes, password hashed)", username);
    }

    pub fn remove_user(&self, username: &str) {
        if self.users.remove(username).is_some() {
            // Save updated credentials to disk
            self.save_all_credentials();
            tracing::info!("Removed FTP user: {}", username);
        }
    }

    /// Remove all users for a specific container
    pub fn remove_container_users(&self, container_id: &str) {
        let users_to_remove: Vec<String> = self.users
            .iter()
            .filter(|u| u.container_id == container_id)
            .map(|u| u.username.clone())
            .collect();

        for username in users_to_remove {
            self.users.remove(&username);
        }

        // Save updated credentials to disk
        self.save_all_credentials();
        tracing::info!("Removed all FTP users for container: {}", container_id);
    }

    pub fn get_user(&self, username: &str) -> Option<FtpUser> {
        self.users.get(username).map(|u| u.clone())
    }

    /// Verify user credentials using bcrypt
    pub fn verify_user(&self, username: &str, password: &str) -> Option<FtpUser> {
        if let Some(user) = self.users.get(username) {
            if Self::verify_password(password, &user.password_hash) {
                return Some(user.clone());
            }
        }
        None
    }
}

/// Custom authenticator that returns RaptorUser with home directory
#[derive(Clone, Debug)]
pub struct RaptorAuthenticator {
    state: Arc<FtpServerState>,
}

impl RaptorAuthenticator {
    pub fn new(state: Arc<FtpServerState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl Authenticator<RaptorUser> for RaptorAuthenticator {
    async fn authenticate(
        &self,
        username: &str,
        creds: &Credentials,
    ) -> Result<RaptorUser, AuthenticationError> {
        tracing::info!("FTP auth attempt for user: {}", username);

        let password = match &creds.password {
            Some(p) => p.as_str(),
            None => return Err(AuthenticationError::BadPassword),
        };

        // Use bcrypt verification
        if let Some(user) = self.state.verify_user(username, password) {
            tracing::info!("FTP auth success for user: {} (admin: {}, home: {:?})",
                username, user.is_admin, user.home_path);
            return Ok(RaptorUser {
                username: user.username,
                home_path: user.home_path,
                is_admin: user.is_admin,
            });
        }

        tracing::warn!("FTP auth failed for user: {}", username);
        Err(AuthenticationError::BadPassword)
    }
}

/// Custom storage backend that jails users to their home directory
#[derive(Debug, Clone)]
pub struct JailedFilesystem {
    base_path: PathBuf,
}

/// Wrapper for std::fs::Metadata that implements libunftp's Metadata trait
#[derive(Debug)]
pub struct FtpMetadata(std::fs::Metadata);

impl Metadata for FtpMetadata {
    fn len(&self) -> u64 {
        self.0.len()
    }

    fn is_dir(&self) -> bool {
        self.0.is_dir()
    }

    fn is_file(&self) -> bool {
        self.0.is_file()
    }

    fn is_symlink(&self) -> bool {
        self.0.file_type().is_symlink()
    }

    fn modified(&self) -> StorageResult<SystemTime> {
        self.0.modified().map_err(|e| StorageError::new(StorageErrorKind::LocalError, e))
    }

    fn gid(&self) -> u32 {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            self.0.gid()
        }
        #[cfg(not(unix))]
        {
            0
        }
    }

    fn uid(&self) -> u32 {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            self.0.uid()
        }
        #[cfg(not(unix))]
        {
            0
        }
    }

    fn links(&self) -> u64 {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            self.0.nlink()
        }
        #[cfg(not(unix))]
        {
            1
        }
    }
}

impl JailedFilesystem {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Resolve a path relative to user's home directory
    fn resolve_path(&self, user: &RaptorUser, path: &std::path::Path) -> PathBuf {
        let clean_path = path.to_string_lossy()
            .trim_start_matches('/')
            .to_string();

        user.home_path.join(clean_path)
    }
}

#[async_trait]
impl StorageBackend<RaptorUser> for JailedFilesystem {
    type Metadata = FtpMetadata;

    fn supported_features(&self) -> u32 {
        libunftp::storage::FEATURE_RESTART
    }

    async fn metadata<P: AsRef<std::path::Path> + Send + std::fmt::Debug>(
        &self,
        user: &RaptorUser,
        path: P,
    ) -> StorageResult<Self::Metadata> {
        let full_path = self.resolve_path(user, path.as_ref());
        tracing::debug!("FTP metadata: {:?} -> {:?}", path.as_ref(), full_path);

        tokio::fs::metadata(&full_path).await
            .map(FtpMetadata)
            .map_err(|e| StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e))
    }

    async fn list<P: AsRef<std::path::Path> + Send + std::fmt::Debug>(
        &self,
        user: &RaptorUser,
        path: P,
    ) -> StorageResult<Vec<Fileinfo<std::path::PathBuf, Self::Metadata>>>
    where
        Self::Metadata: Metadata,
    {
        let full_path = self.resolve_path(user, path.as_ref());
        tracing::debug!("FTP list: {:?} -> {:?}", path.as_ref(), full_path);

        // Create directory if it doesn't exist
        if !full_path.exists() {
            tokio::fs::create_dir_all(&full_path).await.map_err(|e| {
                StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
            })?;
        }

        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(&full_path).await.map_err(|e| {
            StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
        })?;

        while let Some(entry) = dir.next_entry().await.map_err(|e| {
            StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
        })? {
            let metadata = entry.metadata().await.map_err(|e| {
                StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
            })?;

            entries.push(Fileinfo {
                path: entry.file_name().into(),
                metadata: FtpMetadata(metadata),
            });
        }

        Ok(entries)
    }

    async fn get<P: AsRef<std::path::Path> + Send + std::fmt::Debug>(
        &self,
        user: &RaptorUser,
        path: P,
        start_pos: u64,
    ) -> StorageResult<Box<dyn tokio::io::AsyncRead + Send + Sync + Unpin>> {
        let full_path = self.resolve_path(user, path.as_ref());
        tracing::debug!("FTP get: {:?} -> {:?}", path.as_ref(), full_path);

        let mut file = tokio::fs::File::open(&full_path).await.map_err(|e| {
            StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
        })?;

        if start_pos > 0 {
            file.seek(std::io::SeekFrom::Start(start_pos)).await.map_err(|e| {
                StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
            })?;
        }

        Ok(Box::new(file))
    }

    async fn put<
        P: AsRef<std::path::Path> + Send + std::fmt::Debug,
        R: tokio::io::AsyncRead + Send + Sync + Unpin + 'static,
    >(
        &self,
        user: &RaptorUser,
        input: R,
        path: P,
        start_pos: u64,
    ) -> StorageResult<u64> {
        let full_path = self.resolve_path(user, path.as_ref());
        tracing::debug!("FTP put: {:?} -> {:?}", path.as_ref(), full_path);

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
            })?;
        }

        let mut file = if start_pos > 0 {
            let mut f = tokio::fs::OpenOptions::new()
                .write(true)
                .open(&full_path)
                .await
                .map_err(|e| StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e))?;
            f.seek(std::io::SeekFrom::Start(start_pos)).await.map_err(|e| {
                StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
            })?;
            f
        } else {
            tokio::fs::File::create(&full_path).await.map_err(|e| {
                StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
            })?
        };

        let bytes = tokio::io::copy(&mut tokio::io::BufReader::new(input), &mut file).await.map_err(|e| {
            StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
        })?;

        Ok(bytes)
    }

    async fn del<P: AsRef<std::path::Path> + Send + std::fmt::Debug>(
        &self,
        user: &RaptorUser,
        path: P,
    ) -> StorageResult<()> {
        let full_path = self.resolve_path(user, path.as_ref());
        tracing::debug!("FTP del: {:?} -> {:?}", path.as_ref(), full_path);

        tokio::fs::remove_file(&full_path).await.map_err(|e| {
            StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
        })
    }

    async fn mkd<P: AsRef<std::path::Path> + Send + std::fmt::Debug>(
        &self,
        user: &RaptorUser,
        path: P,
    ) -> StorageResult<()> {
        let full_path = self.resolve_path(user, path.as_ref());
        tracing::debug!("FTP mkd: {:?} -> {:?}", path.as_ref(), full_path);

        tokio::fs::create_dir_all(&full_path).await.map_err(|e| {
            StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
        })
    }

    async fn rename<P: AsRef<std::path::Path> + Send + std::fmt::Debug>(
        &self,
        user: &RaptorUser,
        from: P,
        to: P,
    ) -> StorageResult<()> {
        let from_path = self.resolve_path(user, from.as_ref());
        let to_path = self.resolve_path(user, to.as_ref());
        tracing::debug!("FTP rename: {:?} -> {:?}", from_path, to_path);

        tokio::fs::rename(&from_path, &to_path).await.map_err(|e| {
            StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
        })
    }

    async fn rmd<P: AsRef<std::path::Path> + Send + std::fmt::Debug>(
        &self,
        user: &RaptorUser,
        path: P,
    ) -> StorageResult<()> {
        let full_path = self.resolve_path(user, path.as_ref());
        tracing::debug!("FTP rmd: {:?} -> {:?}", path.as_ref(), full_path);

        tokio::fs::remove_dir_all(&full_path).await.map_err(|e| {
            StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
        })
    }

    async fn cwd<P: AsRef<std::path::Path> + Send + std::fmt::Debug>(
        &self,
        user: &RaptorUser,
        path: P,
    ) -> StorageResult<()> {
        let full_path = self.resolve_path(user, path.as_ref());
        tracing::debug!("FTP cwd: {:?} -> {:?}", path.as_ref(), full_path);

        // Just verify the directory exists
        let meta = tokio::fs::metadata(&full_path).await.map_err(|e| {
            StorageError::new(StorageErrorKind::PermanentFileNotAvailable, e)
        })?;

        if !meta.is_dir() {
            return Err(StorageError::new(
                StorageErrorKind::PermanentFileNotAvailable,
                std::io::Error::new(std::io::ErrorKind::NotFound, "Not a directory"),
            ));
        }

        Ok(())
    }
}

/// Start the FTP server with jailed filesystem
pub async fn start_ftp_server(state: Arc<FtpServerState>, addr: &str, port: u16) -> anyhow::Result<()> {
    let bind_addr = format!("{}:{}", addr, port);
    tracing::info!("Starting FTP server on {} (with per-user jailing)", bind_addr);

    // Create base volumes directory
    let volumes_path = state.base_path.join("volumes");
    std::fs::create_dir_all(&volumes_path)?;

    let authenticator = Arc::new(RaptorAuthenticator::new(state.clone()));
    let base_path = state.base_path.clone();

    // Create server with custom storage and authenticator
    // The authenticator determines the User type (RaptorUser)
    let server = libunftp::ServerBuilder::<JailedFilesystem, RaptorUser>::with_authenticator(
        Box::new(move || JailedFilesystem::new(base_path.clone())),
        authenticator,
    )
    .greeting("Welcome to Raptor FTP Server")
    .passive_ports(50000..=50100)
    .build()
    .unwrap();

    server.listen(bind_addr).await?;

    Ok(())
}

/// Credentials returned when creating FTP access
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FtpCredentials {
    pub user: String,
    pub pass: String,
    pub host: String,
    pub port: u16,
}

/// Create FTP access for a container
pub fn create_ftp_access(
    state: &Arc<FtpServerState>,
    container_id: &str,
    password: &str,
) -> FtpCredentials {
    // Username is first 8 characters of container UUID
    let username = container_id.replace("-", "")[..8].to_string();

    // Add user to FTP server state (jailed to their container's volume)
    state.add_user(&username, password, container_id);

    let host = std::env::var("FTP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("FTP_PORT")
        .unwrap_or_else(|_| "2121".to_string())
        .parse()
        .unwrap_or(2121);

    FtpCredentials {
        user: username,
        pass: password.to_string(),
        host,
        port,
    }
}

/// Create admin FTP access (access to all volumes)
pub fn create_admin_ftp_access(
    state: &Arc<FtpServerState>,
    username: &str,
    password: &str,
) -> FtpCredentials {
    state.add_admin_user(username, password);

    let host = std::env::var("FTP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("FTP_PORT")
        .unwrap_or_else(|_| "2121".to_string())
        .parse()
        .unwrap_or(2121);

    FtpCredentials {
        user: username.to_string(),
        pass: password.to_string(),
        host,
        port,
    }
}

