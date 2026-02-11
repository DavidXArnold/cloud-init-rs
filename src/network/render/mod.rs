//! Network configuration renderers
//!
//! Converts NetworkConfig to system-specific configuration files.
//!
//! Supported renderers:
//! - `networkd` - systemd-networkd (*.network files)
//! - `network_manager` - NetworkManager (*.nmconnection files)
//! - `eni` - Debian ENI (/etc/network/interfaces)

pub mod eni;
pub mod network_manager;
pub mod networkd;

use crate::CloudInitError;
use crate::network::NetworkConfig;
use std::path::Path;
use tracing::{debug, info};

/// Network renderer types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererType {
    /// systemd-networkd
    Networkd,
    /// NetworkManager
    NetworkManager,
    /// Debian ENI (/etc/network/interfaces)
    Eni,
}

impl RendererType {
    /// Detect the appropriate renderer for this system
    pub async fn detect() -> Option<Self> {
        // Check for systemd-networkd
        if Path::new("/run/systemd/system").exists()
            && Path::new("/lib/systemd/systemd-networkd").exists()
        {
            return Some(Self::Networkd);
        }

        // Check for NetworkManager
        if Path::new("/usr/sbin/NetworkManager").exists() || Path::new("/usr/bin/nmcli").exists() {
            return Some(Self::NetworkManager);
        }

        // Check for ENI (Debian/Ubuntu without systemd)
        if Path::new("/etc/network/interfaces").exists() {
            return Some(Self::Eni);
        }

        None
    }

    /// Get renderer from string hint
    pub fn from_hint(hint: &str) -> Option<Self> {
        match hint.to_lowercase().as_str() {
            "networkd" | "systemd-networkd" => Some(Self::Networkd),
            "networkmanager" | "network-manager" | "nm" => Some(Self::NetworkManager),
            "eni" | "interfaces" | "ifupdown" => Some(Self::Eni),
            _ => None,
        }
    }
}

/// Trait for network configuration renderers
pub trait Renderer {
    /// Render network configuration to files
    fn render(
        &self,
        config: &NetworkConfig,
        output_dir: &Path,
    ) -> Result<Vec<RenderedFile>, CloudInitError>;

    /// Get the renderer type
    fn renderer_type(&self) -> RendererType;

    /// Check if this renderer is available on the system
    fn is_available(&self) -> bool;
}

/// A rendered configuration file
#[derive(Debug, Clone)]
pub struct RenderedFile {
    /// File path (relative to output directory)
    pub path: String,
    /// File contents
    pub content: String,
    /// File permissions (octal)
    pub mode: u32,
}

/// Apply network configuration using the appropriate renderer
pub async fn apply_network_config(
    config: &NetworkConfig,
    renderer_hint: Option<&str>,
) -> Result<(), CloudInitError> {
    // Determine renderer
    let renderer_type = if let Some(hint) = renderer_hint {
        RendererType::from_hint(hint)
    } else if let Some(hint) = &config.renderer {
        RendererType::from_hint(hint)
    } else {
        RendererType::detect().await
    };

    let renderer_type = renderer_type.ok_or_else(|| CloudInitError::Module {
        module: "network".to_string(),
        message: "No suitable network renderer found".to_string(),
    })?;

    info!("Using network renderer: {:?}", renderer_type);

    // Get output directory based on renderer
    let output_dir = match renderer_type {
        RendererType::Networkd => Path::new("/etc/systemd/network"),
        RendererType::NetworkManager => Path::new("/etc/NetworkManager/system-connections"),
        RendererType::Eni => Path::new("/etc/network"),
    };

    // Create renderer and render files
    let files = match renderer_type {
        RendererType::Networkd => {
            let renderer = networkd::NetworkdRenderer::new();
            renderer.render(config, output_dir)?
        }
        RendererType::NetworkManager => {
            let renderer = network_manager::NetworkManagerRenderer::new();
            renderer.render(config, output_dir)?
        }
        RendererType::Eni => {
            let renderer = eni::EniRenderer::new();
            renderer.render(config, output_dir)?
        }
    };

    // Write files
    for file in &files {
        let full_path = output_dir.join(&file.path);
        debug!("Writing network config: {}", full_path.display());

        // Create parent directories
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Write file
        tokio::fs::write(&full_path, &file.content).await?;

        // Set permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&full_path, std::fs::Permissions::from_mode(file.mode))
                .await?;
        }
    }

    info!("Wrote {} network configuration files", files.len());

    // Reload/restart network service
    match renderer_type {
        RendererType::Networkd => {
            reload_networkd().await?;
        }
        RendererType::NetworkManager => {
            reload_network_manager().await?;
        }
        RendererType::Eni => {
            // ENI typically requires ifup/ifdown or reboot
            debug!("ENI config written, may require ifup or reboot");
        }
    }

    Ok(())
}

/// Reload systemd-networkd
async fn reload_networkd() -> Result<(), CloudInitError> {
    debug!("Reloading systemd-networkd");

    let output = tokio::process::Command::new("networkctl")
        .arg("reload")
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => {
            info!("systemd-networkd reloaded");
            Ok(())
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            debug!("networkctl reload failed: {}", stderr);
            // Try systemctl restart as fallback
            let _ = tokio::process::Command::new("systemctl")
                .args(["restart", "systemd-networkd"])
                .output()
                .await;
            Ok(())
        }
        Err(e) => {
            debug!("networkctl not available: {}", e);
            Ok(())
        }
    }
}

/// Reload NetworkManager
async fn reload_network_manager() -> Result<(), CloudInitError> {
    debug!("Reloading NetworkManager connections");

    let output = tokio::process::Command::new("nmcli")
        .args(["connection", "reload"])
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => {
            info!("NetworkManager connections reloaded");
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            debug!("nmcli reload failed: {}", stderr);
        }
        Err(e) => {
            debug!("nmcli not available: {}", e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_from_hint() {
        assert_eq!(
            RendererType::from_hint("networkd"),
            Some(RendererType::Networkd)
        );
        assert_eq!(
            RendererType::from_hint("NetworkManager"),
            Some(RendererType::NetworkManager)
        );
        assert_eq!(RendererType::from_hint("eni"), Some(RendererType::Eni));
        assert_eq!(RendererType::from_hint("unknown"), None);
    }
}
