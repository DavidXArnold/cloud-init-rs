//! NoCloud datasource
//!
//! Reads metadata and user data from local files or mounted ISO.
//! Common locations:
//! - /var/lib/cloud/seed/nocloud/
//! - /var/lib/cloud/seed/nocloud-net/
//! - Mounted filesystem with label 'cidata' or 'CIDATA'

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::debug;

use super::Datasource;
use crate::{CloudInitError, InstanceMetadata, UserData, config::CloudConfig};

/// NoCloud datasource for local file-based configuration
pub struct NoCloud {
    seed_dirs: Vec<PathBuf>,
}

impl NoCloud {
    pub fn new() -> Self {
        Self {
            seed_dirs: vec![
                PathBuf::from("/var/lib/cloud/seed/nocloud"),
                PathBuf::from("/var/lib/cloud/seed/nocloud-net"),
            ],
        }
    }

    /// Find the seed directory containing meta-data
    async fn find_seed_dir(&self) -> Option<PathBuf> {
        for dir in &self.seed_dirs {
            let meta_data_path = dir.join("meta-data");
            if fs::metadata(&meta_data_path).await.is_ok() {
                return Some(dir.clone());
            }
        }

        // Check for mounted cidata filesystem
        if let Some(mount_point) = self.find_cidata_mount().await {
            return Some(mount_point);
        }

        None
    }

    /// Find mounted filesystem with cidata label
    async fn find_cidata_mount(&self) -> Option<PathBuf> {
        // Check common mount points for cidata
        let possible_mounts = ["/mnt/cidata", "/media/cidata", "/run/cloud-init/cidata"];

        for mount in possible_mounts {
            let path = Path::new(mount);
            if let Ok(metadata) = fs::metadata(path.join("meta-data")).await {
                if metadata.is_file() {
                    return Some(path.to_path_buf());
                }
            }
        }

        None
    }

    async fn read_file(&self, seed_dir: &Path, filename: &str) -> Option<String> {
        let path = seed_dir.join(filename);
        fs::read_to_string(&path).await.ok()
    }
}

impl Default for NoCloud {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Datasource for NoCloud {
    fn name(&self) -> &'static str {
        "NoCloud"
    }

    async fn is_available(&self) -> bool {
        self.find_seed_dir().await.is_some()
    }

    async fn get_metadata(&self) -> Result<InstanceMetadata, CloudInitError> {
        let seed_dir = self
            .find_seed_dir()
            .await
            .ok_or_else(|| CloudInitError::Datasource("NoCloud seed directory not found".into()))?;

        debug!("Reading NoCloud metadata from {:?}", seed_dir);

        let mut metadata = InstanceMetadata {
            cloud_name: Some("nocloud".to_string()),
            ..Default::default()
        };

        // Parse meta-data YAML
        if let Some(content) = self.read_file(&seed_dir, "meta-data").await {
            if let Ok(parsed) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                if let Some(id) = parsed.get("instance-id").and_then(|v| v.as_str()) {
                    metadata.instance_id = Some(id.to_string());
                }
                if let Some(hostname) = parsed.get("local-hostname").and_then(|v| v.as_str()) {
                    metadata.local_hostname = Some(hostname.to_string());
                }
            }
        }

        Ok(metadata)
    }

    async fn get_userdata(&self) -> Result<UserData, CloudInitError> {
        let seed_dir = self
            .find_seed_dir()
            .await
            .ok_or_else(|| CloudInitError::Datasource("NoCloud seed directory not found".into()))?;

        debug!("Reading NoCloud user-data from {:?}", seed_dir);

        let content = match self.read_file(&seed_dir, "user-data").await {
            Some(c) if !c.trim().is_empty() => c,
            _ => return Ok(UserData::None),
        };

        // Determine type of user data
        if CloudConfig::is_cloud_config(&content) {
            let config = CloudConfig::from_yaml(&content)?;
            Ok(UserData::CloudConfig(Box::new(config)))
        } else if content.starts_with("#!") {
            Ok(UserData::Script(content))
        } else {
            // Try to parse as cloud-config anyway
            match CloudConfig::from_yaml(&content) {
                Ok(config) => Ok(UserData::CloudConfig(Box::new(config))),
                Err(_) => Ok(UserData::Script(content)),
            }
        }
    }
}
