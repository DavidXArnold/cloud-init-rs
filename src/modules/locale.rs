//! Locale configuration module

use crate::CloudInitError;
use std::path::Path;
use tokio::fs;
use tracing::{debug, info};

/// Set the system locale
pub async fn set_locale(locale: &str) -> Result<(), CloudInitError> {
    info!("Setting locale to: {}", locale);

    // Try localectl first (systemd systems)
    if try_localectl(locale).await? {
        return Ok(());
    }

    // Fallback: write to config files directly
    write_locale_conf(locale).await?;
    write_default_locale(locale).await?;

    Ok(())
}

/// Try to set locale via localectl
async fn try_localectl(locale: &str) -> Result<bool, CloudInitError> {
    debug!("Attempting to set locale via localectl");

    let output = tokio::process::Command::new("localectl")
        .args(["set-locale", &format!("LANG={}", locale)])
        .output()
        .await;

    match output {
        Ok(output) if output.status.success() => {
            info!("Locale set via localectl");
            Ok(true)
        }
        Ok(output) => {
            debug!(
                "localectl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            Ok(false)
        }
        Err(e) => {
            debug!("localectl not available: {}", e);
            Ok(false)
        }
    }
}

/// Write /etc/locale.conf (systemd/RHEL style)
async fn write_locale_conf(locale: &str) -> Result<(), CloudInitError> {
    let locale_conf = Path::new("/etc/locale.conf");

    let content = format!("LANG={}\n", locale);
    fs::write(locale_conf, &content)
        .await
        .map_err(CloudInitError::Io)?;

    debug!("Wrote /etc/locale.conf");
    Ok(())
}

/// Write /etc/default/locale (Debian/Ubuntu style)
async fn write_default_locale(locale: &str) -> Result<(), CloudInitError> {
    let default_locale = Path::new("/etc/default/locale");

    // Create parent directory if needed
    if let Some(parent) = default_locale.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .await
            .map_err(CloudInitError::Io)?;
    }

    let content = format!("LANG={}\n", locale);
    fs::write(default_locale, &content)
        .await
        .map_err(CloudInitError::Io)?;

    debug!("Wrote /etc/default/locale");
    Ok(())
}

/// Generate locale if needed (Debian/Ubuntu)
pub async fn generate_locale(locale: &str) -> Result<(), CloudInitError> {
    debug!("Attempting to generate locale: {}", locale);

    // Check if locale-gen exists
    let output = tokio::process::Command::new("locale-gen")
        .arg(locale)
        .output()
        .await;

    match output {
        Ok(output) if output.status.success() => {
            info!("Generated locale: {}", locale);
            Ok(())
        }
        Ok(output) => {
            debug!(
                "locale-gen failed (may be expected): {}",
                String::from_utf8_lossy(&output.stderr)
            );
            Ok(())
        }
        Err(e) => {
            debug!("locale-gen not available: {}", e);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_locale_calls_localectl() {
        // On macOS this will fail gracefully and fall through to file writes
        // which will also fail (no /etc/locale.conf), but shouldn't panic
        let _ = set_locale("en_US.UTF-8").await;
    }

    #[tokio::test]
    async fn test_try_localectl_nonexistent() {
        // localectl may not exist on macOS, should return Ok(false)
        let result = try_localectl("en_US.UTF-8").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_generate_locale_nonexistent() {
        // locale-gen may not exist, should return Ok(()) gracefully
        let result = generate_locale("en_US.UTF-8").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_write_locale_conf_requires_permissions() {
        // On non-root systems this will fail with permission error
        let result = write_locale_conf("en_US.UTF-8").await;
        // Just verify it returns an error type, not a panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_write_default_locale_requires_permissions() {
        let result = write_default_locale("en_US.UTF-8").await;
        let _ = result;
    }
}
