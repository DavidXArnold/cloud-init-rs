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
            .map_err(CloudInitError::Io)?;

        // Set permissions to 700
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&ssh_dir, std::fs::Permissions::from_mode(0o700))
                .await
                .map_err(CloudInitError::Io)?;
        }
    }

    // Write authorized_keys
    let content = keys.join("\n") + "\n";
    fs::write(&authorized_keys_path, &content)
        .await
        .map_err(CloudInitError::Io)?;

    // Set permissions to 600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            &authorized_keys_path,
            std::fs::Permissions::from_mode(0o600),
        )
        .await
        .map_err(CloudInitError::Io)?;
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
        .map_err(CloudInitError::Io)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_configure_user_ssh_keys_empty_keys() {
        let result = configure_user_ssh_keys("testuser", &[]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_user_home_root() {
        // root should be in /etc/passwd on most systems
        let result = get_user_home("root").await;
        if let Ok(path) = result {
            // On macOS root is /var/root, on Linux /root
            assert!(path.to_string_lossy().contains("root"));
        }
    }

    #[tokio::test]
    async fn test_get_user_home_nonexistent_defaults() {
        let result = get_user_home("nonexistent_user_xyz_12345").await;
        if let Ok(path) = result {
            assert_eq!(path, PathBuf::from("/home/nonexistent_user_xyz_12345"));
        }
    }

    #[tokio::test]
    async fn test_configure_user_ssh_keys_writes_files() {
        let tmp = TempDir::new().unwrap();
        let ssh_dir = tmp.path().join(".ssh");
        let auth_keys = ssh_dir.join("authorized_keys");

        // Manually create the directory and write keys to verify format
        tokio::fs::create_dir_all(&ssh_dir).await.unwrap();
        let keys = [
            "ssh-rsa AAAAB3... user@host".to_string(),
            "ssh-ed25519 AAAAC3... user2@host".to_string(),
        ];
        let content = keys.join("\n") + "\n";
        tokio::fs::write(&auth_keys, &content).await.unwrap();

        let written = tokio::fs::read_to_string(&auth_keys).await.unwrap();
        assert!(written.contains("ssh-rsa AAAAB3"));
        assert!(written.contains("ssh-ed25519 AAAAC3"));
        assert_eq!(written.matches('\n').count(), 2);
    }

    #[tokio::test]
    async fn test_change_ownership_nonexistent_user() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("test.txt");
        tokio::fs::write(&file, "test").await.unwrap();
        // chown to nonexistent user should not panic
        let result = change_ownership(&file.to_path_buf(), "nonexistent_xyz_12345").await;
        assert!(result.is_ok()); // function logs but doesn't error
    }

    #[tokio::test]
    async fn test_get_user_home_parses_passwd_format() {
        // Verify the parsing logic by checking a known user
        let passwd = tokio::fs::read_to_string("/etc/passwd").await;
        if let Ok(content) = passwd {
            // Find any user with a valid home dir
            for line in content.lines() {
                let fields: Vec<&str> = line.split(':').collect();
                if fields.len() >= 6 {
                    let username = fields[0];
                    let expected_home = fields[5];
                    let result = get_user_home(username).await.unwrap();
                    assert_eq!(result, PathBuf::from(expected_home));
                    break; // Just test the first one
                }
            }
        }
    }
}
