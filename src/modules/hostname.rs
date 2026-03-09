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
    let existing = fs::read_to_string(hosts_path)
        .await
        .unwrap_or_else(|_| String::new());

    let content = build_hosts_content(&existing, hostname, fqdn);

    fs::write(hosts_path, &content)
        .await
        .map_err(CloudInitError::Io)?;

    info!("Updated /etc/hosts");
    Ok(())
}

/// Build the content for /etc/hosts (pure function for testability)
fn build_hosts_content(existing: &str, hostname: &str, fqdn: &str) -> String {
    let mut new_lines: Vec<String> = Vec::new();
    let mut found_127_0_0_1 = false;
    let mut found_127_0_1_1 = false;

    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            new_lines.push(line.to_string());
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            new_lines.push(line.to_string());
            continue;
        }
        let ip = parts[0];
        if ip == "127.0.0.1" {
            found_127_0_0_1 = true;
            new_lines.push(format!("127.0.0.1 localhost {hostname}"));
        } else if ip == "127.0.1.1" {
            found_127_0_1_1 = true;
            if fqdn != hostname {
                new_lines.push(format!("127.0.1.1 {fqdn} {hostname}"));
            } else {
                new_lines.push(format!("127.0.1.1 {hostname}"));
            }
        } else {
            new_lines.push(line.to_string());
        }
    }
    if !found_127_0_0_1 {
        new_lines.insert(0, format!("127.0.0.1 localhost {hostname}"));
    }
    if !found_127_0_1_1 && fqdn != hostname {
        let pos = new_lines
            .iter()
            .position(|l| l.starts_with("127.0.0.1"))
            .map(|p| p + 1)
            .unwrap_or(0);
        new_lines.insert(pos, format!("127.0.1.1 {fqdn} {hostname}"));
    }
    new_lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_hosts_empty_existing() {
        let result = build_hosts_content("", "myhost", "myhost.example.com");
        assert!(result.contains("127.0.0.1 localhost myhost"));
        assert!(result.contains("127.0.1.1 myhost.example.com myhost"));
    }

    #[test]
    fn test_build_hosts_existing_127001() {
        let existing = "127.0.0.1 localhost oldhost\n";
        let result = build_hosts_content(existing, "newhost", "newhost.example.com");
        assert!(result.contains("127.0.0.1 localhost newhost"));
        assert!(!result.contains("oldhost"));
        assert!(result.contains("127.0.1.1 newhost.example.com newhost"));
    }

    #[test]
    fn test_build_hosts_existing_both_entries() {
        let existing = "127.0.0.1 localhost\n127.0.1.1 old.example.com old\n";
        let result = build_hosts_content(existing, "new", "new.example.com");
        assert!(result.contains("127.0.0.1 localhost new"));
        assert!(result.contains("127.0.1.1 new.example.com new"));
        assert!(!result.contains("old"));
    }

    #[test]
    fn test_build_hosts_fqdn_same_as_hostname() {
        let existing = "127.0.0.1 localhost\n";
        let result = build_hosts_content(existing, "simple", "simple");
        assert!(result.contains("127.0.0.1 localhost simple"));
        assert!(!result.contains("127.0.1.1"));
    }

    #[test]
    fn test_build_hosts_fqdn_same_with_existing_127011() {
        let existing = "127.0.0.1 localhost\n127.0.1.1 old.example.com old\n";
        let result = build_hosts_content(existing, "simple", "simple");
        assert!(result.contains("127.0.1.1 simple"));
        assert!(!result.contains("old"));
    }

    #[test]
    fn test_build_hosts_preserves_comments() {
        let existing = "# This is a comment\n127.0.0.1 localhost\n";
        let result = build_hosts_content(existing, "host", "host.example.com");
        assert!(result.contains("# This is a comment"));
    }

    #[test]
    fn test_build_hosts_preserves_other_entries() {
        let existing = "127.0.0.1 localhost\n192.168.1.1 gateway\n";
        let result = build_hosts_content(existing, "host", "host.example.com");
        assert!(result.contains("192.168.1.1 gateway"));
    }

    #[test]
    fn test_build_hosts_inserts_127011_after_127001() {
        let result = build_hosts_content("", "host", "host.example.com");
        let lines: Vec<&str> = result.lines().collect();
        assert!(lines[0].starts_with("127.0.0.1"));
        assert!(lines[1].starts_with("127.0.1.1"));
    }

    #[tokio::test]
    async fn test_set_hostname_fqdn_without_manage_hosts() {
        let _ = set_hostname_fqdn("test-fqdn-host", Some("test-fqdn-host.local"), false).await;
    }
}
