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

    /// Create with custom seed directories (for testing)
    pub fn with_seed_dirs(dirs: Vec<PathBuf>) -> Self {
        Self { seed_dirs: dirs }
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
            if let Ok(metadata) = fs::metadata(path.join("meta-data")).await
                && metadata.is_file()
            {
                return Some(path.to_path_buf());
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
#[allow(clippy::manual_try_fold)]
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
        if let Some(content) = self.read_file(&seed_dir, "meta-data").await
            && let Ok(parsed) = serde_yaml::from_str::<serde_yaml::Value>(&content)
        {
            if let Some(id) = parsed.get("instance-id").and_then(|v| v.as_str()) {
                metadata.instance_id = Some(id.to_string());
            }
            if let Some(hostname) = parsed.get("local-hostname").and_then(|v| v.as_str()) {
                metadata.local_hostname = Some(hostname.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_seed_dir(temp: &TempDir) -> PathBuf {
        let seed = temp.path().join("nocloud");
        std::fs::create_dir_all(&seed).unwrap();
        seed
    }

    #[tokio::test]
    async fn test_nocloud_is_available() {
        let temp = TempDir::new().unwrap();
        let seed = create_seed_dir(&temp);
        tokio::fs::write(seed.join("meta-data"), "instance-id: test\n")
            .await
            .unwrap();

        let nc = NoCloud::with_seed_dirs(vec![seed]);
        assert!(nc.is_available().await);
    }

    #[tokio::test]
    async fn test_nocloud_not_available() {
        let nc = NoCloud::with_seed_dirs(vec![PathBuf::from("/nonexistent/seed")]);
        assert!(!nc.is_available().await);
    }

    #[tokio::test]
    async fn test_nocloud_get_metadata() {
        let temp = TempDir::new().unwrap();
        let seed = create_seed_dir(&temp);
        tokio::fs::write(
            seed.join("meta-data"),
            "instance-id: i-nc123\nlocal-hostname: nc-host\n",
        )
        .await
        .unwrap();

        let nc = NoCloud::with_seed_dirs(vec![seed]);
        let metadata = nc.get_metadata().await.unwrap();

        assert_eq!(metadata.cloud_name, Some("nocloud".to_string()));
        assert_eq!(metadata.instance_id, Some("i-nc123".to_string()));
        assert_eq!(metadata.local_hostname, Some("nc-host".to_string()));
    }

    #[tokio::test]
    async fn test_nocloud_get_metadata_no_seed() {
        let nc = NoCloud::with_seed_dirs(vec![PathBuf::from("/nonexistent")]);
        let result = nc.get_metadata().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_nocloud_get_userdata_cloud_config() {
        let temp = TempDir::new().unwrap();
        let seed = create_seed_dir(&temp);
        tokio::fs::write(seed.join("meta-data"), "instance-id: test\n")
            .await
            .unwrap();
        tokio::fs::write(seed.join("user-data"), "#cloud-config\nhostname: nc-host\n")
            .await
            .unwrap();

        let nc = NoCloud::with_seed_dirs(vec![seed]);
        let userdata = nc.get_userdata().await.unwrap();

        match userdata {
            UserData::CloudConfig(config) => {
                assert_eq!(config.hostname, Some("nc-host".to_string()));
            }
            _ => panic!("Expected CloudConfig"),
        }
    }

    #[tokio::test]
    async fn test_nocloud_get_userdata_script() {
        let temp = TempDir::new().unwrap();
        let seed = create_seed_dir(&temp);
        tokio::fs::write(seed.join("meta-data"), "instance-id: test\n")
            .await
            .unwrap();
        tokio::fs::write(seed.join("user-data"), "#!/bin/bash\necho hello")
            .await
            .unwrap();

        let nc = NoCloud::with_seed_dirs(vec![seed]);
        let userdata = nc.get_userdata().await.unwrap();

        assert!(matches!(userdata, UserData::Script(_)));
    }

    #[tokio::test]
    async fn test_nocloud_get_userdata_empty() {
        let temp = TempDir::new().unwrap();
        let seed = create_seed_dir(&temp);
        tokio::fs::write(seed.join("meta-data"), "instance-id: test\n")
            .await
            .unwrap();
        tokio::fs::write(seed.join("user-data"), "  \n")
            .await
            .unwrap();

        let nc = NoCloud::with_seed_dirs(vec![seed]);
        let userdata = nc.get_userdata().await.unwrap();
        assert!(matches!(userdata, UserData::None));
    }

    #[tokio::test]
    async fn test_nocloud_get_userdata_missing() {
        let temp = TempDir::new().unwrap();
        let seed = create_seed_dir(&temp);
        tokio::fs::write(seed.join("meta-data"), "instance-id: test\n")
            .await
            .unwrap();
        // No user-data file

        let nc = NoCloud::with_seed_dirs(vec![seed]);
        let userdata = nc.get_userdata().await.unwrap();
        assert!(matches!(userdata, UserData::None));
    }

    #[tokio::test]
    async fn test_nocloud_get_userdata_ambiguous() {
        let temp = TempDir::new().unwrap();
        let seed = create_seed_dir(&temp);
        tokio::fs::write(seed.join("meta-data"), "instance-id: test\n")
            .await
            .unwrap();
        tokio::fs::write(seed.join("user-data"), "hostname: fallback-test")
            .await
            .unwrap();

        let nc = NoCloud::with_seed_dirs(vec![seed]);
        let userdata = nc.get_userdata().await.unwrap();

        match userdata {
            UserData::CloudConfig(config) => {
                assert_eq!(config.hostname, Some("fallback-test".to_string()));
            }
            _ => panic!("Expected CloudConfig from fallback"),
        }
    }

    #[tokio::test]
    async fn test_nocloud_get_userdata_unparseable() {
        let temp = TempDir::new().unwrap();
        let seed = create_seed_dir(&temp);
        tokio::fs::write(seed.join("meta-data"), "instance-id: test\n")
            .await
            .unwrap();
        tokio::fs::write(seed.join("user-data"), "random text content")
            .await
            .unwrap();

        let nc = NoCloud::with_seed_dirs(vec![seed]);
        let userdata = nc.get_userdata().await.unwrap();
        assert!(matches!(userdata, UserData::Script(_)));
    }

    #[tokio::test]
    async fn test_nocloud_get_userdata_no_seed() {
        let nc = NoCloud::with_seed_dirs(vec![PathBuf::from("/nonexistent")]);
        let result = nc.get_userdata().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_nocloud_name() {
        let nc = NoCloud::new();
        assert_eq!(nc.name(), "NoCloud");
    }

    #[test]
    fn test_nocloud_default() {
        let nc = NoCloud::default();
        assert_eq!(nc.seed_dirs.len(), 2);
    }
}
