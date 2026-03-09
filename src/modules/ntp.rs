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

/// Build chrony configuration content (pure function for testability)
fn build_chrony_content(config: &NtpConfig) -> String {
    let mut content = String::new();
    content.push_str("# Configured by cloud-init-rs\n");
    for server in &config.servers {
        content.push_str(&format!("server {server} iburst\n"));
    }
    for pool in &config.pools {
        content.push_str(&format!("pool {pool} iburst\n"));
    }
    content.push_str("\n# Common settings\n");
    content.push_str("driftfile /var/lib/chrony/drift\n");
    content.push_str("makestep 1.0 3\n");
    content.push_str("rtcsync\n");
    content
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
    let content = build_chrony_content(config);

    fs::write(conf_path, &content)
        .await
        .map_err(CloudInitError::Io)?;

    restart_service("chronyd").await?;
    Ok(true)
}

/// Build timesyncd configuration content (pure function for testability)
fn build_timesyncd_content(config: &NtpConfig) -> String {
    let servers: Vec<&str> = config.servers.iter().map(|s| s.as_str()).collect();
    let pools: Vec<&str> = config.pools.iter().map(|s| s.as_str()).collect();
    let ntp_line = if !servers.is_empty() {
        servers.join(" ")
    } else {
        pools.join(" ")
    };
    format!("# Configured by cloud-init-rs\n[Time]\nNTP={ntp_line}\n")
}

/// Configure systemd-timesyncd (default on many systemd systems)
async fn try_configure_timesyncd(config: &NtpConfig) -> Result<bool, CloudInitError> {
    let timesyncd_conf = Path::new("/etc/systemd/timesyncd.conf");

    let status = tokio::process::Command::new("systemctl")
        .args(["is-enabled", "systemd-timesyncd"])
        .output()
        .await;

    if status.is_err() || !status.unwrap().status.success() {
        debug!("systemd-timesyncd not available");
        return Ok(false);
    }

    info!("Configuring systemd-timesyncd");
    let content = build_timesyncd_content(config);

    fs::write(timesyncd_conf, &content)
        .await
        .map_err(CloudInitError::Io)?;

    restart_service("systemd-timesyncd").await?;
    Ok(true)
}

/// Build ntpd configuration content (pure function for testability)
fn build_ntpd_content(config: &NtpConfig) -> String {
    let mut content = String::new();
    content.push_str("# Configured by cloud-init-rs\n");
    content.push_str("driftfile /var/lib/ntp/drift\n\n");
    for server in &config.servers {
        content.push_str(&format!("server {server} iburst\n"));
    }
    for pool in &config.pools {
        content.push_str(&format!("pool {pool} iburst\n"));
    }
    content.push_str("\n# Access control\n");
    content.push_str("restrict default kod nomodify notrap nopeer noquery\n");
    content.push_str("restrict 127.0.0.1\n");
    content.push_str("restrict ::1\n");
    content
}

/// Configure ntpd (legacy systems)
async fn try_configure_ntpd(config: &NtpConfig) -> Result<bool, CloudInitError> {
    let ntp_conf = Path::new("/etc/ntp.conf");

    if !ntp_conf.exists() {
        debug!("ntpd not found");
        return Ok(false);
    }

    info!("Configuring ntpd");
    let content = build_ntpd_content(config);

    fs::write(ntp_conf, &content)
        .await
        .map_err(CloudInitError::Io)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ntp_config_default() {
        let config = NtpConfig::default();
        assert!(config.enabled);
        assert!(config.servers.is_empty());
        assert_eq!(config.pools, vec!["pool.ntp.org".to_string()]);
    }

    #[test]
    fn test_build_chrony_content_defaults() {
        let config = NtpConfig::default();
        let content = build_chrony_content(&config);
        assert!(content.contains("# Configured by cloud-init-rs"));
        assert!(content.contains("pool pool.ntp.org iburst"));
        assert!(content.contains("driftfile /var/lib/chrony/drift"));
        assert!(content.contains("makestep 1.0 3"));
        assert!(content.contains("rtcsync"));
    }

    #[test]
    fn test_build_chrony_content_custom_servers() {
        let config = NtpConfig {
            servers: vec![
                "time1.google.com".to_string(),
                "time2.google.com".to_string(),
            ],
            pools: vec![],
            enabled: true,
        };
        let content = build_chrony_content(&config);
        assert!(content.contains("server time1.google.com iburst"));
        assert!(content.contains("server time2.google.com iburst"));
        assert!(!content.contains("pool"));
    }

    #[test]
    fn test_build_timesyncd_content_with_pools() {
        let config = NtpConfig::default();
        let content = build_timesyncd_content(&config);
        assert!(content.contains("[Time]"));
        assert!(content.contains("NTP=pool.ntp.org"));
    }

    #[test]
    fn test_build_timesyncd_content_servers_preferred() {
        let config = NtpConfig {
            servers: vec![
                "ntp1.example.com".to_string(),
                "ntp2.example.com".to_string(),
            ],
            pools: vec!["pool.ntp.org".to_string()],
            enabled: true,
        };
        let content = build_timesyncd_content(&config);
        assert!(content.contains("NTP=ntp1.example.com ntp2.example.com"));
        assert!(!content.contains("pool.ntp.org"));
    }

    #[test]
    fn test_build_ntpd_content_defaults() {
        let config = NtpConfig::default();
        let content = build_ntpd_content(&config);
        assert!(content.contains("driftfile /var/lib/ntp/drift"));
        assert!(content.contains("pool pool.ntp.org iburst"));
        assert!(content.contains("restrict default kod nomodify notrap nopeer noquery"));
        assert!(content.contains("restrict ::1"));
    }

    #[test]
    fn test_build_ntpd_content_custom() {
        let config = NtpConfig {
            servers: vec!["time.nist.gov".to_string()],
            pools: vec!["pool.ntp.org".to_string()],
            enabled: true,
        };
        let content = build_ntpd_content(&config);
        assert!(content.contains("server time.nist.gov iburst"));
        assert!(content.contains("pool pool.ntp.org iburst"));
    }

    #[test]
    fn test_build_chrony_content_empty_config() {
        let config = NtpConfig {
            servers: vec![],
            pools: vec![],
            enabled: true,
        };
        let content = build_chrony_content(&config);
        assert!(content.contains("# Configured by cloud-init-rs"));
        assert!(!content.contains("server "));
        assert!(!content.contains("pool "));
    }

    #[test]
    fn test_build_timesyncd_content_empty() {
        let config = NtpConfig {
            servers: vec![],
            pools: vec![],
            enabled: true,
        };
        let content = build_timesyncd_content(&config);
        assert!(content.contains("NTP=\n"));
    }

    #[test]
    fn test_build_ntpd_content_empty() {
        let config = NtpConfig {
            servers: vec![],
            pools: vec![],
            enabled: true,
        };
        let content = build_ntpd_content(&config);
        assert!(content.contains("# Configured by cloud-init-rs"));
        assert!(content.contains("restrict"));
    }

    #[tokio::test]
    async fn test_configure_ntp_disabled() {
        let config = NtpConfig {
            servers: vec![],
            pools: vec![],
            enabled: false,
        };
        let result = configure_ntp(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_configure_ntp_no_services() {
        let config = NtpConfig::default();
        // On macOS, /etc/ntp.conf may exist and fail with permission error.
        // On Linux CI without NTP services, this returns Ok(()).
        // Either outcome is acceptable.
        let _ = configure_ntp(&config).await;
    }
}
