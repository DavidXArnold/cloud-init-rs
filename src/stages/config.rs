//! Config stage - applies user configuration
//!
//! Responsibilities:
//! - Create users and groups
//! - Install packages
//! - Write files (write_files directive)
//! - Configure services

use crate::CloudInitError;
use tracing::{debug, info};

/// Run the config stage
pub async fn run() -> Result<(), CloudInitError> {
    info!("Config stage: applying user configuration");

    // Parse cloud-config
    let cloud_config = load_cloud_config().await?;

    // Apply configuration modules in order
    apply_users(&cloud_config).await?;
    apply_groups(&cloud_config).await?;
    apply_write_files(&cloud_config).await?;
    apply_packages(&cloud_config).await?;

    info!("Config stage: completed");
    Ok(())
}

#[allow(dead_code)]
#[derive(Debug, Default)]
struct CloudConfigData {
    users: Vec<UserConfig>,
    groups: Vec<String>,
    write_files: Vec<WriteFile>,
    packages: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct UserConfig {
    name: String,
    groups: Vec<String>,
    shell: Option<String>,
    ssh_authorized_keys: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct WriteFile {
    path: String,
    content: String,
    permissions: Option<String>,
    owner: Option<String>,
}

async fn load_cloud_config() -> Result<CloudConfigData, CloudInitError> {
    debug!("Loading cloud-config");
    // TODO: Load from /var/lib/cloud/instance/cloud-config.txt or userdata
    Ok(CloudConfigData::default())
}

async fn apply_users(_config: &CloudConfigData) -> Result<(), CloudInitError> {
    debug!("Applying user configuration");
    // TODO: Create users via useradd or direct /etc/passwd manipulation
    Ok(())
}

async fn apply_groups(_config: &CloudConfigData) -> Result<(), CloudInitError> {
    debug!("Applying group configuration");
    // TODO: Create groups via groupadd
    Ok(())
}

async fn apply_write_files(_config: &CloudConfigData) -> Result<(), CloudInitError> {
    debug!("Applying write_files configuration");
    // TODO: Write files with specified content, permissions, and ownership
    Ok(())
}

async fn apply_packages(_config: &CloudConfigData) -> Result<(), CloudInitError> {
    debug!("Applying package configuration");
    // TODO: Install packages via apt/yum/dnf/etc.
    Ok(())
}
