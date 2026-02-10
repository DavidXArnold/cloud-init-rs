//! GCE (Google Compute Engine) datasource
//!
//! Fetches metadata from the GCE metadata server.
//! <https://cloud.google.com/compute/docs/metadata/overview>

use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;
use tracing::debug;

use super::Datasource;
use crate::{CloudInitError, InstanceMetadata, UserData, config::CloudConfig};

/// GCE metadata service base URL
const GCE_METADATA_URL: &str = "http://metadata.google.internal/computeMetadata/v1";

/// Required header for GCE metadata requests
const METADATA_FLAVOR_HEADER: &str = "Metadata-Flavor";
const METADATA_FLAVOR_VALUE: &str = "Google";

/// GCE datasource for Google Cloud Platform
pub struct Gce {
    client: Client,
    base_url: String,
}

impl Gce {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(2))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: GCE_METADATA_URL.to_string(),
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
            base_url: base_url.to_string(),
        }
    }

    /// Fetch a metadata path with the required Metadata-Flavor header
    async fn fetch_metadata(&self, path: &str) -> Result<String, CloudInitError> {
        let url = format!("{}/{}", self.base_url, path);
        debug!("Fetching GCE metadata: {}", url);

        let response = self
            .client
            .get(&url)
            .header(METADATA_FLAVOR_HEADER, METADATA_FLAVOR_VALUE)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.text().await?)
        } else {
            Err(CloudInitError::Datasource(format!(
                "Failed to fetch {}: {}",
                path,
                response.status()
            )))
        }
    }

    /// Check if GCE metadata server is reachable
    async fn check_metadata_server(&self) -> bool {
        let url = format!("{}/", self.base_url);
        self.client
            .get(&url)
            .header(METADATA_FLAVOR_HEADER, METADATA_FLAVOR_VALUE)
            .send()
            .await
            .is_ok()
    }

    /// Check DMI data for GCE indicators
    async fn check_dmi_data() -> bool {
        let dmi_paths = [
            "/sys/class/dmi/id/product_name",
            "/sys/class/dmi/id/bios_vendor",
            "/sys/class/dmi/id/sys_vendor",
        ];

        for path in &dmi_paths {
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                let content = content.to_lowercase();
                if content.contains("google") {
                    return true;
                }
            }
        }

        false
    }
}

impl Default for Gce {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Datasource for Gce {
    fn name(&self) -> &'static str {
        "GCE"
    }

    async fn is_available(&self) -> bool {
        // First check DMI data (fast, local check)
        if Self::check_dmi_data().await {
            return self.check_metadata_server().await;
        }

        // Also try metadata server directly (for nested virt or containers)
        self.check_metadata_server().await
    }

    async fn get_metadata(&self) -> Result<InstanceMetadata, CloudInitError> {
        debug!("Fetching GCE instance metadata");

        let mut metadata = InstanceMetadata {
            cloud_name: Some("gce".to_string()),
            platform: Some("gce".to_string()),
            ..Default::default()
        };

        // Instance ID
        if let Ok(instance_id) = self.fetch_metadata("instance/id").await {
            metadata.instance_id = Some(instance_id);
        }

        // Hostname
        if let Ok(hostname) = self.fetch_metadata("instance/hostname").await {
            metadata.local_hostname = Some(hostname);
        }

        // Zone (format: projects/PROJECT_NUM/zones/ZONE)
        if let Ok(zone_path) = self.fetch_metadata("instance/zone").await {
            // Extract zone name from path
            if let Some(zone) = zone_path.rsplit('/').next() {
                metadata.availability_zone = Some(zone.to_string());
                // Region is zone minus the last part (e.g., us-central1-a -> us-central1)
                if let Some(idx) = zone.rfind('-') {
                    metadata.region = Some(zone[..idx].to_string());
                }
            }
        }

        // Machine type
        if let Ok(machine_type_path) = self.fetch_metadata("instance/machine-type").await {
            // Extract machine type from path
            if let Some(machine_type) = machine_type_path.rsplit('/').next() {
                metadata.instance_type = Some(machine_type.to_string());
            }
        }

        Ok(metadata)
    }

    async fn get_userdata(&self) -> Result<UserData, CloudInitError> {
        debug!("Fetching GCE user-data");

        // GCE stores user-data in instance attributes
        // Try both "user-data" and "startup-script" attributes
        let userdata_result = self.fetch_metadata("instance/attributes/user-data").await;

        let content = match userdata_result {
            Ok(content) if !content.is_empty() => content,
            _ => {
                // Try startup-script as fallback
                match self
                    .fetch_metadata("instance/attributes/startup-script")
                    .await
                {
                    Ok(content) if !content.is_empty() => content,
                    _ => {
                        debug!("No user-data or startup-script available");
                        return Ok(UserData::None);
                    }
                }
            }
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

    #[test]
    fn test_gce_default() {
        let gce = Gce::new();
        assert_eq!(gce.name(), "GCE");
        assert_eq!(gce.base_url, GCE_METADATA_URL);
    }
}
