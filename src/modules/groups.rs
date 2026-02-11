//! Group creation and configuration module

use crate::CloudInitError;
use crate::config::GroupConfig;
use tracing::{debug, info};

/// Create groups from cloud-config
pub async fn create_groups(groups: &[GroupConfig]) -> Result<(), CloudInitError> {
    for group in groups {
        match group {
            GroupConfig::Name(name) => {
                create_group_simple(name).await?;
            }
            GroupConfig::WithMembers { name, members } => {
                create_group_with_members(name, members).await?;
            }
        }
    }
    Ok(())
}

/// Create a simple group
async fn create_group_simple(name: &str) -> Result<(), CloudInitError> {
    info!("Creating group: {}", name);

    let output = tokio::process::Command::new("groupadd")
        .arg(name)
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    // Exit code 9 means group already exists, which is fine
    if !output.status.success() && output.status.code() != Some(9) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::UserGroup(format!(
            "Failed to create group {}: {}",
            name, stderr
        )));
    }

    Ok(())
}

/// Create a group and add members to it
async fn create_group_with_members(name: &str, members: &[String]) -> Result<(), CloudInitError> {
    // First create the group
    create_group_simple(name).await?;

    // Then add each member
    for member in members {
        add_user_to_group(member, name).await?;
    }

    Ok(())
}

/// Add a user to a group
async fn add_user_to_group(username: &str, group: &str) -> Result<(), CloudInitError> {
    debug!("Adding user {} to group {}", username, group);

    let output = tokio::process::Command::new("usermod")
        .args(["--append", "--groups", group, username])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::UserGroup(format!(
            "Failed to add user {} to group {}: {}",
            username, group, stderr
        )));
    }

    Ok(())
}
