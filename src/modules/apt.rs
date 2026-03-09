//! APT-specific configuration module
//!
//! Handles Debian/Ubuntu APT configuration:
//!
//! - Proxy settings written to `/etc/apt/apt.conf.d/90-cloud-init-proxy`
//! - Additional `apt.conf` snippets written to `/etc/apt/apt.conf.d/90-cloud-init-apt`
//! - Custom repository sources written to `/etc/apt/sources.list.d/`
//! - GPG key import (inline PEM/armored key or key-server fetch)
//! - Package pinning written to `/etc/apt/preferences.d/90-cloud-init`
//! - Primary archive sources written to `/etc/apt/sources.list.d/cloud-init-primary.list`

use crate::config::{AptConfig, AptPreference, AptPrimarySource, AptSource};
use crate::CloudInitError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Apply the full APT configuration described in `config`.
pub async fn configure_apt(config: &AptConfig) -> Result<(), CloudInitError> {
    info!("Configuring APT");

    configure_proxy(config).await?;
    configure_conf(config.conf.as_deref()).await?;
    configure_sources(&config.sources).await?;
    configure_preferences(&config.preferences).await?;
    configure_primary_sources(&config.primary).await?;

    info!("APT configuration complete");
    Ok(())
}

// ---------------------------------------------------------------------------
// Proxy
// ---------------------------------------------------------------------------

/// Write proxy settings to `/etc/apt/apt.conf.d/90-cloud-init-proxy`.
///
/// If no proxy is configured the file is removed (if it exists) so that a
/// previous run does not leave stale settings behind.
pub async fn configure_proxy(config: &AptConfig) -> Result<(), CloudInitError> {
    let proxy_file = Path::new("/etc/apt/apt.conf.d/90-cloud-init-proxy");

    // Resolve the effective HTTP proxy: `proxy` is an alias for `http_proxy`.
    let http = config
        .proxy
        .as_deref()
        .or(config.http_proxy.as_deref());
    let https = config.https_proxy.as_deref();
    let ftp = config.ftp_proxy.as_deref();

    if http.is_none() && https.is_none() && ftp.is_none() {
        // Nothing to configure; remove stale file if present.
        if proxy_file.exists() {
            fs::remove_file(proxy_file).await.map_err(CloudInitError::Io)?;
            debug!("Removed stale APT proxy file");
        }
        return Ok(());
    }

    let mut content = String::from("// Configured by cloud-init-rs\n");
    if let Some(url) = http {
        content.push_str(&format!("Acquire::http::Proxy \"{}\";\n", url));
    }
    if let Some(url) = https {
        content.push_str(&format!("Acquire::https::Proxy \"{}\";\n", url));
    }
    if let Some(url) = ftp {
        content.push_str(&format!("Acquire::ftp::Proxy \"{}\";\n", url));
    }

    ensure_dir(proxy_file).await?;
    fs::write(proxy_file, &content)
        .await
        .map_err(CloudInitError::Io)?;
    info!("Wrote APT proxy configuration to {:?}", proxy_file);
    Ok(())
}

// ---------------------------------------------------------------------------
// Custom apt.conf snippet
// ---------------------------------------------------------------------------

/// Write an arbitrary `apt.conf` snippet to
/// `/etc/apt/apt.conf.d/90-cloud-init-apt`.
pub async fn configure_conf(conf: Option<&str>) -> Result<(), CloudInitError> {
    let conf_file = Path::new("/etc/apt/apt.conf.d/90-cloud-init-apt");

    let Some(snippet) = conf else {
        if conf_file.exists() {
            fs::remove_file(conf_file)
                .await
                .map_err(CloudInitError::Io)?;
            debug!("Removed stale APT conf file");
        }
        return Ok(());
    };

    let content = format!("// Configured by cloud-init-rs\n{}", snippet);
    ensure_dir(conf_file).await?;
    fs::write(conf_file, &content)
        .await
        .map_err(CloudInitError::Io)?;
    info!("Wrote APT conf to {:?}", conf_file);
    Ok(())
}

// ---------------------------------------------------------------------------
// Sources
// ---------------------------------------------------------------------------

/// Write named APT sources to `/etc/apt/sources.list.d/`.
///
/// Each entry in `sources` maps a logical name to an [`AptSource`].  The
/// resulting file is named `<logical-name>.list` unless `AptSource::filename`
/// overrides it.
pub async fn configure_sources(
    sources: &HashMap<String, AptSource>,
) -> Result<(), CloudInitError> {
    if sources.is_empty() {
        return Ok(());
    }

    for (name, source) in sources {
        configure_source(name, source).await?;
    }
    Ok(())
}

/// Configure a single APT source entry.
async fn configure_source(name: &str, source: &AptSource) -> Result<(), CloudInitError> {
    // Determine filename
    let stem = source
        .filename
        .as_deref()
        .unwrap_or(name)
        .trim_end_matches(".list");
    let list_path = PathBuf::from(format!("/etc/apt/sources.list.d/{}.list", stem));

    // Write source line if provided
    if let Some(source_line) = &source.source {
        let content = format!("# Configured by cloud-init-rs\n{}\n", source_line);
        ensure_dir(&list_path).await?;
        fs::write(&list_path, &content)
            .await
            .map_err(CloudInitError::Io)?;
        info!("Wrote APT source {:?}", list_path);
    }

    // Import GPG key
    if let Some(key_content) = &source.key {
        import_gpg_key_content(name, key_content).await?;
    } else if let Some(keyid) = &source.keyid {
        let keyserver = source
            .keyserver
            .as_deref()
            .unwrap_or("keyserver.ubuntu.com");
        fetch_gpg_key(name, keyid, keyserver).await?;
    }

    Ok(())
}

/// Import an ASCII-armored (or binary) GPG key supplied as a string.
///
/// The key is dearmored with `gpg --dearmor` and written to
/// `/etc/apt/trusted.gpg.d/<name>.gpg`.
async fn import_gpg_key_content(name: &str, key_content: &str) -> Result<(), CloudInitError> {
    use tokio::io::AsyncWriteExt;

    let key_path = PathBuf::from(format!("/etc/apt/trusted.gpg.d/{}.gpg", name));
    ensure_dir(&key_path).await?;

    // Pipe the key through `gpg --dearmor` and capture its stdout.
    let mut child = tokio::process::Command::new("gpg")
        .args(["--batch", "--no-default-keyring", "--dearmor"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| CloudInitError::Command(format!("Failed to spawn gpg: {}", e)))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(key_content.as_bytes())
            .await
            .map_err(|e| CloudInitError::Command(format!("Failed to write to gpg stdin: {}", e)))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| CloudInitError::Command(format!("gpg wait failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::Module {
            module: "apt".to_string(),
            message: format!("gpg --dearmor failed for key '{}': {}", name, stderr),
        });
    }

    fs::write(&key_path, &output.stdout)
        .await
        .map_err(CloudInitError::Io)?;
    info!("Imported GPG key for '{}' to {:?}", name, key_path);
    Ok(())
}

/// Fetch a GPG key by ID from a key server and add it to the APT trusted keys.
async fn fetch_gpg_key(name: &str, keyid: &str, keyserver: &str) -> Result<(), CloudInitError> {
    let key_path = PathBuf::from(format!("/etc/apt/trusted.gpg.d/{}.gpg", name));
    ensure_dir(&key_path).await?;

    debug!(
        "Fetching GPG key {} from {} for '{}'",
        keyid, keyserver, name
    );

    let output = tokio::process::Command::new("gpg")
        .args([
            "--batch",
            "--no-default-keyring",
            "--keyring",
            key_path.to_str().unwrap_or_default(),
            "--keyserver",
            keyserver,
            "--recv-keys",
            keyid,
        ])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(format!("Failed to run gpg: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "gpg --recv-keys failed for key '{}' from {}: {}",
            keyid, keyserver, stderr
        );
        // Non-fatal: the source may still work if the key is already trusted.
    } else {
        info!(
            "Fetched GPG key {} from {} for '{}'",
            keyid, keyserver, name
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Preferences (pinning)
// ---------------------------------------------------------------------------

/// Write APT package-pinning preferences to
/// `/etc/apt/preferences.d/90-cloud-init`.
pub async fn configure_preferences(
    preferences: &[AptPreference],
) -> Result<(), CloudInitError> {
    let pref_file = Path::new("/etc/apt/preferences.d/90-cloud-init");

    if preferences.is_empty() {
        if pref_file.exists() {
            fs::remove_file(pref_file)
                .await
                .map_err(CloudInitError::Io)?;
            debug!("Removed stale APT preferences file");
        }
        return Ok(());
    }

    let mut content = String::from("# Configured by cloud-init-rs\n");
    for pref in preferences {
        content.push_str(&format!(
            "\nPackage: {}\nPin: {}\nPin-Priority: {}\n",
            pref.package, pref.pin, pref.pin_priority
        ));
    }

    ensure_dir(pref_file).await?;
    fs::write(pref_file, &content)
        .await
        .map_err(CloudInitError::Io)?;
    info!("Wrote APT preferences to {:?}", pref_file);
    Ok(())
}

// ---------------------------------------------------------------------------
// Primary sources
// ---------------------------------------------------------------------------

/// Write primary archive source overrides to
/// `/etc/apt/sources.list.d/cloud-init-primary.list`.
///
/// Only entries where `arches` contains `"default"` or matches the current
/// architecture are written.  If no primary sources are configured the file
/// is left untouched.
pub async fn configure_primary_sources(
    primary: &[AptPrimarySource],
) -> Result<(), CloudInitError> {
    if primary.is_empty() {
        return Ok(());
    }

    // Detect the host architecture once.
    let arch = detect_arch().await;

    let mut lines: Vec<String> = vec!["# Configured by cloud-init-rs".to_string()];

    for entry in primary {
        let matches = entry.arches.is_empty()
            || entry.arches.iter().any(|a| a == "default" || a == &arch);

        if !matches {
            continue;
        }

        let Some(uri) = &entry.uri else {
            continue;
        };

        // Resolve codename: use the override if provided, otherwise try to
        // detect from the running system.
        let codename = match &entry.codename {
            Some(c) => c.clone(),
            None => detect_codename().await,
        };

        if codename.is_empty() {
            warn!("Could not determine distribution codename for primary source; skipping");
            continue;
        }

        lines.push(format!(
            "deb {} {} main restricted universe multiverse",
            uri, codename
        ));
        lines.push(format!(
            "deb {} {}-updates main restricted universe multiverse",
            uri, codename
        ));
        lines.push(format!(
            "deb {} {}-security main restricted universe multiverse",
            uri, codename
        ));
    }

    if lines.len() == 1 {
        // Only the comment line was added; nothing to write.
        return Ok(());
    }

    let list_path = Path::new("/etc/apt/sources.list.d/cloud-init-primary.list");
    ensure_dir(list_path).await?;
    let content = lines.join("\n") + "\n";
    fs::write(list_path, &content)
        .await
        .map_err(CloudInitError::Io)?;
    info!("Wrote primary APT sources to {:?}", list_path);
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Ensure the parent directory of `path` exists.
async fn ensure_dir(path: &Path) -> Result<(), CloudInitError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(CloudInitError::Io)?;
    }
    Ok(())
}

/// Detect the host CPU architecture (returns e.g. "amd64", "arm64").
async fn detect_arch() -> String {
    match tokio::process::Command::new("dpkg")
        .arg("--print-architecture")
        .output()
        .await
    {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
        _ => {
            // Fallback: use uname -m and map to Debian arch names.
            match tokio::process::Command::new("uname")
                .arg("-m")
                .output()
                .await
            {
                Ok(o) if o.status.success() => {
                    let m = String::from_utf8_lossy(&o.stdout);
                    match m.trim() {
                        "x86_64" => "amd64".to_string(),
                        "aarch64" => "arm64".to_string(),
                        other => other.to_string(),
                    }
                }
                _ => String::new(),
            }
        }
    }
}

/// Detect the distribution codename (returns e.g. "focal", "jammy").
async fn detect_codename() -> String {
    // Try lsb_release first.
    if let Ok(o) = tokio::process::Command::new("lsb_release")
        .args(["-cs"])
        .output()
        .await
        && o.status.success()
    {
        let name = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }

    // Fallback: parse /etc/os-release.
    if let Ok(content) = tokio::fs::read_to_string("/etc/os-release").await {
        for line in content.lines() {
            if let Some(val) = line.strip_prefix("VERSION_CODENAME=") {
                return val.trim_matches('"').to_string();
            }
        }
    }

    String::new()
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::config::{AptConfig, AptPreference, AptSource};

    // ------------------------------------------------------------------
    // build_proxy_content – pure function extracted for testing
    // ------------------------------------------------------------------

    fn build_proxy_content(config: &AptConfig) -> String {
        let http = config.proxy.as_deref().or(config.http_proxy.as_deref());
        let https = config.https_proxy.as_deref();
        let ftp = config.ftp_proxy.as_deref();

        let mut content = String::from("// Configured by cloud-init-rs\n");
        if let Some(url) = http {
            content.push_str(&format!("Acquire::http::Proxy \"{}\";\n", url));
        }
        if let Some(url) = https {
            content.push_str(&format!("Acquire::https::Proxy \"{}\";\n", url));
        }
        if let Some(url) = ftp {
            content.push_str(&format!("Acquire::ftp::Proxy \"{}\";\n", url));
        }
        content
    }

    #[test]
    fn test_proxy_content_http() {
        let config = AptConfig {
            proxy: Some("http://proxy.example.com:3128".to_string()),
            ..Default::default()
        };
        let content = build_proxy_content(&config);
        assert!(content.contains("Acquire::http::Proxy"));
        assert!(content.contains("http://proxy.example.com:3128"));
        assert!(!content.contains("https::Proxy"));
        assert!(!content.contains("ftp::Proxy"));
    }

    #[test]
    fn test_proxy_content_http_proxy_alias() {
        // `http_proxy` should behave identically to `proxy`
        let config = AptConfig {
            http_proxy: Some("http://mirror.local:3128".to_string()),
            ..Default::default()
        };
        let content = build_proxy_content(&config);
        assert!(content.contains("Acquire::http::Proxy"));
        assert!(content.contains("http://mirror.local:3128"));
    }

    #[test]
    fn test_proxy_content_proxy_takes_precedence_over_http_proxy() {
        // When both `proxy` and `http_proxy` are set, `proxy` wins.
        let config = AptConfig {
            proxy: Some("http://proxy.example.com:3128".to_string()),
            http_proxy: Some("http://other.example.com:3128".to_string()),
            ..Default::default()
        };
        let content = build_proxy_content(&config);
        assert!(content.contains("http://proxy.example.com:3128"));
        assert!(!content.contains("http://other.example.com:3128"));
    }

    #[test]
    fn test_proxy_content_all_protocols() {
        let config = AptConfig {
            proxy: Some("http://proxy.example.com:3128".to_string()),
            https_proxy: Some("https://proxy.example.com:3128".to_string()),
            ftp_proxy: Some("ftp://proxy.example.com:21".to_string()),
            ..Default::default()
        };
        let content = build_proxy_content(&config);
        assert!(content.contains("Acquire::http::Proxy"));
        assert!(content.contains("Acquire::https::Proxy"));
        assert!(content.contains("Acquire::ftp::Proxy"));
    }

    #[test]
    fn test_proxy_content_empty() {
        let config = AptConfig::default();
        let content = build_proxy_content(&config);
        // Only the header line should be present.
        assert_eq!(
            content.trim(),
            "// Configured by cloud-init-rs"
        );
    }

    // ------------------------------------------------------------------
    // build_preferences_content – pure function extracted for testing
    // ------------------------------------------------------------------

    fn build_preferences_content(preferences: &[AptPreference]) -> String {
        let mut content = String::from("# Configured by cloud-init-rs\n");
        for pref in preferences {
            content.push_str(&format!(
                "\nPackage: {}\nPin: {}\nPin-Priority: {}\n",
                pref.package, pref.pin, pref.pin_priority
            ));
        }
        content
    }

    #[test]
    fn test_preferences_single_entry() {
        let prefs = vec![AptPreference {
            package: "nginx".to_string(),
            pin: "origin nginx.org".to_string(),
            pin_priority: 900,
        }];
        let content = build_preferences_content(&prefs);
        assert!(content.contains("Package: nginx"));
        assert!(content.contains("Pin: origin nginx.org"));
        assert!(content.contains("Pin-Priority: 900"));
    }

    #[test]
    fn test_preferences_multiple_entries() {
        let prefs = vec![
            AptPreference {
                package: "nginx".to_string(),
                pin: "origin nginx.org".to_string(),
                pin_priority: 900,
            },
            AptPreference {
                package: "*".to_string(),
                pin: "release a=stable".to_string(),
                pin_priority: 100,
            },
        ];
        let content = build_preferences_content(&prefs);
        assert!(content.contains("Package: nginx"));
        assert!(content.contains("Package: *"));
        assert!(content.contains("Pin-Priority: 900"));
        assert!(content.contains("Pin-Priority: 100"));
    }

    #[test]
    fn test_preferences_empty() {
        let content = build_preferences_content(&[]);
        assert_eq!(content.trim(), "# Configured by cloud-init-rs");
    }

    // ------------------------------------------------------------------
    // Source filename resolution
    // ------------------------------------------------------------------

    #[test]
    fn test_source_filename_default() {
        let source = AptSource::default();
        let stem = source
            .filename
            .as_deref()
            .unwrap_or("my-ppa")
            .trim_end_matches(".list");
        assert_eq!(stem, "my-ppa");
    }

    #[test]
    fn test_source_filename_override() {
        let source = AptSource {
            filename: Some("custom-repo".to_string()),
            ..Default::default()
        };
        let stem = source
            .filename
            .as_deref()
            .unwrap_or("fallback")
            .trim_end_matches(".list");
        assert_eq!(stem, "custom-repo");
    }

    #[test]
    fn test_source_filename_strips_list_suffix() {
        let source = AptSource {
            filename: Some("custom-repo.list".to_string()),
            ..Default::default()
        };
        let stem = source
            .filename
            .as_deref()
            .unwrap_or("fallback")
            .trim_end_matches(".list");
        assert_eq!(stem, "custom-repo");
    }

    // ------------------------------------------------------------------
    // Config parsing round-trip
    // ------------------------------------------------------------------

    #[test]
    fn test_apt_config_parse_minimal() {
        let yaml = r#"
proxy: "http://proxy.example.com:3128"
"#;
        let config: AptConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config.proxy,
            Some("http://proxy.example.com:3128".to_string())
        );
    }

    #[test]
    fn test_apt_config_parse_full() {
        let yaml = r#"
proxy: "http://proxy.example.com:3128"
https_proxy: "https://proxy.example.com:3128"
ftp_proxy: "ftp://proxy.example.com:21"
conf: |
  Acquire::ForceIPv4 "true";
sources:
  my-ppa:
    source: "deb http://ppa.launchpad.net/user/ppa/ubuntu focal main"
    keyid: "DEADBEEF"
    keyserver: "keyserver.ubuntu.com"
preferences:
  - package: "nginx"
    pin: "origin nginx.org"
    pin-priority: 900
"#;
        let config: AptConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config.proxy,
            Some("http://proxy.example.com:3128".to_string())
        );
        assert!(config.conf.as_deref().unwrap().contains("ForceIPv4"));
        assert!(config.sources.contains_key("my-ppa"));
        assert_eq!(config.preferences.len(), 1);
        assert_eq!(config.preferences[0].pin_priority, 900);
    }
}
