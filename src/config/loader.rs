//! Cloud-config loader
//!
//! Loads and merges cloud-configs from standard locations.

use super::{CloudConfig, merge};
use crate::{CloudInitError, state::CloudPaths};
use std::path::Path;
use tokio::fs;
use tracing::{debug, info, warn};

/// Load and merge all cloud-configs from standard locations
pub async fn load_merged_config(paths: &CloudPaths) -> Result<CloudConfig, CloudInitError> {
    let mut configs = Vec::new();

    // 1. Load base config (/etc/cloud/cloud.cfg)
    if let Some(config) = load_config_file(paths.main_config()).await? {
        debug!("Loaded base config from {}", paths.main_config().display());
        configs.push(config);
    }

    // 2. Load drop-in configs (/etc/cloud/cloud.cfg.d/*.cfg)
    let dropins = load_dropin_configs(paths.config_d()).await?;
    configs.extend(dropins);

    // Merge all configs
    Ok(merge::merge_all_configs(&configs))
}

/// Load cloud-config from a single file
async fn load_config_file(path: impl AsRef<Path>) -> Result<Option<CloudConfig>, CloudInitError> {
    let path = path.as_ref();

    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path).await?;

    match CloudConfig::from_yaml(&content) {
        Ok(config) => Ok(Some(config)),
        Err(e) => {
            warn!("Failed to parse {}: {}", path.display(), e);
            Ok(None)
        }
    }
}

/// Load all drop-in configs from a directory (sorted alphabetically)
async fn load_dropin_configs(dir: impl AsRef<Path>) -> Result<Vec<CloudConfig>, CloudInitError> {
    let dir = dir.as_ref();

    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut configs = Vec::new();
    let mut entries = Vec::new();

    // Read directory entries
    let mut read_dir = fs::read_dir(dir).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let path = entry.path();

        // Only process .cfg files
        if path.extension().is_some_and(|e| e == "cfg") {
            entries.push(path);
        }
    }

    // Sort alphabetically
    entries.sort();

    // Load each config file
    for path in entries {
        if let Some(config) = load_config_file(&path).await? {
            debug!("Loaded drop-in config from {}", path.display());
            configs.push(config);
        }
    }

    info!("Loaded {} drop-in configs", configs.len());
    Ok(configs)
}

/// Load and merge user-data, vendor-data, and system configs
#[allow(clippy::collapsible_if)]
pub async fn load_full_config(
    paths: &CloudPaths,
    userdata: Option<&str>,
    vendordata: Option<&str>,
) -> Result<CloudConfig, CloudInitError> {
    let mut configs = Vec::new();

    // 1. Load base config and drop-ins
    let system_config = load_merged_config(paths).await?;
    configs.push(system_config);

    // 2. Add vendor-data if present
    if let Some(vendor) = vendordata {
        if CloudConfig::is_cloud_config(vendor) {
            match CloudConfig::from_yaml(vendor) {
                Ok(config) => {
                    debug!("Loaded vendor-data config");
                    configs.push(config);
                }
                Err(e) => {
                    warn!("Failed to parse vendor-data: {}", e);
                }
            }
        }
    }

    // 3. Add user-data if present (highest priority)
    if let Some(user) = userdata {
        if CloudConfig::is_cloud_config(user) {
            match CloudConfig::from_yaml(user) {
                Ok(config) => {
                    debug!("Loaded user-data config");
                    configs.push(config);
                }
                Err(e) => {
                    warn!("Failed to parse user-data: {}", e);
                }
            }
        }
    }

    Ok(merge::merge_all_configs(&configs))
}

/// Configuration loader builder for more control
pub struct ConfigLoader {
    paths: CloudPaths,
    include_system: bool,
    include_dropins: bool,
    userdata: Option<String>,
    vendordata: Option<String>,
}

impl ConfigLoader {
    /// Create a new config loader with default paths
    pub fn new() -> Self {
        Self {
            paths: CloudPaths::new(),
            include_system: true,
            include_dropins: true,
            userdata: None,
            vendordata: None,
        }
    }

    /// Use custom paths
    pub fn with_paths(mut self, paths: CloudPaths) -> Self {
        self.paths = paths;
        self
    }

    /// Skip loading system config
    pub fn skip_system(mut self) -> Self {
        self.include_system = false;
        self
    }

    /// Skip loading drop-in configs
    pub fn skip_dropins(mut self) -> Self {
        self.include_dropins = false;
        self
    }

    /// Set user-data
    pub fn with_userdata(mut self, data: impl Into<String>) -> Self {
        self.userdata = Some(data.into());
        self
    }

    /// Set vendor-data
    pub fn with_vendordata(mut self, data: impl Into<String>) -> Self {
        self.vendordata = Some(data.into());
        self
    }

    /// Load and merge all configs
    #[allow(clippy::collapsible_if)]
    pub async fn load(self) -> Result<CloudConfig, CloudInitError> {
        let mut configs = Vec::new();

        // System config
        if self.include_system {
            if let Some(config) = load_config_file(self.paths.main_config()).await? {
                configs.push(config);
            }
        }

        // Drop-in configs
        if self.include_dropins {
            let dropins = load_dropin_configs(self.paths.config_d()).await?;
            configs.extend(dropins);
        }

        // Vendor-data
        if let Some(vendor) = &self.vendordata {
            if CloudConfig::is_cloud_config(vendor) {
                if let Ok(config) = CloudConfig::from_yaml(vendor) {
                    configs.push(config);
                }
            }
        }

        // User-data
        if let Some(user) = &self.userdata {
            if CloudConfig::is_cloud_config(user) {
                if let Ok(config) = CloudConfig::from_yaml(user) {
                    configs.push(config);
                }
            }
        }

        Ok(merge::merge_all_configs(&configs))
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_load_config_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("cloud.cfg");

        fs::write(&path, "#cloud-config\nhostname: test")
            .await
            .unwrap();

        let config = load_config_file(&path).await.unwrap().unwrap();
        assert_eq!(config.hostname, Some("test".to_string()));
    }

    #[tokio::test]
    async fn test_load_config_file_not_exists() {
        let result = load_config_file("/nonexistent/path").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_load_dropin_configs() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("cloud.cfg.d");
        fs::create_dir_all(&dir).await.unwrap();

        // Create drop-in files
        fs::write(dir.join("00-base.cfg"), "#cloud-config\nhostname: base")
            .await
            .unwrap();
        fs::write(
            dir.join("10-override.cfg"),
            "#cloud-config\nhostname: override",
        )
        .await
        .unwrap();
        // Non-cfg file should be ignored
        fs::write(dir.join("ignored.txt"), "not a config")
            .await
            .unwrap();

        let configs = load_dropin_configs(&dir).await.unwrap();
        assert_eq!(configs.len(), 2);

        // Should be sorted, so 00-base first, then 10-override
        assert_eq!(configs[0].hostname, Some("base".to_string()));
        assert_eq!(configs[1].hostname, Some("override".to_string()));
    }

    #[tokio::test]
    async fn test_load_merged_config() {
        let temp = TempDir::new().unwrap();

        // Create config structure
        let config_dir = temp.path().join("etc/cloud");
        let config_d = config_dir.join("cloud.cfg.d");
        fs::create_dir_all(&config_d).await.unwrap();

        // Base config
        fs::write(
            config_dir.join("cloud.cfg"),
            "#cloud-config\nhostname: base\ntimezone: UTC",
        )
        .await
        .unwrap();

        // Drop-in
        fs::write(
            config_d.join("override.cfg"),
            "#cloud-config\nhostname: override",
        )
        .await
        .unwrap();

        let paths = CloudPaths::with_dirs(temp.path(), &config_dir);
        let config = load_merged_config(&paths).await.unwrap();

        // hostname should be from drop-in (override)
        assert_eq!(config.hostname, Some("override".to_string()));
        // timezone should be from base
        assert_eq!(config.timezone, Some("UTC".to_string()));
    }

    #[tokio::test]
    async fn test_config_loader_builder() {
        let userdata = "#cloud-config\nhostname: userdata\npackages:\n  - nginx";

        let config = ConfigLoader::new()
            .skip_system()
            .skip_dropins()
            .with_userdata(userdata)
            .load()
            .await
            .unwrap();

        assert_eq!(config.hostname, Some("userdata".to_string()));
        assert_eq!(config.packages, vec!["nginx"]);
    }

    #[tokio::test]
    async fn test_load_full_config() {
        let temp = TempDir::new().unwrap();
        let config_dir = temp.path().join("etc/cloud");
        fs::create_dir_all(&config_dir).await.unwrap();

        fs::write(
            config_dir.join("cloud.cfg"),
            "#cloud-config\nhostname: system",
        )
        .await
        .unwrap();

        let paths = CloudPaths::with_dirs(temp.path(), &config_dir);

        let config = load_full_config(
            &paths,
            Some("#cloud-config\nhostname: user"),
            Some("#cloud-config\ntimezone: UTC"),
        )
        .await
        .unwrap();

        // User-data wins for hostname
        assert_eq!(config.hostname, Some("user".to_string()));
        // Vendor-data provides timezone
        assert_eq!(config.timezone, Some("UTC".to_string()));
    }
}
