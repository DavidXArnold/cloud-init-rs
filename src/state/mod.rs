//! Instance state management for cloud-init
//!
//! Manages the /var/lib/cloud directory structure including:
//! - Instance tracking (current vs previous)
//! - Semaphore files for module execution control
//! - Cached data and status

pub mod paths;
pub mod semaphore;

pub use paths::CloudPaths;
pub use semaphore::{Frequency, SemaphoreManager};

use crate::CloudInitError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use tracing::{debug, info};

/// Instance state manager
#[derive(Debug)]
pub struct InstanceState {
    /// Cloud paths configuration
    paths: CloudPaths,
    /// Current instance ID (if known)
    instance_id: Option<String>,
    /// Semaphore manager (initialized when instance ID is set)
    semaphores: Option<SemaphoreManager>,
}

/// Status of cloud-init execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudInitStatus {
    /// Current status (running, done, error)
    pub status: String,
    /// Whether boot is finished
    pub boot_finished: bool,
    /// Current stage being executed
    pub stage: Option<String>,
    /// Error message if any
    pub error: Option<String>,
    /// Datasource name
    pub datasource: Option<String>,
}

impl Default for CloudInitStatus {
    fn default() -> Self {
        Self {
            status: "not-started".to_string(),
            boot_finished: false,
            stage: None,
            error: None,
            datasource: None,
        }
    }
}

impl Default for InstanceState {
    fn default() -> Self {
        Self::new()
    }
}

impl InstanceState {
    /// Create a new instance state manager with default paths
    pub fn new() -> Self {
        Self {
            paths: CloudPaths::new(),
            instance_id: None,
            semaphores: None,
        }
    }

    /// Create with custom paths (useful for testing)
    pub fn with_paths(paths: CloudPaths) -> Self {
        Self {
            paths,
            instance_id: None,
            semaphores: None,
        }
    }

    /// Get the paths configuration
    pub fn paths(&self) -> &CloudPaths {
        &self.paths
    }

    /// Get the current instance ID
    pub fn instance_id(&self) -> Option<&str> {
        self.instance_id.as_deref()
    }

    /// Get the semaphore manager (requires instance ID to be set)
    pub fn semaphores(&self) -> Option<&SemaphoreManager> {
        self.semaphores.as_ref()
    }

    /// Initialize the cloud-init directory structure
    pub async fn initialize(&mut self) -> Result<(), CloudInitError> {
        info!("Initializing cloud-init state directories");

        // Create base directories
        fs::create_dir_all(self.paths.data_dir()).await?;
        fs::create_dir_all(self.paths.instances_dir()).await?;
        fs::create_dir_all(self.paths.scripts_per_boot()).await?;
        fs::create_dir_all(self.paths.scripts_per_instance()).await?;
        fs::create_dir_all(self.paths.scripts_per_once()).await?;
        fs::create_dir_all(self.paths.seed_dir()).await?;

        debug!(
            "Created cloud-init directories under {}",
            self.paths.base.display()
        );
        Ok(())
    }

    /// Set the current instance ID and initialize instance-specific state
    pub async fn set_instance_id(&mut self, instance_id: &str) -> Result<bool, CloudInitError> {
        info!("Setting instance ID: {}", instance_id);

        // Check if this is a new instance
        let is_new_instance = self.check_instance_change(instance_id).await?;

        // Create instance directory
        let instance_dir = self.paths.instance_dir(instance_id);
        fs::create_dir_all(&instance_dir).await?;

        // Create sem directory
        let sem_dir = self.paths.sem_dir(instance_id);
        fs::create_dir_all(&sem_dir).await?;

        // Update instance symlink
        self.update_instance_link(instance_id).await?;

        // Save instance ID to cache
        fs::write(self.paths.cached_instance_id(), instance_id).await?;

        // Initialize semaphore manager
        self.semaphores = Some(SemaphoreManager::new(sem_dir, self.paths.data_dir()));
        self.instance_id = Some(instance_id.to_string());

        if is_new_instance {
            info!("New instance detected: {}", instance_id);
        }

        Ok(is_new_instance)
    }

    /// Check if the instance has changed
    async fn check_instance_change(&self, new_id: &str) -> Result<bool, CloudInitError> {
        let cached_path = self.paths.cached_instance_id();

        if cached_path.exists() {
            let cached_id = fs::read_to_string(&cached_path).await?;
            let cached_id = cached_id.trim();

            if cached_id != new_id {
                // Save previous instance ID
                fs::write(self.paths.previous_instance_id(), cached_id).await?;
                return Ok(true);
            }
            return Ok(false);
        }

        Ok(true) // No cached ID means new instance
    }

    /// Update the /var/lib/cloud/instance symlink
    async fn update_instance_link(&self, instance_id: &str) -> Result<(), CloudInitError> {
        let link_path = self.paths.instance_link();
        let target = self.paths.instance_dir(instance_id);

        // Remove existing symlink if present
        if link_path.exists() || link_path.is_symlink() {
            fs::remove_file(&link_path).await.ok();
        }

        // Create new symlink
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target, &link_path)?;
            debug!(
                "Created instance symlink: {} -> {}",
                link_path.display(),
                target.display()
            );
        }

        #[cfg(not(unix))]
        {
            // On non-Unix, just write the path to a file
            fs::write(&link_path, target.to_string_lossy().as_bytes()).await?;
        }

        Ok(())
    }

    /// Save user-data to instance directory
    pub async fn save_userdata(&self, data: &str) -> Result<(), CloudInitError> {
        if let Some(id) = &self.instance_id {
            let path = self.paths.user_data(id);
            fs::write(&path, data).await?;
            debug!("Saved user-data to {}", path.display());
        }
        Ok(())
    }

    /// Save vendor-data to instance directory
    pub async fn save_vendordata(&self, data: &str) -> Result<(), CloudInitError> {
        if let Some(id) = &self.instance_id {
            let path = self.paths.vendor_data(id);
            fs::write(&path, data).await?;
            debug!("Saved vendor-data to {}", path.display());
        }
        Ok(())
    }

    /// Save merged cloud-config to instance directory
    pub async fn save_cloud_config(&self, data: &str) -> Result<(), CloudInitError> {
        if let Some(id) = &self.instance_id {
            let path = self.paths.cloud_config(id);
            fs::write(&path, data).await?;
            debug!("Saved cloud-config to {}", path.display());
        }
        Ok(())
    }

    /// Save datasource identifier
    pub async fn save_datasource(&self, datasource: &str) -> Result<(), CloudInitError> {
        if let Some(id) = &self.instance_id {
            let path = self.paths.datasource_file(id);
            fs::write(&path, datasource).await?;
            debug!("Saved datasource identifier: {}", datasource);
        }
        Ok(())
    }

    /// Mark boot as finished
    pub async fn mark_boot_finished(&self) -> Result<(), CloudInitError> {
        if let Some(id) = &self.instance_id {
            let path = self.paths.boot_finished(id);
            let timestamp = format!(
                "{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            );
            fs::write(&path, timestamp).await?;
            info!("Boot finished marker created");
        }
        Ok(())
    }

    /// Check if boot has finished
    pub fn is_boot_finished(&self) -> bool {
        if let Some(id) = &self.instance_id {
            self.paths.boot_finished(id).exists()
        } else {
            false
        }
    }

    /// Update status
    pub async fn update_status(&self, status: &CloudInitStatus) -> Result<(), CloudInitError> {
        let path = self.paths.status_file();
        let json = serde_json::to_string_pretty(status)?;
        fs::write(&path, json).await?;
        Ok(())
    }

    /// Read current status
    pub async fn read_status(&self) -> Result<CloudInitStatus, CloudInitError> {
        let path = self.paths.status_file();
        if path.exists() {
            let content = fs::read_to_string(&path).await?;
            let status: CloudInitStatus = serde_json::from_str(&content)?;
            Ok(status)
        } else {
            Ok(CloudInitStatus::default())
        }
    }

    /// Clean all cloud-init state (for testing or reset)
    pub async fn clean(&self, include_logs: bool) -> Result<(), CloudInitError> {
        info!("Cleaning cloud-init state");

        // Remove all instance directories
        if self.paths.instances_dir().exists() {
            fs::remove_dir_all(self.paths.instances_dir()).await?;
        }

        // Remove instance symlink
        let link = self.paths.instance_link();
        if link.exists() || link.is_symlink() {
            fs::remove_file(&link).await.ok();
        }

        // Remove data directory
        if self.paths.data_dir().exists() {
            fs::remove_dir_all(self.paths.data_dir()).await?;
        }

        if include_logs {
            // Remove log files (typically /var/log/cloud-init*.log)
            let log_patterns = ["/var/log/cloud-init.log", "/var/log/cloud-init-output.log"];
            for pattern in log_patterns {
                let path = Path::new(pattern);
                if path.exists() {
                    fs::remove_file(path).await.ok();
                }
            }
        }

        info!("Cloud-init state cleaned");
        Ok(())
    }

    /// Load cached instance ID from disk
    pub async fn load_cached_instance_id(&mut self) -> Result<Option<String>, CloudInitError> {
        let path = self.paths.cached_instance_id();
        if path.exists() {
            let id = fs::read_to_string(&path).await?;
            let id = id.trim().to_string();
            if !id.is_empty() {
                self.instance_id = Some(id.clone());

                // Initialize semaphore manager
                let sem_dir = self.paths.sem_dir(&id);
                self.semaphores = Some(SemaphoreManager::new(sem_dir, self.paths.data_dir()));

                return Ok(Some(id));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_state() -> (InstanceState, TempDir) {
        let temp = TempDir::new().unwrap();
        let paths = CloudPaths::with_base(temp.path());
        let state = InstanceState::with_paths(paths);
        (state, temp)
    }

    #[tokio::test]
    async fn test_initialize() {
        let (mut state, temp) = create_test_state().await;
        state.initialize().await.unwrap();

        assert!(temp.path().join("data").exists());
        assert!(temp.path().join("instances").exists());
        assert!(temp.path().join("scripts/per-boot").exists());
    }

    #[tokio::test]
    async fn test_set_instance_id() {
        let (mut state, temp) = create_test_state().await;
        state.initialize().await.unwrap();

        let is_new = state.set_instance_id("i-12345").await.unwrap();
        assert!(is_new);
        assert_eq!(state.instance_id(), Some("i-12345"));

        // Instance directory should exist
        assert!(temp.path().join("instances/i-12345").exists());
        assert!(temp.path().join("instances/i-12345/sem").exists());

        // Setting same ID again should not be "new"
        let is_new = state.set_instance_id("i-12345").await.unwrap();
        assert!(!is_new);
    }

    #[tokio::test]
    async fn test_instance_change() {
        let (mut state, temp) = create_test_state().await;
        state.initialize().await.unwrap();

        state.set_instance_id("i-old").await.unwrap();
        let is_new = state.set_instance_id("i-new").await.unwrap();

        assert!(is_new);
        assert!(temp.path().join("data/previous-instance-id").exists());

        let prev = fs::read_to_string(temp.path().join("data/previous-instance-id"))
            .await
            .unwrap();
        assert_eq!(prev.trim(), "i-old");
    }

    #[tokio::test]
    async fn test_save_userdata() {
        let (mut state, temp) = create_test_state().await;
        state.initialize().await.unwrap();
        state.set_instance_id("i-test").await.unwrap();

        state
            .save_userdata("#cloud-config\nhostname: test")
            .await
            .unwrap();

        let content = fs::read_to_string(temp.path().join("instances/i-test/user-data.txt"))
            .await
            .unwrap();
        assert!(content.contains("hostname: test"));
    }

    #[tokio::test]
    async fn test_boot_finished() {
        let (mut state, _temp) = create_test_state().await;
        state.initialize().await.unwrap();
        state.set_instance_id("i-test").await.unwrap();

        assert!(!state.is_boot_finished());

        state.mark_boot_finished().await.unwrap();

        assert!(state.is_boot_finished());
    }

    #[tokio::test]
    async fn test_status() {
        let (mut state, _temp) = create_test_state().await;
        state.initialize().await.unwrap();

        let mut status = CloudInitStatus::default();
        status.status = "running".to_string();
        status.stage = Some("config".to_string());

        state.update_status(&status).await.unwrap();

        let loaded = state.read_status().await.unwrap();
        assert_eq!(loaded.status, "running");
        assert_eq!(loaded.stage, Some("config".to_string()));
    }

    #[tokio::test]
    async fn test_clean() {
        let (mut state, temp) = create_test_state().await;
        state.initialize().await.unwrap();
        state.set_instance_id("i-test").await.unwrap();

        state.clean(false).await.unwrap();

        assert!(!temp.path().join("instances").exists());
        assert!(!temp.path().join("data").exists());
    }
}
