//! User creation and configuration module

use crate::config::{UserConfig, UserFullConfig};
use crate::CloudInitError;
use tracing::{debug, info};

/// Create users from cloud-config
pub async fn create_users(users: &[UserConfig]) -> Result<(), CloudInitError> {
    for user in users {
        match user {
            UserConfig::Name(name) => {
                create_user_simple(name).await?;
            }
            UserConfig::Full(config) => {
                create_user_full(config).await?;
            }
        }
    }
    Ok(())
}

async fn create_user_simple(name: &str) -> Result<(), CloudInitError> {
    info!("Creating user: {}", name);

    let output = tokio::process::Command::new("useradd")
        .args(["--create-home", name])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    // Exit code 9 means user already exists, which is fine
    if !output.status.success() && output.status.code() != Some(9) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::UserGroup(format!(
            "Failed to create user {}: {}",
            name, stderr
        )));
    }

    Ok(())
}

async fn create_user_full(config: &UserFullConfig) -> Result<(), CloudInitError> {
    info!("Creating user with full config: {}", config.name);

    let mut cmd = tokio::process::Command::new("useradd");
    cmd.arg("--create-home");

    if let Some(shell) = &config.shell {
        cmd.args(["--shell", shell]);
    }

    if let Some(homedir) = &config.homedir {
        cmd.args(["--home-dir", homedir]);
    }

    if let Some(gecos) = &config.gecos {
        cmd.args(["--comment", gecos]);
    }

    if let Some(uid) = config.uid {
        cmd.args(["--uid", &uid.to_string()]);
    }

    if config.system == Some(true) {
        cmd.arg("--system");
    }

    cmd.arg(&config.name);

    let output = cmd
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    // Exit code 9 means user already exists
    if !output.status.success() && output.status.code() != Some(9) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::UserGroup(format!(
            "Failed to create user {}: {}",
            config.name, stderr
        )));
    }

    // Add to supplementary groups
    if !config.groups.is_empty() {
        debug!("Adding user {} to groups: {:?}", config.name, config.groups);
        let groups = config.groups.join(",");
        let output = tokio::process::Command::new("usermod")
            .args(["--append", "--groups", &groups, &config.name])
            .output()
            .await
            .map_err(|e| CloudInitError::Command(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CloudInitError::UserGroup(format!(
                "Failed to add user {} to groups: {}",
                config.name, stderr
            )));
        }
    }

    // Configure SSH keys
    if !config.ssh_authorized_keys.is_empty() {
        crate::modules::ssh_keys::configure_user_ssh_keys(
            &config.name,
            &config.ssh_authorized_keys,
        )
        .await?;
    }

    Ok(())
}
