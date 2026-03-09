//! YUM/DNF repository management module
//!
//! Configures YUM/DNF repositories by writing `.repo` files to
//! `/etc/yum.repos.d/`.  Each entry in the cloud-config `yum_repos` map
//! becomes a separate `<id>.repo` file in INI format.

use std::collections::HashMap;

use tokio::fs;
use tracing::{debug, info, warn};

use crate::CloudInitError;
use crate::config::YumRepoConfig;

/// Default directory for YUM repository files
const YUM_REPOS_DIR: &str = "/etc/yum.repos.d";

/// Configure YUM/DNF repositories from cloud-config `yum_repos` map
pub async fn configure_yum_repos(
    repos: &HashMap<String, YumRepoConfig>,
) -> Result<(), CloudInitError> {
    if repos.is_empty() {
        return Ok(());
    }

    info!("Configuring {} YUM/DNF repositories", repos.len());

    // Ensure the repos directory exists
    fs::create_dir_all(YUM_REPOS_DIR).await?;

    for (repo_id, repo_config) in repos {
        if let Err(e) = write_repo_file(repo_id, repo_config).await {
            warn!("Failed to configure YUM repository '{}': {}", repo_id, e);
        }
    }

    Ok(())
}

/// Write a single `.repo` file for the given repository
async fn write_repo_file(repo_id: &str, config: &YumRepoConfig) -> Result<(), CloudInitError> {
    validate_repo_id(repo_id)?;

    let path = format!("{}/{}.repo", YUM_REPOS_DIR, repo_id);
    let content = generate_repo_content(repo_id, config);

    debug!("Writing YUM repo file: {}", path);
    fs::write(&path, content)
        .await
        .map_err(CloudInitError::Io)?;
    info!("Configured YUM repository '{}'", repo_id);

    Ok(())
}

/// Validate that `repo_id` is safe to use as a file-system basename.
///
/// Allowed characters: alphanumeric, `-`, `_`, `.`  
/// Disallowed: empty string, leading `.`, `..` component (path traversal).
fn validate_repo_id(repo_id: &str) -> Result<(), CloudInitError> {
    if repo_id.is_empty() {
        return Err(CloudInitError::Module {
            module: "yum_repos".to_string(),
            message: "Repository ID must not be empty".to_string(),
        });
    }
    if repo_id.starts_with('.') {
        return Err(CloudInitError::Module {
            module: "yum_repos".to_string(),
            message: format!("Repository ID '{}' must not start with '.'", repo_id),
        });
    }
    if repo_id.contains("..") {
        return Err(CloudInitError::Module {
            module: "yum_repos".to_string(),
            message: format!(
                "Repository ID '{}' must not contain '..' (path traversal)",
                repo_id
            ),
        });
    }
    if !repo_id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(CloudInitError::Module {
            module: "yum_repos".to_string(),
            message: format!(
                "Repository ID '{}' contains invalid characters \
                 (allowed: alphanumeric, '-', '_', '.')",
                repo_id
            ),
        });
    }
    Ok(())
}

/// Strip newline and carriage-return characters from an INI value to prevent
/// format injection (e.g. injecting a new `[section]` header via a crafted value).
fn sanitize_ini_value(value: &str) -> String {
    value.replace('\n', " ").replace('\r', "")
}

/// Generate the INI-format content of a `.repo` file
///
/// Boolean fields are written as `1` / `0` as required by YUM/DNF.
/// String values are sanitized to prevent INI format injection.
pub fn generate_repo_content(repo_id: &str, config: &YumRepoConfig) -> String {
    let mut out = format!("[{}]\n", repo_id);

    macro_rules! push_str {
        ($field:expr, $key:expr) => {
            if let Some(ref v) = $field {
                out.push_str(&format!("{}={}\n", $key, sanitize_ini_value(v)));
            }
        };
    }

    macro_rules! push_bool {
        ($field:expr, $key:expr) => {
            if let Some(v) = $field {
                out.push_str(&format!("{}={}\n", $key, if v { 1 } else { 0 }));
            }
        };
    }

    push_str!(config.name, "name");
    push_str!(config.baseurl, "baseurl");
    push_str!(config.mirrorlist, "mirrorlist");
    push_str!(config.metalink, "metalink");
    push_bool!(config.enabled, "enabled");
    push_bool!(config.gpgcheck, "gpgcheck");
    push_str!(config.gpgkey, "gpgkey");
    push_bool!(config.skip_if_unavailable, "skip_if_unavailable");
    push_str!(config.failovermethod, "failovermethod");
    if let Some(p) = config.priority {
        out.push_str(&format!("priority={}\n", p));
    }
    push_bool!(config.sslverify, "sslverify");
    push_str!(config.sslcacert, "sslcacert");
    push_str!(config.sslclientcert, "sslclientcert");
    push_str!(config.sslclientkey, "sslclientkey");
    push_str!(config.exclude, "exclude");
    push_str!(config.includepkgs, "includepkgs");
    push_str!(config.repo_type, "type");

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_repo_id_valid() {
        assert!(validate_repo_id("epel").is_ok());
        assert!(validate_repo_id("my-repo").is_ok());
        assert!(validate_repo_id("my_repo").is_ok());
        assert!(validate_repo_id("centos-appstream-9").is_ok());
        assert!(validate_repo_id("repo.1").is_ok());
    }

    #[test]
    fn test_validate_repo_id_empty() {
        assert!(validate_repo_id("").is_err());
    }

    #[test]
    fn test_validate_repo_id_leading_dot() {
        assert!(validate_repo_id(".hidden").is_err());
        assert!(validate_repo_id(".").is_err());
    }

    #[test]
    fn test_validate_repo_id_double_dot() {
        assert!(validate_repo_id("..").is_err());
        assert!(validate_repo_id("a..b").is_err());
    }

    #[test]
    fn test_validate_repo_id_invalid_chars() {
        assert!(validate_repo_id("my/repo").is_err());
        assert!(validate_repo_id("my repo").is_err());
        assert!(validate_repo_id("repo\0null").is_err());
        assert!(validate_repo_id("repo!").is_err());
    }

    #[test]
    fn test_sanitize_ini_value_no_change() {
        assert_eq!(sanitize_ini_value("normal value"), "normal value");
        assert_eq!(
            sanitize_ini_value("https://example.com/"),
            "https://example.com/"
        );
    }

    #[test]
    fn test_sanitize_ini_value_strips_newlines() {
        assert_eq!(sanitize_ini_value("line1\nline2"), "line1 line2");
        // \r\n: \n -> space, then \r -> "" leaving a single space
        assert_eq!(sanitize_ini_value("line1\r\nline2"), "line1 line2");
        assert_eq!(sanitize_ini_value("a\rb"), "ab");
    }
}
