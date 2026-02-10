//! EC2 (AWS) datasource
//!
//! Fetches metadata from the EC2 Instance Metadata Service (IMDS).
//! Supports both IMDSv1 and IMDSv2 (preferred for security).

use async_trait::async_trait;
use reqwest::Client;
use std::path::Path;
use std::time::Duration;
use tracing::{debug, warn};

use super::Datasource;
use crate::{config::CloudConfig, CloudInitError, InstanceMetadata, UserData};

/// EC2 metadata service base URL (link-local address)
const IMDS_BASE_URL: &str = "http://169.254.169.254";

/// IMDSv2 token TTL in seconds
const TOKEN_TTL_SECONDS: u32 = 300;

/// EC2 datasource for AWS and compatible clouds (OpenStack, etc.)
pub struct Ec2 {
    client: Client,
    base_url: String,
}

impl Ec2 {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(2))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: IMDS_BASE_URL.to_string(),
        }
    }

    /// Get IMDSv2 token for authenticated requests
    async fn get_imdsv2_token(&self) -> Option<String> {
        let url = format!("{}/latest/api/token", self.base_url);
        let response = self
            .client
            .put(&url)
            .header("X-aws-ec2-metadata-token-ttl-seconds", TOKEN_TTL_SECONDS.to_string())
            .send()
            .await
            .ok()?;

        if response.status().is_success() {
            response.text().await.ok()
        } else {
            None
        }
    }

    /// Fetch a metadata path, trying IMDSv2 first then falling back to IMDSv1
    async fn fetch_metadata_path(&self, path: &str) -> Result<String, CloudInitError> {
        let url = format!("{}/latest/meta-data/{}", self.base_url, path);

        // Try IMDSv2 first (more secure)
        if let Some(token) = self.get_imdsv2_token().await {
            debug!("Using IMDSv2 for {}", path);
            let response = self
                .client
                .get(&url)
                .header("X-aws-ec2-metadata-token", &token)
                .send()
                .await?;

            if response.status().is_success() {
                return Ok(response.text().await?);
            }
        }

        // Fall back to IMDSv1
        debug!("Falling back to IMDSv1 for {}", path);
        let response = self.client.get(&url).send().await?;

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

    /// Check if IMDS is reachable
    async fn check_imds(&self) -> bool {
        let url = format!("{}/latest/meta-data/", self.base_url);

        // Try IMDSv2 first
        if let Some(token) = self.get_imdsv2_token().await {
            let result = self
                .client
                .get(&url)
                .header("X-aws-ec2-metadata-token", &token)
                .send()
                .await;
            if result.is_ok() {
                return true;
            }
        }

        // Fall back to IMDSv1
        self.client.get(&url).send().await.is_ok()
    }

    /// Check if we're running on EC2 by looking for DMI data
    async fn check_dmi_data() -> bool {
        let dmi_paths = [
            "/sys/class/dmi/id/product_name",
            "/sys/class/dmi/id/bios_vendor",
            "/sys/class/dmi/id/sys_vendor",
        ];

        for path in &dmi_paths {
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                let content = content.to_lowercase();
                if content.contains("amazon") || content.contains("ec2") {
                    return true;
                }
            }
        }

        Path::new("/sys/hypervisor/uuid").exists()
    }
}

impl Default for Ec2 {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Datasource for Ec2 {
    fn name(&self) -> &'static str {
        "EC2"
    }

    async fn is_available(&self) -> bool {
        // First check DMI data (fast, local check)
        if Self::check_dmi_data().await {
            // Then verify IMDS is reachable
            return self.check_imds().await;
        }
        false
    }

    async fn get_metadata(&self) -> Result<InstanceMetadata, CloudInitError> {
        debug!("Fetching EC2 instance metadata");

        let mut metadata = InstanceMetadata::default();
        metadata.cloud_name = Some("aws".to_string());
        metadata.platform = Some("ec2".to_string());

        // Fetch individual metadata items (continue on individual failures)
        if let Ok(instance_id) = self.fetch_metadata_path("instance-id").await {
            metadata.instance_id = Some(instance_id);
        }

        if let Ok(hostname) = self.fetch_metadata_path("local-hostname").await {
            metadata.local_hostname = Some(hostname);
        }

        if let Ok(az) = self.fetch_metadata_path("placement/availability-zone").await {
            metadata.availability_zone = Some(az.clone());
            // Region is AZ minus the last character (e.g., us-east-1a -> us-east-1)
            if az.len() > 1 {
                metadata.region = Some(az[..az.len() - 1].to_string());
            }
        }

        Ok(metadata)
    }

    async fn get_userdata(&self) -> Result<UserData, CloudInitError> {
        debug!("Fetching EC2 user-data");

        let url = format!("{}/latest/user-data", self.base_url);

        // Try IMDSv2 first
        let response = if let Some(token) = self.get_imdsv2_token().await {
            self.client
                .get(&url)
                .header("X-aws-ec2-metadata-token", &token)
                .send()
                .await?
        } else {
            self.client.get(&url).send().await?
        };

        // 404 means no user-data configured
        if response.status().as_u16() == 404 {
            debug!("No user-data available");
            return Ok(UserData::None);
        }

        if !response.status().is_success() {
            warn!("Failed to fetch user-data: {}", response.status());
            return Ok(UserData::None);
        }

        let content = response.text().await?;

        if content.is_empty() {
            return Ok(UserData::None);
        }

        // Determine type of user data
        if CloudConfig::is_cloud_config(&content) {
            let config = CloudConfig::from_yaml(&content)?;
            Ok(UserData::CloudConfig(config))
        } else if content.starts_with("#!") {
            Ok(UserData::Script(content))
        } else {
            // Try to parse as cloud-config anyway
            match CloudConfig::from_yaml(&content) {
                Ok(config) => Ok(UserData::CloudConfig(config)),
                Err(_) => Ok(UserData::Script(content)),
            }
        }
    }
}
