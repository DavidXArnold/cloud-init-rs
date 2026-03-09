//! Timezone configuration module

use crate::CloudInitError;
use std::path::Path;
use tokio::fs;
use tracing::{debug, info};

/// Set the system timezone
pub async fn set_timezone(timezone: &str) -> Result<(), CloudInitError> {
    info!("Setting timezone to: {}", timezone);

    // Validate timezone exists
    let zoneinfo_path = format!("/usr/share/zoneinfo/{}", timezone);
    if !Path::new(&zoneinfo_path).exists() {
        return Err(CloudInitError::InvalidData(format!(
            "Invalid timezone: {} (not found in /usr/share/zoneinfo)",
            timezone
        )));
    }

    // Try timedatectl first (systemd systems)
    if try_timedatectl(timezone).await? {
        return Ok(());
    }

    // Fallback: symlink /etc/localtime
    set_localtime_symlink(timezone).await?;

    // Also write /etc/timezone for Debian-based systems
    write_etc_timezone(timezone).await?;

    Ok(())
}

/// Try to set timezone via timedatectl
async fn try_timedatectl(timezone: &str) -> Result<bool, CloudInitError> {
    debug!("Attempting to set timezone via timedatectl");

    let output = tokio::process::Command::new("timedatectl")
        .args(["set-timezone", timezone])
        .output()
        .await;

    match output {
        Ok(output) if output.status.success() => {
            info!("Timezone set via timedatectl");
            Ok(true)
        }
        Ok(output) => {
            debug!(
                "timedatectl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            Ok(false)
        }
        Err(e) => {
            debug!("timedatectl not available: {}", e);
            Ok(false)
        }
    }
}

/// Set /etc/localtime symlink
async fn set_localtime_symlink(timezone: &str) -> Result<(), CloudInitError> {
    debug!("Setting /etc/localtime symlink");

    let localtime = Path::new("/etc/localtime");
    let zoneinfo = format!("/usr/share/zoneinfo/{}", timezone);

    // Remove existing localtime if it exists
    if localtime.exists() {
        fs::remove_file(localtime)
            .await
            .map_err(CloudInitError::Io)?;
    }

    // Create symlink
    #[cfg(unix)]
    {
        tokio::fs::symlink(&zoneinfo, localtime)
            .await
            .map_err(CloudInitError::Io)?;
    }

    info!("Created /etc/localtime symlink to {}", zoneinfo);
    Ok(())
}

/// Write /etc/timezone file (Debian/Ubuntu)
async fn write_etc_timezone(timezone: &str) -> Result<(), CloudInitError> {
    let etc_timezone = Path::new("/etc/timezone");

    fs::write(etc_timezone, format!("{}\n", timezone))
        .await
        .map_err(CloudInitError::Io)?;

    debug!("Wrote /etc/timezone");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_timezone_invalid() {
        let result = set_timezone("Invalid/Not_A_Timezone").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid timezone"));
    }

    #[tokio::test]
    async fn test_set_timezone_valid_utc() {
        // UTC exists on most systems in /usr/share/zoneinfo
        let result = set_timezone("UTC").await;
        // May fail due to permissions but should not return InvalidData
        if let Err(e) = &result {
            let msg = e.to_string();
            assert!(
                !msg.contains("Invalid timezone"),
                "UTC should be a valid timezone"
            );
        }
    }

    #[tokio::test]
    async fn test_try_timedatectl_nonexistent_tz() {
        // timedatectl may not exist on macOS
        let result = try_timedatectl("US/Eastern").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_set_timezone_america_new_york() {
        let result = set_timezone("America/New_York").await;
        if let Err(e) = &result {
            let msg = e.to_string();
            assert!(!msg.contains("Invalid timezone"));
        }
    }

    #[tokio::test]
    async fn test_set_timezone_empty_string() {
        let result = set_timezone("").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_timezone_path_traversal() {
        let result = set_timezone("../../etc/passwd").await;
        // This should fail - the file won't exist in zoneinfo
        assert!(result.is_err());
    }
}
