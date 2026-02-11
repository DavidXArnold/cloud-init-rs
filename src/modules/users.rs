//! User creation and configuration module

use crate::CloudInitError;
use crate::config::{UserConfig, UserFullConfig};
use std::path::Path;
use tokio::fs;
use tracing::{debug, info, warn};

/// Create users from cloud-config
pub async fn create_users(users: &[UserConfig]) -> Result<(), CloudInitError> {
    for user in users {
        match user {
            UserConfig::Name(name) => {
                // Handle special "default" user
                if name == "default" {
                    debug!("Skipping 'default' user (would use distro default)");
                    continue;
                }
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

    if let Some(primary_group) = &config.primary_group {
        cmd.args(["--gid", primary_group]);
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
        add_user_to_groups(&config.name, &config.groups).await?;
    }

    // Set password if provided
    if let Some(passwd) = &config.passwd {
        set_user_password(&config.name, passwd).await?;
    }

    // Lock password if requested
    if config.lock_passwd == Some(true) {
        lock_user_password(&config.name).await?;
    }

    // Configure sudo access
    if let Some(sudo) = &config.sudo {
        configure_sudo(&config.name, sudo).await?;
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

/// Add user to supplementary groups
async fn add_user_to_groups(username: &str, groups: &[String]) -> Result<(), CloudInitError> {
    debug!("Adding user {} to groups: {:?}", username, groups);
    let groups_str = groups.join(",");
    let output = tokio::process::Command::new("usermod")
        .args(["--append", "--groups", &groups_str, username])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::UserGroup(format!(
            "Failed to add user {} to groups: {}",
            username, stderr
        )));
    }
    Ok(())
}

/// Set user password (expects pre-hashed password)
async fn set_user_password(username: &str, hashed_password: &str) -> Result<(), CloudInitError> {
    debug!("Setting password for user {}", username);

    // Use chpasswd with -e for pre-encrypted passwords
    let input = format!("{}:{}", username, hashed_password);
    let mut child = tokio::process::Command::new("chpasswd")
        .arg("-e")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if let Some(stdin) = child.stdin.as_mut() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|e| CloudInitError::Command(e.to_string()))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::UserGroup(format!(
            "Failed to set password for {}: {}",
            username, stderr
        )));
    }

    Ok(())
}

/// Lock user password (disable password login)
async fn lock_user_password(username: &str) -> Result<(), CloudInitError> {
    debug!("Locking password for user {}", username);

    let output = tokio::process::Command::new("passwd")
        .args(["-l", username])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("Failed to lock password for {}: {}", username, stderr);
        // Don't fail - user may not have a password set
    }

    Ok(())
}

/// Configure sudo access for a user
async fn configure_sudo(username: &str, sudo_spec: &str) -> Result<(), CloudInitError> {
    debug!("Configuring sudo for user {}: {}", username, sudo_spec);

    // Create sudoers.d directory if it doesn't exist
    let sudoers_dir = Path::new("/etc/sudoers.d");
    if !sudoers_dir.exists() {
        fs::create_dir_all(sudoers_dir)
            .await
            .map_err(CloudInitError::Io)?;
    }

    // Write sudoers file for this user
    // Filename is 90-cloud-init-users to match Python cloud-init
    let sudoers_file = sudoers_dir.join(format!("90-cloud-init-{}", username));

    // Format: "username sudo_spec" or if sudo_spec contains username, use as-is
    let content = if sudo_spec.contains(username) || sudo_spec.starts_with("ALL") {
        // sudo_spec is complete (e.g., "ALL=(ALL) NOPASSWD:ALL")
        format!("{} {}\n", username, sudo_spec)
    } else {
        // sudo_spec is just the rule
        format!("{} {}\n", username, sudo_spec)
    };

    fs::write(&sudoers_file, &content)
        .await
        .map_err(CloudInitError::Io)?;

    // Set permissions to 0440 (required for sudoers files)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&sudoers_file, std::fs::Permissions::from_mode(0o440))
            .await
            .map_err(CloudInitError::Io)?;
    }

    // Validate sudoers file
    let output = tokio::process::Command::new("visudo")
        .args(["-c", "-f", &sudoers_file.to_string_lossy()])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        // Remove invalid sudoers file
        let _ = fs::remove_file(&sudoers_file).await;
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::UserGroup(format!(
            "Invalid sudoers configuration for {}: {}",
            username, stderr
        )));
    }

    info!("Configured sudo access for user {}", username);
    Ok(())
}
