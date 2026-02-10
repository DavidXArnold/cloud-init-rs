//! Azure IMDS datasource
//!
//! Fetches metadata from Azure Instance Metadata Service (IMDS).
//! <https://docs.microsoft.com/en-us/azure/virtual-machines/linux/instance-metadata-service>

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::debug;

use super::Datasource;
use crate::{CloudInitError, InstanceMetadata, UserData, config::CloudConfig};

/// Azure IMDS base URL (link-local address)
const AZURE_IMDS_URL: &str = "http://169.254.169.254/metadata";

/// API version for Azure IMDS
const AZURE_API_VERSION: &str = "2021-02-01";

/// Azure IMDS response structures
#[derive(Debug, Deserialize)]
struct AzureInstanceMetadata {
    compute: AzureCompute,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzureCompute {
    #[serde(default)]
    vm_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    vm_size: String,
    #[serde(default)]
    zone: String,
    #[serde(default)]
    computer_name: String,
}

/// Azure IMDS datasource
pub struct Azure {
    client: Client,
    base_url: String,
}

impl Azure {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(2))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: AZURE_IMDS_URL.to_string(),
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

    /// Fetch Azure IMDS instance metadata
    async fn fetch_instance_metadata(&self) -> Result<AzureInstanceMetadata, CloudInitError> {
        let url = format!(
            "{}/instance?api-version={}",
            self.base_url, AZURE_API_VERSION
        );
        debug!("Fetching Azure IMDS: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Metadata", "true")
            .send()
            .await?;

        if response.status().is_success() {
            let metadata: AzureInstanceMetadata = response.json().await?;
            Ok(metadata)
        } else {
            Err(CloudInitError::Datasource(format!(
                "Failed to fetch Azure metadata: {}",
                response.status()
            )))
        }
    }

    /// Check if Azure IMDS is reachable
    async fn check_imds(&self) -> bool {
        let url = format!(
            "{}/instance?api-version={}",
            self.base_url, AZURE_API_VERSION
        );
        self.client
            .get(&url)
            .header("Metadata", "true")
            .send()
            .await
            .is_ok()
    }

    /// Check DMI data for Azure indicators
    async fn check_dmi_data() -> bool {
        let dmi_paths = [
            "/sys/class/dmi/id/product_name",
            "/sys/class/dmi/id/bios_vendor",
            "/sys/class/dmi/id/sys_vendor",
            "/sys/class/dmi/id/chassis_asset_tag",
        ];

        for path in &dmi_paths {
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                let content = content.to_lowercase();
                if content.contains("microsoft")
                    || content.contains("azure")
                    || content.contains("virtual machine")
                {
                    return true;
                }
            }
        }

        // Check for Azure-specific chassis asset tag
        if let Ok(asset_tag) =
            tokio::fs::read_to_string("/sys/class/dmi/id/chassis_asset_tag").await
            && asset_tag
                .trim()
                .eq_ignore_ascii_case("7783-7084-3265-9085-8269-3286-77")
        {
            return true;
        }

        false
    }
}

impl Default for Azure {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Datasource for Azure {
    fn name(&self) -> &'static str {
        "Azure"
    }

    async fn is_available(&self) -> bool {
        // First check DMI data (fast, local check)
        if Self::check_dmi_data().await {
            return self.check_imds().await;
        }

        // Also try IMDS directly
        self.check_imds().await
    }

    async fn get_metadata(&self) -> Result<InstanceMetadata, CloudInitError> {
        debug!("Fetching Azure instance metadata");

        let azure_meta = self.fetch_instance_metadata().await?;

        let mut metadata = InstanceMetadata {
            cloud_name: Some("azure".to_string()),
            platform: Some("azure".to_string()),
            ..Default::default()
        };

        if !azure_meta.compute.vm_id.is_empty() {
            metadata.instance_id = Some(azure_meta.compute.vm_id);
        }

        if !azure_meta.compute.computer_name.is_empty() {
            metadata.local_hostname = Some(azure_meta.compute.computer_name);
        } else if !azure_meta.compute.name.is_empty() {
            metadata.local_hostname = Some(azure_meta.compute.name);
        }

        if !azure_meta.compute.location.is_empty() {
            metadata.region = Some(azure_meta.compute.location.clone());
            // Azure uses zone within location
            if !azure_meta.compute.zone.is_empty() {
                metadata.availability_zone = Some(format!(
                    "{}-{}",
                    azure_meta.compute.location, azure_meta.compute.zone
                ));
            }
        }

        if !azure_meta.compute.vm_size.is_empty() {
            metadata.instance_type = Some(azure_meta.compute.vm_size);
        }

        Ok(metadata)
    }

    async fn get_userdata(&self) -> Result<UserData, CloudInitError> {
        debug!("Fetching Azure user-data");

        // Azure provides custom data via IMDS
        let url = format!(
            "{}/instance/compute/customData?api-version={}&format=text",
            self.base_url, AZURE_API_VERSION
        );

        let response = self
            .client
            .get(&url)
            .header("Metadata", "true")
            .send()
            .await?;

        if !response.status().is_success() {
            debug!("No custom data available: {}", response.status());
            return Ok(UserData::None);
        }

        let content = response.text().await?;

        if content.is_empty() {
            return Ok(UserData::None);
        }

        // Azure custom data is base64 encoded
        let decoded =
            match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &content) {
                Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                Err(_) => {
                    // Not base64, use as-is
                    content
                }
            };

        // Determine type of user data
        if CloudConfig::is_cloud_config(&decoded) {
            let config = CloudConfig::from_yaml(&decoded)?;
            Ok(UserData::CloudConfig(Box::new(config)))
        } else if decoded.starts_with("#!") {
            Ok(UserData::Script(decoded))
        } else {
            match CloudConfig::from_yaml(&decoded) {
                Ok(config) => Ok(UserData::CloudConfig(Box::new(config))),
                Err(_) => Ok(UserData::Script(decoded)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_default() {
        let azure = Azure::new();
        assert_eq!(azure.name(), "Azure");
        assert_eq!(azure.base_url, AZURE_IMDS_URL);
    }
}
