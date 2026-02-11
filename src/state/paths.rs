//! Standard cloud-init paths
//!
//! Defines the directory structure used by cloud-init for state management.

use std::path::{Path, PathBuf};

/// Base directory for cloud-init state
pub const CLOUD_DIR: &str = "/var/lib/cloud";

/// Cloud configuration directory
pub const CONFIG_DIR: &str = "/etc/cloud";

/// Standard cloud-init paths
#[derive(Debug, Clone)]
pub struct CloudPaths {
    /// Base cloud directory (default: /var/lib/cloud)
    pub base: PathBuf,
    /// Config directory (default: /etc/cloud)
    pub config: PathBuf,
}

impl Default for CloudPaths {
    fn default() -> Self {
        Self::new()
    }
}

impl CloudPaths {
    /// Create with default paths
    pub fn new() -> Self {
        Self {
            base: PathBuf::from(CLOUD_DIR),
            config: PathBuf::from(CONFIG_DIR),
        }
    }

    /// Create with custom base directory (useful for testing)
    pub fn with_base(base: impl AsRef<Path>) -> Self {
        Self {
            base: base.as_ref().to_path_buf(),
            config: PathBuf::from(CONFIG_DIR),
        }
    }

    /// Create with custom base and config directories
    pub fn with_dirs(base: impl AsRef<Path>, config: impl AsRef<Path>) -> Self {
        Self {
            base: base.as_ref().to_path_buf(),
            config: config.as_ref().to_path_buf(),
        }
    }

    // ==================== Base Directories ====================

    /// /var/lib/cloud/data - Cached data directory
    pub fn data_dir(&self) -> PathBuf {
        self.base.join("data")
    }

    /// /var/lib/cloud/instances - All instances directory
    pub fn instances_dir(&self) -> PathBuf {
        self.base.join("instances")
    }

    /// /var/lib/cloud/instance - Symlink to current instance
    pub fn instance_link(&self) -> PathBuf {
        self.base.join("instance")
    }

    /// /var/lib/cloud/scripts - Scripts directory
    pub fn scripts_dir(&self) -> PathBuf {
        self.base.join("scripts")
    }

    /// /var/lib/cloud/seed - NoCloud seed directory
    pub fn seed_dir(&self) -> PathBuf {
        self.base.join("seed")
    }

    // ==================== Instance-specific Paths ====================

    /// /var/lib/cloud/instances/<id> - Instance directory
    pub fn instance_dir(&self, instance_id: &str) -> PathBuf {
        self.instances_dir().join(instance_id)
    }

    /// /var/lib/cloud/instances/<id>/sem - Semaphore directory
    pub fn sem_dir(&self, instance_id: &str) -> PathBuf {
        self.instance_dir(instance_id).join("sem")
    }

    /// /var/lib/cloud/instances/<id>/boot-finished - Boot completion marker
    pub fn boot_finished(&self, instance_id: &str) -> PathBuf {
        self.instance_dir(instance_id).join("boot-finished")
    }

    /// /var/lib/cloud/instances/<id>/cloud-config.txt - Merged cloud-config
    pub fn cloud_config(&self, instance_id: &str) -> PathBuf {
        self.instance_dir(instance_id).join("cloud-config.txt")
    }

    /// /var/lib/cloud/instances/<id>/user-data.txt - Raw user-data
    pub fn user_data(&self, instance_id: &str) -> PathBuf {
        self.instance_dir(instance_id).join("user-data.txt")
    }

    /// /var/lib/cloud/instances/<id>/vendor-data.txt - Raw vendor-data
    pub fn vendor_data(&self, instance_id: &str) -> PathBuf {
        self.instance_dir(instance_id).join("vendor-data.txt")
    }

    /// /var/lib/cloud/instances/<id>/datasource - Datasource identifier
    pub fn datasource_file(&self, instance_id: &str) -> PathBuf {
        self.instance_dir(instance_id).join("datasource")
    }

    // ==================== Scripts Directories ====================

    /// /var/lib/cloud/scripts/per-boot - Scripts run every boot
    pub fn scripts_per_boot(&self) -> PathBuf {
        self.scripts_dir().join("per-boot")
    }

    /// /var/lib/cloud/scripts/per-instance - Scripts run once per instance
    pub fn scripts_per_instance(&self) -> PathBuf {
        self.scripts_dir().join("per-instance")
    }

    /// /var/lib/cloud/scripts/per-once - Scripts run once ever
    pub fn scripts_per_once(&self) -> PathBuf {
        self.scripts_dir().join("per-once")
    }

    // ==================== Config Paths ====================

    /// /etc/cloud/cloud.cfg - Main config file
    pub fn main_config(&self) -> PathBuf {
        self.config.join("cloud.cfg")
    }

    /// /etc/cloud/cloud.cfg.d - Config drop-in directory
    pub fn config_d(&self) -> PathBuf {
        self.config.join("cloud.cfg.d")
    }

    // ==================== Data Paths ====================

    /// /var/lib/cloud/data/instance-id - Cached instance ID
    pub fn cached_instance_id(&self) -> PathBuf {
        self.data_dir().join("instance-id")
    }

    /// /var/lib/cloud/data/previous-instance-id - Previous instance ID
    pub fn previous_instance_id(&self) -> PathBuf {
        self.data_dir().join("previous-instance-id")
    }

    /// /var/lib/cloud/data/result.json - Execution result
    pub fn result_file(&self) -> PathBuf {
        self.data_dir().join("result.json")
    }

    /// /var/lib/cloud/data/status.json - Current status
    pub fn status_file(&self) -> PathBuf {
        self.data_dir().join("status.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_paths() {
        let paths = CloudPaths::new();
        assert_eq!(paths.base, PathBuf::from("/var/lib/cloud"));
        assert_eq!(paths.config, PathBuf::from("/etc/cloud"));
    }

    #[test]
    fn test_custom_base() {
        let paths = CloudPaths::with_base("/tmp/cloud");
        assert_eq!(paths.data_dir(), PathBuf::from("/tmp/cloud/data"));
        assert_eq!(paths.instances_dir(), PathBuf::from("/tmp/cloud/instances"));
    }

    #[test]
    fn test_instance_paths() {
        let paths = CloudPaths::new();
        let id = "i-1234567890abcdef0";

        assert_eq!(
            paths.instance_dir(id),
            PathBuf::from("/var/lib/cloud/instances/i-1234567890abcdef0")
        );
        assert_eq!(
            paths.sem_dir(id),
            PathBuf::from("/var/lib/cloud/instances/i-1234567890abcdef0/sem")
        );
        assert_eq!(
            paths.boot_finished(id),
            PathBuf::from("/var/lib/cloud/instances/i-1234567890abcdef0/boot-finished")
        );
    }

    #[test]
    fn test_scripts_paths() {
        let paths = CloudPaths::new();
        assert_eq!(
            paths.scripts_per_boot(),
            PathBuf::from("/var/lib/cloud/scripts/per-boot")
        );
        assert_eq!(
            paths.scripts_per_instance(),
            PathBuf::from("/var/lib/cloud/scripts/per-instance")
        );
        assert_eq!(
            paths.scripts_per_once(),
            PathBuf::from("/var/lib/cloud/scripts/per-once")
        );
    }

    #[test]
    fn test_config_paths() {
        let paths = CloudPaths::new();
        assert_eq!(paths.main_config(), PathBuf::from("/etc/cloud/cloud.cfg"));
        assert_eq!(paths.config_d(), PathBuf::from("/etc/cloud/cloud.cfg.d"));
    }
}
