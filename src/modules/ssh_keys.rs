//! SSH key configuration module

use crate::CloudInitError;
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, info};

/// Configure SSH authorized keys for a user
pub async fn configure_user_ssh_keys(
    username: &str,
    keys: &[String],
) -> Result<(), CloudInitError> {
    if keys.is_empty() {
        return Ok(());
    }

    info!("Configuring {} SSH keys for user {}", keys.len(), username);

    // Get user's home directory
    let home_dir = get_user_home(username).await?;
    let ssh_dir = home_dir.join(".ssh");
    let authorized_keys_path = ssh_dir.join("authorized_keys");

    // Create .ssh directory if it doesn't exist
    if !ssh_dir.exists() {
        debug!("Creating SSH directory: {:?}", ssh_dir);
        fs::create_dir_all(&ssh_dir)
            .await
            .map_err(|e| CloudInitError::Io(e))?;

        // Set permissions to 700
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&ssh_dir, std::fs::Permissions::from_mode(0o700))
                .await
                .map_err(|e| CloudInitError::Io(e))?;
        }
    }

    // Write authorized_keys
    let content = keys.join("\n") + "\n";
    fs::write(&authorized_keys_path, &content)
        .await
        .map_err(|e| CloudInitError::Io(e))?;

    // Set permissions to 600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&authorized_keys_path, std::fs::Permissions::from_mode(0o600))
            .await
            .map_err(|e| CloudInitError::Io(e))?;
    }

    // Change ownership to the user
    change_ownership(&ssh_dir, username).await?;
    change_ownership(&authorized_keys_path, username).await?;

    Ok(())
}

async fn get_user_home(username: &str) -> Result<PathBuf, CloudInitError> {
    // Read /etc/passwd to find home directory
    let passwd = fs::read_to_string("/etc/passwd")
        .await
        .map_err(|e| CloudInitError::Io(e))?;

    for line in passwd.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() >= 6 && fields[0] == username {
            return Ok(PathBuf::from(fields[5]));
        }
    }

    // Default to /home/username
    Ok(PathBuf::from(format!("/home/{}", username)))
}

async fn change_ownership(path: &PathBuf, username: &str) -> Result<(), CloudInitError> {
    let output = tokio::process::Command::new("chown")
        .args([username, &path.to_string_lossy()])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        debug!(
            "Failed to change ownership of {:?}: {}",
            path,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}
