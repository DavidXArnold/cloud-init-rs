//! EC2 (AWS) datasource
//!
//! Fetches metadata from the EC2 Instance Metadata Service (IMDS).
//! Supports both IMDSv1 and IMDSv2.
//!
//! Note: HTTP client implementation is stubbed out. Add ureq or reqwest
//! as a dependency to enable actual IMDS queries.

use async_trait::async_trait;
use std::path::Path;
use tracing::debug;

use super::Datasource;
use crate::{CloudInitError, InstanceMetadata, UserData};

/// EC2 datasource for AWS and compatible clouds
pub struct Ec2;

impl Ec2 {
    pub fn new() -> Self {
        Self
    }

    /// Check if we're running on EC2 by looking for DMI data
    async fn check_dmi_data() -> bool {
        // Check for EC2 hypervisor in DMI data
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

        // Check for EC2 specific files
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
        // Check DMI data for EC2 indicators
        Self::check_dmi_data().await
    }

    async fn get_metadata(&self) -> Result<InstanceMetadata, CloudInitError> {
        debug!("EC2 datasource: metadata fetch not implemented");
        // TODO: Implement IMDS query when HTTP client is available
        // For now, return empty metadata
        let mut metadata = InstanceMetadata::default();
        metadata.cloud_name = Some("aws".to_string());
        metadata.platform = Some("ec2".to_string());
        Ok(metadata)
    }

    async fn get_userdata(&self) -> Result<UserData, CloudInitError> {
        debug!("EC2 datasource: userdata fetch not implemented");
        // TODO: Implement IMDS query when HTTP client is available
        Ok(UserData::None)
    }
}

// Note: Full EC2 IMDS implementation requires an HTTP client.
// Add the following to Cargo.toml to enable:
//   ureq = { version = "2.9", features = ["json"] }
// Then implement the IMDS v2 token-based authentication and queries.
