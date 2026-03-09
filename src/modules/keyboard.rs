//! Keyboard layout configuration module

use crate::CloudInitError;
use crate::config::KeyboardConfig;
use std::path::Path;
use tokio::fs;
use tracing::{debug, info};

/// Set the keyboard layout configuration
pub async fn set_keyboard(config: &KeyboardConfig) -> Result<(), CloudInitError> {
    info!("Setting keyboard layout to: {}", config.layout);

    // Try localectl first (systemd systems)
    if try_localectl(config).await? {
        return Ok(());
    }

    // Fallback: write to /etc/default/keyboard (Debian/Ubuntu)
    write_default_keyboard(config).await?;

    Ok(())
}

/// Try to configure keyboard via localectl (systemd)
async fn try_localectl(config: &KeyboardConfig) -> Result<bool, CloudInitError> {
    debug!("Attempting to set keyboard layout via localectl");

    // localectl set-x11-keymap LAYOUT [MODEL [VARIANT [OPTIONS]]]
    let mut args = vec!["set-x11-keymap".to_string(), config.layout.clone()];

    // If variant or options are provided, model must also be included (even if empty)
    let model = config.model.as_deref().unwrap_or("");
    let variant = config.variant.as_deref().unwrap_or("");
    let options = config.options.as_deref().unwrap_or("");

    if !variant.is_empty() || !options.is_empty() || !model.is_empty() {
        args.push(model.to_string());
    }
    if !variant.is_empty() || !options.is_empty() {
        args.push(variant.to_string());
    }
    if !options.is_empty() {
        args.push(options.to_string());
    }

    let output = tokio::process::Command::new("localectl")
        .args(&args)
        .output()
        .await;

    match output {
        Ok(output) if output.status.success() => {
            info!("Keyboard layout set via localectl");
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

/// Write /etc/default/keyboard (Debian/Ubuntu style)
async fn write_default_keyboard(config: &KeyboardConfig) -> Result<(), CloudInitError> {
    let keyboard_file = Path::new("/etc/default/keyboard");

    // Create parent directory if needed
    if let Some(parent) = keyboard_file.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(CloudInitError::Io)?;
    }

    let model = config.model.as_deref().unwrap_or("pc105");
    let variant = config.variant.as_deref().unwrap_or("");
    let options = config.options.as_deref().unwrap_or("");

    let content = format!(
        "XKBMODEL=\"{}\"\nXKBLAYOUT=\"{}\"\nXKBVARIANT=\"{}\"\nXKBOPTIONS=\"{}\"\nBACKSPACE=\"guess\"\n",
        model, config.layout, variant, options
    );

    fs::write(keyboard_file, &content)
        .await
        .map_err(CloudInitError::Io)?;

    debug!("Wrote /etc/default/keyboard");
    Ok(())
}
