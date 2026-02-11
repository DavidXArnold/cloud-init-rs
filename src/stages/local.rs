//! Local stage - runs before network is available
//!
//! Responsibilities:
//! - Grow root partition (growpart)
//! - Resize filesystem
//! - Mount additional volumes
//! - Set up disk partitions
//! - Apply network configuration

use crate::CloudInitError;
use crate::network::render::apply_network_config;
use crate::network::v1::parse_network_config;
use crate::state::InstanceState;
use std::path::Path;
use tokio::fs;
use tracing::{debug, info, warn};

/// Run the local stage
pub async fn run() -> Result<(), CloudInitError> {
    info!("Local stage: starting pre-network initialization");

    // Check for NoCloud datasource (local files)
    check_nocloud_datasource().await?;

    // Apply network configuration (before network comes up)
    apply_network_configuration().await?;

    // Grow partition if needed
    grow_partition().await?;

    // Resize filesystem
    resize_filesystem().await?;

    info!("Local stage: completed");
    Ok(())
}

async fn check_nocloud_datasource() -> Result<(), CloudInitError> {
    debug!("Checking for NoCloud datasource");
    // Check standard locations for NoCloud data:
    // - /var/lib/cloud/seed/nocloud/
    // - /var/lib/cloud/seed/nocloud-net/
    // - Mounted filesystem with label 'cidata' or 'CIDATA'
    Ok(())
}

/// Apply network configuration from various sources
async fn apply_network_configuration() -> Result<(), CloudInitError> {
    debug!("Checking for network configuration");

    // Standard network config locations (in order of precedence)
    let config_paths = [
        "/etc/cloud/cloud.cfg.d/50-curtin-networking.cfg",
        "/etc/cloud/cloud.cfg.d/network-config",
        "/var/lib/cloud/seed/nocloud/network-config",
        "/var/lib/cloud/seed/nocloud-net/network-config",
    ];

    for path_str in &config_paths {
        let path = Path::new(path_str);
        if path.exists() {
            info!("Found network config at: {}", path_str);
            match fs::read_to_string(path).await {
                Ok(content) => {
                    return apply_network_from_content(&content).await;
                }
                Err(e) => {
                    warn!("Failed to read network config from {}: {}", path_str, e);
                }
            }
        }
    }

    // Check instance state for network config
    let mut state = InstanceState::new();
    if let Ok(Some(_instance_id)) = state.load_cached_instance_id().await {
        // Could load network config from instance-specific location
        debug!("No network configuration found in standard locations");
    }

    Ok(())
}

/// Apply network configuration from YAML content
async fn apply_network_from_content(content: &str) -> Result<(), CloudInitError> {
    // Parse network config (auto-detects v1 or v2)
    let config = parse_network_config(content).map_err(|e| {
        CloudInitError::InvalidData(format!("Failed to parse network config: {}", e))
    })?;

    if !config.has_interfaces() {
        debug!("Network config has no interfaces defined");
        return Ok(());
    }

    info!(
        "Applying network configuration for {} interfaces",
        config.interface_names().len()
    );

    // Apply the configuration using the appropriate renderer
    apply_network_config(&config, config.renderer.as_deref()).await?;

    Ok(())
}

async fn grow_partition() -> Result<(), CloudInitError> {
    debug!("Checking if partition needs to be grown");
    // TODO: Implement growpart functionality
    // This is typically done via growpart utility or direct partition manipulation
    Ok(())
}

async fn resize_filesystem() -> Result<(), CloudInitError> {
    debug!("Checking if filesystem needs to be resized");
    // TODO: Implement filesystem resize (resize2fs, xfs_growfs, etc.)
    Ok(())
}
