//! NTP configuration module
//!
//! Configures NTP time synchronization via chrony, systemd-timesyncd, or ntpd.

use crate::CloudInitError;
use std::path::Path;
use tokio::fs;
use tracing::{debug, info, warn};

/// NTP configuration
#[derive(Debug, Clone)]
pub struct NtpConfig {
    /// NTP servers to use
    pub servers: Vec<String>,
    /// NTP pools to use
    pub pools: Vec<String>,
    /// Enable NTP (default: true)
    pub enabled: bool,
}

impl Default for NtpConfig {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            pools: vec!["pool.ntp.org".to_string()],
            enabled: true,
        }
    }
}

/// Configure NTP based on available service
pub async fn configure_ntp(config: &NtpConfig) -> Result<(), CloudInitError> {
    if !config.enabled {
        info!("NTP disabled by configuration");
        return Ok(());
    }

    info!("Configuring NTP");

    // Try services in order of preference
    if try_configure_chrony(config).await? {
        return Ok(());
    }

    if try_configure_timesyncd(config).await? {
        return Ok(());
    }

    if try_configure_ntpd(config).await? {
        return Ok(());
    }

    warn!("No supported NTP service found");
    Ok(())
}

/// Configure chrony (preferred on RHEL/Fedora/newer Ubuntu)
async fn try_configure_chrony(config: &NtpConfig) -> Result<bool, CloudInitError> {
    let chrony_conf = Path::new("/etc/chrony.conf");
    let chrony_d = Path::new("/etc/chrony/chrony.conf");

    let conf_path = if chrony_conf.exists() {
        chrony_conf
    } else if chrony_d.exists() {
        chrony_d
    } else {
        debug!("Chrony not found");
        return Ok(false);
    };

    info!("Configuring chrony");

    // Build configuration
    let mut content = String::new();
    content.push_str("# Configured by cloud-init-rs\n");

    for server in &config.servers {
        content.push_str(&format!("server {} iburst\n", server));
    }

    for pool in &config.pools {
        content.push_str(&format!("pool {} iburst\n", pool));
    }

    // Add common chrony options
    content.push_str("\n# Common settings\n");
    content.push_str("driftfile /var/lib/chrony/drift\n");
    content.push_str("makestep 1.0 3\n");
    content.push_str("rtcsync\n");

    fs::write(conf_path, &content)
        .await
        .map_err(CloudInitError::Io)?;

    // Restart chrony
    restart_service("chronyd").await?;

    Ok(true)
}

/// Configure systemd-timesyncd (default on many systemd systems)
async fn try_configure_timesyncd(config: &NtpConfig) -> Result<bool, CloudInitError> {
    let timesyncd_conf = Path::new("/etc/systemd/timesyncd.conf");

    // Check if systemd-timesyncd is available
    let status = tokio::process::Command::new("systemctl")
        .args(["is-enabled", "systemd-timesyncd"])
        .output()
        .await;

    if status.is_err() || !status.unwrap().status.success() {
        debug!("systemd-timesyncd not available");
        return Ok(false);
    }

    info!("Configuring systemd-timesyncd");

    // Build configuration
    let servers: Vec<&str> = config.servers.iter().map(|s| s.as_str()).collect();
    let pools: Vec<&str> = config.pools.iter().map(|s| s.as_str()).collect();

    let ntp_line = if !servers.is_empty() {
        servers.join(" ")
    } else {
        pools.join(" ")
    };

    let content = format!("# Configured by cloud-init-rs\n[Time]\nNTP={}\n", ntp_line);

    fs::write(timesyncd_conf, &content)
        .await
        .map_err(CloudInitError::Io)?;

    // Restart timesyncd
    restart_service("systemd-timesyncd").await?;

    Ok(true)
}

/// Configure ntpd (legacy systems)
async fn try_configure_ntpd(config: &NtpConfig) -> Result<bool, CloudInitError> {
    let ntp_conf = Path::new("/etc/ntp.conf");

    if !ntp_conf.exists() {
        debug!("ntpd not found");
        return Ok(false);
    }

    info!("Configuring ntpd");

    // Build configuration
    let mut content = String::new();
    content.push_str("# Configured by cloud-init-rs\n");
    content.push_str("driftfile /var/lib/ntp/drift\n\n");

    for server in &config.servers {
        content.push_str(&format!("server {} iburst\n", server));
    }

    for pool in &config.pools {
        content.push_str(&format!("pool {} iburst\n", pool));
    }

    // Restrict access
    content.push_str("\n# Access control\n");
    content.push_str("restrict default kod nomodify notrap nopeer noquery\n");
    content.push_str("restrict 127.0.0.1\n");
    content.push_str("restrict ::1\n");

    fs::write(ntp_conf, &content)
        .await
        .map_err(CloudInitError::Io)?;

    // Restart ntpd
    restart_service("ntpd").await?;

    Ok(true)
}

/// Restart a systemd service
async fn restart_service(service: &str) -> Result<(), CloudInitError> {
    debug!("Restarting service: {}", service);

    let output = tokio::process::Command::new("systemctl")
        .args(["restart", service])
        .output()
        .await;

    match output {
        Ok(output) if output.status.success() => {
            info!("Restarted {}", service);
            Ok(())
        }
        Ok(output) => {
            warn!(
                "Failed to restart {}: {}",
                service,
                String::from_utf8_lossy(&output.stderr)
            );
            Ok(())
        }
        Err(e) => {
            warn!("Could not restart {}: {}", service, e);
            Ok(())
        }
    }
}
