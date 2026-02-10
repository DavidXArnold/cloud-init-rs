//! OpenStack datasource
//!
//! Fetches metadata from OpenStack metadata service or config-drive.
//! https://docs.openstack.org/nova/latest/user/metadata.html

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tracing::debug;

use super::Datasource;
use crate::{CloudInitError, InstanceMetadata, UserData, config::CloudConfig};

/// OpenStack metadata service URL (link-local address)
const OPENSTACK_METADATA_URL: &str = "http://169.254.169.254/openstack";

/// Config-drive mount locations to check
const CONFIG_DRIVE_PATHS: &[&str] = &[
    "/mnt/config",
    "/config-2",
    "/media/configdrive",
    "/run/cloud-init/config-drive",
];

/// OpenStack metadata JSON structure
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenStackMetadata {
    #[serde(default)]
    uuid: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    hostname: String,
    #[serde(default)]
    availability_zone: String,
    #[serde(default)]
    project_id: String,
    #[serde(default)]
    meta: serde_json::Value,
}

/// OpenStack datasource
pub struct OpenStack {
    client: Client,
    metadata_url: String,
}

impl OpenStack {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(2))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            metadata_url: OPENSTACK_METADATA_URL.to_string(),
        }
    }

    /// Create with a custom base URL (for testing)
    pub fn with_base_url(base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(2))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            metadata_url: base_url.to_string(),
        }
    }

    /// Find config-drive mount point
    async fn find_config_drive() -> Option<PathBuf> {
        for path in CONFIG_DRIVE_PATHS {
            let meta_path = Path::new(path).join("openstack/latest/meta_data.json");
            if fs::metadata(&meta_path).await.is_ok() {
                return Some(PathBuf::from(path));
            }
        }
        None
    }

    /// Check if OpenStack metadata service is reachable
    async fn check_metadata_service(&self) -> bool {
        let url = format!("{}/latest/meta_data.json", self.metadata_url);
        self.client.get(&url).send().await.is_ok()
    }

    /// Fetch metadata from HTTP service
    async fn fetch_metadata_http(&self) -> Result<OpenStackMetadata, CloudInitError> {
        let url = format!("{}/latest/meta_data.json", self.metadata_url);
        debug!("Fetching OpenStack metadata from HTTP: {}", url);

        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            let metadata: OpenStackMetadata = response.json().await?;
            Ok(metadata)
        } else {
            Err(CloudInitError::Datasource(format!(
                "Failed to fetch OpenStack metadata: {}",
                response.status()
            )))
        }
    }

    /// Fetch metadata from config-drive
    async fn fetch_metadata_config_drive(
        config_drive: &Path,
    ) -> Result<OpenStackMetadata, CloudInitError> {
        let meta_path = config_drive.join("openstack/latest/meta_data.json");
        debug!(
            "Fetching OpenStack metadata from config-drive: {:?}",
            meta_path
        );

        let content = fs::read_to_string(&meta_path).await.map_err(|e| {
            CloudInitError::Datasource(format!("Failed to read config-drive metadata: {}", e))
        })?;

        let metadata: OpenStackMetadata = serde_json::from_str(&content).map_err(|e| {
            CloudInitError::Datasource(format!("Failed to parse config-drive metadata: {}", e))
        })?;

        Ok(metadata)
    }

    /// Fetch user-data from HTTP service
    async fn fetch_userdata_http(&self) -> Result<Option<String>, CloudInitError> {
        let url = format!("{}/latest/user_data", self.metadata_url);
        debug!("Fetching OpenStack user-data from HTTP: {}", url);

        let response = self.client.get(&url).send().await?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if response.status().is_success() {
            let content = response.text().await?;
            if content.is_empty() {
                Ok(None)
            } else {
                Ok(Some(content))
            }
        } else {
            Ok(None)
        }
    }

    /// Fetch user-data from config-drive
    async fn fetch_userdata_config_drive(
        config_drive: &Path,
    ) -> Result<Option<String>, CloudInitError> {
        let userdata_path = config_drive.join("openstack/latest/user_data");
        debug!(
            "Fetching OpenStack user-data from config-drive: {:?}",
            userdata_path
        );

        match fs::read_to_string(&userdata_path).await {
            Ok(content) if !content.is_empty() => Ok(Some(content)),
            _ => Ok(None),
        }
    }

    /// Check DMI data for OpenStack indicators
    async fn check_dmi_data() -> bool {
        let dmi_paths = [
            "/sys/class/dmi/id/product_name",
            "/sys/class/dmi/id/sys_vendor",
            "/sys/class/dmi/id/chassis_vendor",
        ];

        for path in &dmi_paths {
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                let content = content.to_lowercase();
                if content.contains("openstack")
                    || content.contains("bochs")
                    || content.contains("qemu")
                    || content.contains("kvm")
                    || content.contains("rhev")
                {
                    return true;
                }
            }
        }

        false
    }
}

impl Default for OpenStack {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Datasource for OpenStack {
    fn name(&self) -> &'static str {
        "OpenStack"
    }

    async fn is_available(&self) -> bool {
        // Check for config-drive first (no network needed)
        if Self::find_config_drive().await.is_some() {
            return true;
        }

        // Check DMI data
        if Self::check_dmi_data().await {
            return self.check_metadata_service().await;
        }

        // Try metadata service directly
        self.check_metadata_service().await
    }

    async fn get_metadata(&self) -> Result<InstanceMetadata, CloudInitError> {
        debug!("Fetching OpenStack instance metadata");

        // Try config-drive first, then HTTP
        let os_meta = if let Some(config_drive) = Self::find_config_drive().await {
            Self::fetch_metadata_config_drive(&config_drive).await?
        } else {
            self.fetch_metadata_http().await?
        };

        let mut metadata = InstanceMetadata {
            cloud_name: Some("openstack".to_string()),
            platform: Some("openstack".to_string()),
            ..Default::default()
        };

        if !os_meta.uuid.is_empty() {
            metadata.instance_id = Some(os_meta.uuid);
        }

        if !os_meta.hostname.is_empty() {
            metadata.local_hostname = Some(os_meta.hostname);
        } else if !os_meta.name.is_empty() {
            metadata.local_hostname = Some(os_meta.name);
        }

        if !os_meta.availability_zone.is_empty() {
            metadata.availability_zone = Some(os_meta.availability_zone.clone());
            // Try to extract region from AZ (commonly formatted as region-az)
            if let Some(idx) = os_meta.availability_zone.rfind('-') {
                metadata.region = Some(os_meta.availability_zone[..idx].to_string());
            }
        }

        Ok(metadata)
    }

    async fn get_userdata(&self) -> Result<UserData, CloudInitError> {
        debug!("Fetching OpenStack user-data");

        // Try config-drive first, then HTTP
        let content = if let Some(config_drive) = Self::find_config_drive().await {
            Self::fetch_userdata_config_drive(&config_drive).await?
        } else {
            self.fetch_userdata_http().await?
        };

        let content = match content {
            Some(c) if !c.is_empty() => c,
            _ => {
                debug!("No user-data available");
                return Ok(UserData::None);
            }
        };

        // Determine type of user data
        if CloudConfig::is_cloud_config(&content) {
            let config = CloudConfig::from_yaml(&content)?;
            Ok(UserData::CloudConfig(Box::new(config)))
        } else if content.starts_with("#!") {
            Ok(UserData::Script(content))
        } else {
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

    #[test]
    fn test_openstack_default() {
        let openstack = OpenStack::new();
        assert_eq!(openstack.name(), "OpenStack");
        assert_eq!(openstack.metadata_url, OPENSTACK_METADATA_URL);
    }
}
