//! Hostname configuration module

use crate::CloudInitError;
use tracing::debug;

/// Set the system hostname
pub async fn set_hostname(hostname: &str) -> Result<(), CloudInitError> {
    debug!("Setting hostname to: {}", hostname);

    // Write to /etc/hostname
    tokio::fs::write("/etc/hostname", format!("{}\n", hostname))
        .await
        .map_err(CloudInitError::Io)?;

    // Call hostname command to set it immediately
    let output = tokio::process::Command::new("hostname")
        .arg(hostname)
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::Command(format!(
            "Failed to set hostname: {}",
            stderr
        )));
    }

    Ok(())
}
