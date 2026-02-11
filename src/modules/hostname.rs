//! Hostname configuration module

use crate::CloudInitError;
use tokio::fs;
use tracing::{debug, info};

/// Set the system hostname
pub async fn set_hostname(hostname: &str) -> Result<(), CloudInitError> {
    info!("Setting hostname to: {}", hostname);

    // Write to /etc/hostname
    fs::write("/etc/hostname", format!("{}\n", hostname))
        .await
        .map_err(CloudInitError::Io)?;

    // Try hostnamectl first (systemd)
    if try_hostnamectl(hostname).await? {
        return Ok(());
    }

    // Fallback: hostname command
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

/// Set hostname with FQDN support
pub async fn set_hostname_fqdn(
    hostname: &str,
    fqdn: Option<&str>,
    manage_etc_hosts: bool,
) -> Result<(), CloudInitError> {
    // Set the short hostname
    set_hostname(hostname).await?;

    // If we have an FQDN and should manage /etc/hosts
    if manage_etc_hosts {
        let fqdn = fqdn.unwrap_or(hostname);
        update_etc_hosts(hostname, fqdn).await?;
    }

    Ok(())
}

/// Try to set hostname via hostnamectl (systemd)
async fn try_hostnamectl(hostname: &str) -> Result<bool, CloudInitError> {
    debug!("Attempting to set hostname via hostnamectl");

    let output = tokio::process::Command::new("hostnamectl")
        .args(["set-hostname", hostname])
        .output()
        .await;

    match output {
        Ok(output) if output.status.success() => {
            info!("Hostname set via hostnamectl");
            Ok(true)
        }
        Ok(output) => {
            debug!(
                "hostnamectl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            Ok(false)
        }
        Err(e) => {
            debug!("hostnamectl not available: {}", e);
            Ok(false)
        }
    }
}

/// Update /etc/hosts with hostname entries
pub async fn update_etc_hosts(hostname: &str, fqdn: &str) -> Result<(), CloudInitError> {
    debug!(
        "Updating /etc/hosts for hostname: {}, fqdn: {}",
        hostname, fqdn
    );

    let hosts_path = "/etc/hosts";

    // Read existing hosts file
    let existing = fs::read_to_string(hosts_path)
        .await
        .unwrap_or_else(|_| String::new());

    // Build new hosts file
    let mut new_lines: Vec<String> = Vec::new();
    let mut found_127_0_0_1 = false;
    let mut found_127_0_1_1 = false;

    for line in existing.lines() {
        let trimmed = line.trim();

        // Skip empty lines and comments for now, we'll add them back
        if trimmed.is_empty() || trimmed.starts_with('#') {
            new_lines.push(line.to_string());
            continue;
        }

        // Parse the line to get IP and hostnames
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            new_lines.push(line.to_string());
            continue;
        }

        let ip = parts[0];

        if ip == "127.0.0.1" {
            // Keep localhost entries, add our hostname
            found_127_0_0_1 = true;
            new_lines.push(format!("127.0.0.1 localhost {}", hostname));
        } else if ip == "127.0.1.1" {
            // This is traditionally used for the FQDN
            found_127_0_1_1 = true;
            if fqdn != hostname {
                new_lines.push(format!("127.0.1.1 {} {}", fqdn, hostname));
            } else {
                new_lines.push(format!("127.0.1.1 {}", hostname));
            }
        } else {
            // Keep other entries as-is
            new_lines.push(line.to_string());
        }
    }

    // Add missing entries
    if !found_127_0_0_1 {
        new_lines.insert(0, format!("127.0.0.1 localhost {}", hostname));
    }
    if !found_127_0_1_1 && fqdn != hostname {
        // Find position after 127.0.0.1 line
        let pos = new_lines
            .iter()
            .position(|l| l.starts_with("127.0.0.1"))
            .map(|p| p + 1)
            .unwrap_or(0);
        new_lines.insert(pos, format!("127.0.1.1 {} {}", fqdn, hostname));
    }

    // Write back
    let content = new_lines.join("\n") + "\n";
    fs::write(hosts_path, &content)
        .await
        .map_err(CloudInitError::Io)?;

    info!("Updated /etc/hosts");
    Ok(())
}
