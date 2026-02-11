//! Semaphore file handling for cloud-init
//!
//! Semaphores control when modules run:
//! - per-instance: Run once per instance ID
//! - per-boot: Run every boot
//! - per-once: Run once ever (across all instances)

use crate::CloudInitError;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::debug;

/// Semaphore frequency - how often a module should run
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency {
    /// Run every boot
    PerBoot,
    /// Run once per instance ID
    PerInstance,
    /// Run once ever (even across instance changes)
    PerOnce,
    /// Always run (no semaphore)
    Always,
}

impl Frequency {
    /// Get the subdirectory name for this frequency
    pub fn subdir(&self) -> Option<&'static str> {
        match self {
            Self::PerBoot => None, // No semaphore needed
            Self::PerInstance => Some("per-instance"),
            Self::PerOnce => Some("per-once"),
            Self::Always => None,
        }
    }
}

impl std::fmt::Display for Frequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PerBoot => write!(f, "per-boot"),
            Self::PerInstance => write!(f, "per-instance"),
            Self::PerOnce => write!(f, "per-once"),
            Self::Always => write!(f, "always"),
        }
    }
}

/// Semaphore manager for a specific instance
#[derive(Debug, Clone)]
pub struct SemaphoreManager {
    /// Base semaphore directory (`/var/lib/cloud/instances/<id>/sem`)
    sem_dir: PathBuf,
    /// Data directory for per-once semaphores (`/var/lib/cloud/data`)
    data_dir: PathBuf,
}

impl SemaphoreManager {
    /// Create a new semaphore manager
    pub fn new(sem_dir: impl AsRef<Path>, data_dir: impl AsRef<Path>) -> Self {
        Self {
            sem_dir: sem_dir.as_ref().to_path_buf(),
            data_dir: data_dir.as_ref().to_path_buf(),
        }
    }

    /// Get the semaphore file path for a module
    fn sem_path(&self, module: &str, freq: Frequency) -> Option<PathBuf> {
        match freq {
            Frequency::PerBoot | Frequency::Always => None,
            Frequency::PerInstance => Some(self.sem_dir.join(format!("config_{module}"))),
            Frequency::PerOnce => Some(self.data_dir.join(format!("sem/config_{module}"))),
        }
    }

    /// Check if a module should run based on its semaphore
    pub async fn should_run(&self, module: &str, freq: Frequency) -> Result<bool, CloudInitError> {
        match freq {
            Frequency::PerBoot | Frequency::Always => Ok(true),
            Frequency::PerInstance | Frequency::PerOnce => {
                if let Some(path) = self.sem_path(module, freq) {
                    let exists = path.exists();
                    debug!(
                        "Semaphore check for {} ({}): {} -> {}",
                        module,
                        freq,
                        path.display(),
                        if exists { "skip" } else { "run" }
                    );
                    Ok(!exists)
                } else {
                    Ok(true)
                }
            }
        }
    }

    /// Mark a module as having run (create semaphore)
    pub async fn mark_done(&self, module: &str, freq: Frequency) -> Result<(), CloudInitError> {
        if let Some(path) = self.sem_path(module, freq) {
            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await?;
            }

            // Write timestamp to semaphore file
            let timestamp = chrono_lite_timestamp();
            fs::write(&path, timestamp.as_bytes()).await?;

            debug!("Created semaphore: {}", path.display());
        }
        Ok(())
    }

    /// Clear a module's semaphore (allow it to run again)
    #[allow(clippy::collapsible_if)]
    pub async fn clear(&self, module: &str, freq: Frequency) -> Result<(), CloudInitError> {
        if let Some(path) = self.sem_path(module, freq) {
            if path.exists() {
                fs::remove_file(&path).await?;
                debug!("Removed semaphore: {}", path.display());
            }
        }
        Ok(())
    }

    /// Clear all semaphores for this instance
    pub async fn clear_all(&self) -> Result<(), CloudInitError> {
        if self.sem_dir.exists() {
            fs::remove_dir_all(&self.sem_dir).await?;
            debug!("Cleared all semaphores in: {}", self.sem_dir.display());
        }
        Ok(())
    }

    /// List all existing semaphores
    #[allow(clippy::collapsible_if)]
    pub async fn list(&self) -> Result<Vec<String>, CloudInitError> {
        let mut semaphores = Vec::new();

        if self.sem_dir.exists() {
            let mut entries = fs::read_dir(&self.sem_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("config_") {
                        semaphores.push(name.strip_prefix("config_").unwrap_or(name).to_string());
                    }
                }
            }
        }

        Ok(semaphores)
    }
}

/// Get a simple timestamp string (lightweight, no chrono dependency)
fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    format!("{}", duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_semaphore_should_run() {
        let temp = TempDir::new().unwrap();
        let sem_dir = temp.path().join("sem");
        let data_dir = temp.path().join("data");

        let manager = SemaphoreManager::new(&sem_dir, &data_dir);

        // Should always run for per-boot and always
        assert!(
            manager
                .should_run("test", Frequency::PerBoot)
                .await
                .unwrap()
        );
        assert!(manager.should_run("test", Frequency::Always).await.unwrap());

        // Should run first time for per-instance
        assert!(
            manager
                .should_run("test", Frequency::PerInstance)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_semaphore_mark_done() {
        let temp = TempDir::new().unwrap();
        let sem_dir = temp.path().join("sem");
        let data_dir = temp.path().join("data");

        let manager = SemaphoreManager::new(&sem_dir, &data_dir);

        // Mark as done
        manager
            .mark_done("test_module", Frequency::PerInstance)
            .await
            .unwrap();

        // Should not run again
        assert!(
            !manager
                .should_run("test_module", Frequency::PerInstance)
                .await
                .unwrap()
        );

        // Semaphore file should exist
        assert!(sem_dir.join("config_test_module").exists());
    }

    #[tokio::test]
    async fn test_semaphore_clear() {
        let temp = TempDir::new().unwrap();
        let sem_dir = temp.path().join("sem");
        let data_dir = temp.path().join("data");

        let manager = SemaphoreManager::new(&sem_dir, &data_dir);

        // Mark and then clear
        manager
            .mark_done("test_module", Frequency::PerInstance)
            .await
            .unwrap();
        manager
            .clear("test_module", Frequency::PerInstance)
            .await
            .unwrap();

        // Should run again
        assert!(
            manager
                .should_run("test_module", Frequency::PerInstance)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_semaphore_per_once() {
        let temp = TempDir::new().unwrap();
        let sem_dir = temp.path().join("sem");
        let data_dir = temp.path().join("data");

        let manager = SemaphoreManager::new(&sem_dir, &data_dir);

        // Mark per-once
        manager
            .mark_done("once_module", Frequency::PerOnce)
            .await
            .unwrap();

        // Should not run
        assert!(
            !manager
                .should_run("once_module", Frequency::PerOnce)
                .await
                .unwrap()
        );

        // Semaphore in data dir
        assert!(data_dir.join("sem/config_once_module").exists());
    }

    #[tokio::test]
    async fn test_semaphore_list() {
        let temp = TempDir::new().unwrap();
        let sem_dir = temp.path().join("sem");
        let data_dir = temp.path().join("data");

        let manager = SemaphoreManager::new(&sem_dir, &data_dir);

        manager
            .mark_done("module_a", Frequency::PerInstance)
            .await
            .unwrap();
        manager
            .mark_done("module_b", Frequency::PerInstance)
            .await
            .unwrap();

        let list = manager.list().await.unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"module_a".to_string()));
        assert!(list.contains(&"module_b".to_string()));
    }

    #[test]
    fn test_frequency_display() {
        assert_eq!(Frequency::PerBoot.to_string(), "per-boot");
        assert_eq!(Frequency::PerInstance.to_string(), "per-instance");
        assert_eq!(Frequency::PerOnce.to_string(), "per-once");
        assert_eq!(Frequency::Always.to_string(), "always");
    }
}
