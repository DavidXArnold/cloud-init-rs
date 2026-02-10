//! Network stage - runs after network is configured
//!
//! Responsibilities:
//! - Fetch metadata from cloud provider
//! - Configure SSH authorized keys
//! - Set hostname
//! - Configure network (if cloud-config specifies)

use crate::CloudInitError;
use tracing::{debug, info};

/// Run the network stage
pub async fn run() -> Result<(), CloudInitError> {
    info!("Network stage: fetching metadata and configuring instance");

    // Detect and query datasource
    let metadata = fetch_metadata().await?;
    debug!("Retrieved metadata: {:?}", metadata);

    // Set hostname from metadata
    configure_hostname(&metadata).await?;

    // Configure SSH keys
    configure_ssh_keys(&metadata).await?;

    info!("Network stage: completed");
    Ok(())
}

#[allow(dead_code)]
#[derive(Debug, Default)]
struct Metadata {
    instance_id: Option<String>,
    hostname: Option<String>,
    ssh_public_keys: Vec<String>,
}

async fn fetch_metadata() -> Result<Metadata, CloudInitError> {
    debug!("Attempting to fetch instance metadata");

    // Try datasources in order of priority:
    // 1. NoCloud (already checked in local stage)
    // 2. EC2/AWS
    // 3. GCE
    // 4. Azure
    // 5. OpenStack

    // For now, return empty metadata
    Ok(Metadata::default())
}

async fn configure_hostname(metadata: &Metadata) -> Result<(), CloudInitError> {
    if let Some(hostname) = &metadata.hostname {
        debug!("Setting hostname to: {}", hostname);
        // TODO: Set hostname via hostname crate or direct file manipulation
    }
    Ok(())
}

async fn configure_ssh_keys(metadata: &Metadata) -> Result<(), CloudInitError> {
    if !metadata.ssh_public_keys.is_empty() {
        debug!(
            "Configuring {} SSH public keys",
            metadata.ssh_public_keys.len()
        );
        // TODO: Write keys to /root/.ssh/authorized_keys and user accounts
    }
    Ok(())
}
