//! YUM/DNF repository management module
//!
//! Writes `.repo` files to `/etc/yum.repos.d/` for each repository entry
//! found under the `yum_repos` cloud-config key.
//!
//! # Cloud-config example
//!
//! ```yaml
//! yum_repos:
//!   epel:
//!     name: Extra Packages for Enterprise Linux 8
//!     baseurl: https://download.fedoraproject.org/pub/epel/8/$basearch
//!     enabled: true
//!     gpgcheck: true
//!     gpgkey: https://dl.fedoraproject.org/pub/epel/RPM-GPG-KEY-EPEL-8
//!   my-internal-repo:
//!     name: My Internal Repository
//!     baseurl: https://repo.example.com/centos/8/$basearch/
//!     enabled: true
//!     gpgcheck: false
//! ```

use crate::CloudInitError;
use crate::config::YumRepoConfig;
use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use tracing::{debug, info, warn};

/// Directory where YUM `.repo` files are stored.
const YUM_REPOS_DIR: &str = "/etc/yum.repos.d";

/// Write `.repo` files for every entry in the provided map.
///
/// The map key is used as the repo ID (i.e. the section header) and as the
/// file name: `<id>.repo`.  Invalid/empty entries are skipped with a warning.
pub async fn add_yum_repos(repos: &HashMap<String, YumRepoConfig>) -> Result<(), CloudInitError> {
    if repos.is_empty() {
        return Ok(());
    }

    info!("yum_add_repo: writing {} repo file(s)", repos.len());

    // Ensure the repos directory exists (it should on any RPM-based system,
    // but we create it defensively so tests pass on non-RHEL hosts too).
    tokio::fs::create_dir_all(YUM_REPOS_DIR)
        .await
        .map_err(|e| CloudInitError::Module {
            module: "yum_add_repo".to_string(),
            message: format!("failed to create {}: {}", YUM_REPOS_DIR, e),
        })?;

    for (id, repo_config) in repos {
        if let Err(e) = write_repo_file(id, repo_config).await {
            warn!("yum_add_repo: failed to write repo '{}': {}", id, e);
        }
    }

    Ok(())
}

/// Write a single `.repo` file for the given repo ID and configuration.
///
/// The file is written to `<YUM_REPOS_DIR>/<id>.repo`.
pub async fn write_repo_file(id: &str, config: &YumRepoConfig) -> Result<(), CloudInitError> {
    // At least one URL source must be present
    if config.baseurl.is_none() && config.mirrorlist.is_none() && config.metalink.is_none() {
        return Err(CloudInitError::Module {
            module: "yum_add_repo".to_string(),
            message: format!(
                "repo '{}' must have at least one of: baseurl, mirrorlist, metalink",
                id
            ),
        });
    }

    let content = build_repo_content(id, config);
    let path = format!("{}/{}.repo", YUM_REPOS_DIR, id);

    debug!("yum_add_repo: writing {}", path);

    tokio::fs::write(&path, content)
        .await
        .map_err(|e| CloudInitError::Module {
            module: "yum_add_repo".to_string(),
            message: format!("failed to write {}: {}", path, e),
        })?;

    info!("yum_add_repo: wrote {}", path);
    Ok(())
}

/// Build the INI-style `.repo` file content for the given repo.
pub fn build_repo_content(id: &str, config: &YumRepoConfig) -> String {
    let mut out = String::new();

    // Section header
    writeln!(out, "[{}]", id).expect("writing to String is infallible");

    // name (fall back to id if not specified)
    let name = config.name.as_deref().unwrap_or(id);
    writeln!(out, "name={}", name).expect("writing to String is infallible");

    // URL sources
    if let Some(ref baseurl) = config.baseurl {
        writeln!(out, "baseurl={}", baseurl).expect("writing to String is infallible");
    }
    if let Some(ref mirrorlist) = config.mirrorlist {
        writeln!(out, "mirrorlist={}", mirrorlist).expect("writing to String is infallible");
    }
    if let Some(ref metalink) = config.metalink {
        writeln!(out, "metalink={}", metalink).expect("writing to String is infallible");
    }

    // enabled (default true → 1)
    let enabled = config.enabled.unwrap_or(true);
    writeln!(out, "enabled={}", if enabled { 1 } else { 0 })
        .expect("writing to String is infallible");

    // gpgcheck
    if let Some(gpgcheck) = config.gpgcheck {
        writeln!(out, "gpgcheck={}", if gpgcheck { 1 } else { 0 })
            .expect("writing to String is infallible");
    }
    if let Some(ref gpgkey) = config.gpgkey {
        writeln!(out, "gpgkey={}", gpgkey).expect("writing to String is infallible");
    }

    // Optional fields
    if let Some(priority) = config.priority {
        writeln!(out, "priority={}", priority).expect("writing to String is infallible");
    }
    if let Some(ref failovermethod) = config.failovermethod {
        writeln!(out, "failovermethod={}", failovermethod)
            .expect("writing to String is infallible");
    }

    // SSL
    if let Some(sslverify) = config.sslverify {
        writeln!(out, "sslverify={}", if sslverify { 1 } else { 0 })
            .expect("writing to String is infallible");
    }
    if let Some(ref cert) = config.sslclientcert {
        writeln!(out, "sslclientcert={}", cert).expect("writing to String is infallible");
    }
    if let Some(ref key) = config.sslclientkey {
        writeln!(out, "sslclientkey={}", key).expect("writing to String is infallible");
    }
    if let Some(ref ca) = config.sslcacert {
        writeln!(out, "sslcacert={}", ca).expect("writing to String is infallible");
    }

    out
}
