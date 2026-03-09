//! Zypper repository management module (SUSE/openSUSE)
//!
//! Adds zypper repositories defined in the `zypper.repos` cloud-config key.

use crate::CloudInitError;
use crate::config::ZypperRepoConfig;
use tracing::{debug, info, warn};

/// Add all configured zypper repositories
pub async fn add_repos(repos: &[ZypperRepoConfig]) -> Result<(), CloudInitError> {
    if repos.is_empty() {
        return Ok(());
    }

    info!("Adding {} zypper repositories", repos.len());

    for repo in repos {
        if let Err(e) = add_repo(repo).await {
            warn!("Failed to add zypper repo '{}': {}", repo.id, e);
        }
    }

    Ok(())
}

/// Add a single zypper repository
async fn add_repo(repo: &ZypperRepoConfig) -> Result<(), CloudInitError> {
    let baseurl = repo
        .baseurl
        .as_deref()
        .ok_or_else(|| CloudInitError::Module {
            module: "zypper_add_repo".to_string(),
            message: format!("Repo '{}' has no baseurl", repo.id),
        })?;

    let mut args: Vec<String> = vec!["--non-interactive".to_string(), "addrepo".to_string()];

    // Repository name
    if let Some(ref name) = repo.name {
        args.push("--name".to_string());
        args.push(name.clone());
    }

    // Enabled / disabled
    if repo.enabled == Some(false) {
        args.push("--disable".to_string());
    }

    // Autorefresh
    match repo.autorefresh {
        Some(false) => args.push("--no-refresh".to_string()),
        // enabled by default; pass --refresh only when explicitly requested
        Some(true) => args.push("--refresh".to_string()),
        None => {}
    }

    // GPG check
    match repo.gpgcheck {
        Some(true) => args.push("--gpgcheck".to_string()),
        Some(false) => args.push("--no-gpgcheck".to_string()),
        None => {}
    }

    // Priority
    if let Some(priority) = repo.priority {
        args.push("--priority".to_string());
        args.push(priority.to_string());
    }

    // Positional: URI then alias
    args.push(baseurl.to_string());
    args.push(repo.id.clone());

    debug!("Adding zypper repo '{}' from {}", repo.id, baseurl);

    let output = tokio::process::Command::new("zypper")
        .args(&args)
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::Module {
            module: "zypper_add_repo".to_string(),
            message: format!("Failed to add repo '{}': {}", repo.id, stderr),
        });
    }

    // Import GPG key if specified
    if let Some(ref gpgkey) = repo.gpgkey {
        import_gpg_key(&repo.id, gpgkey).await?;
    }

    info!("Added zypper repo '{}'", repo.id);
    Ok(())
}

/// Import a GPG key for a repository
async fn import_gpg_key(repo_id: &str, gpgkey_url: &str) -> Result<(), CloudInitError> {
    debug!(
        "Importing GPG key for repo '{}' from {}",
        repo_id, gpgkey_url
    );

    let output = tokio::process::Command::new("rpm")
        .args(["--import", gpgkey_url])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "Failed to import GPG key for repo '{}' from {}: {}",
            repo_id, gpgkey_url, stderr
        );
    }

    Ok(())
}
