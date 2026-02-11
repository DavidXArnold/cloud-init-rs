//! Config stage - applies user configuration
//!
//! Responsibilities:
//! - Create users and groups
//! - Install packages
//! - Write files (write_files directive)
//! - Configure services

use crate::CloudInitError;
use crate::config::CloudConfig;
use crate::modules::{groups, hostname, locale, packages, timezone, users, write_files};
use crate::state::InstanceState;
use tokio::fs;
use tracing::{debug, info, warn};

/// Run the config stage
pub async fn run() -> Result<(), CloudInitError> {
    info!("Config stage: applying user configuration");

    // Load cloud-config from instance state
    let config = load_cloud_config().await?;

    // Apply configuration modules in order
    // 1. System configuration (hostname, timezone, locale)
    apply_system_config(&config).await?;

    // 2. Groups (before users, so users can be added to groups)
    apply_groups(&config).await?;

    // 3. Users
    apply_users(&config).await?;

    // 4. Write files (non-deferred)
    apply_write_files(&config, false).await?;

    // 5. Package management
    apply_packages(&config).await?;

    // 6. Write files (deferred - after packages installed)
    apply_write_files(&config, true).await?;

    info!("Config stage: completed");
    Ok(())
}

/// Load cloud-config from instance state directory
async fn load_cloud_config() -> Result<CloudConfig, CloudInitError> {
    debug!("Loading cloud-config");

    let mut state = InstanceState::new();

    // Try to load cached instance ID
    if let Some(instance_id) = state.load_cached_instance_id().await? {
        debug!("Found cached instance ID: {}", instance_id);

        // Try to read cloud-config from instance directory
        let paths = state.paths();
        let config_path = paths.cloud_config(&instance_id);

        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            return CloudConfig::from_yaml(&content).map_err(|e| {
                CloudInitError::InvalidData(format!("Failed to parse cloud-config: {}", e))
            });
        }

        // Try user-data as fallback
        let userdata_path = paths.user_data(&instance_id);
        if userdata_path.exists() {
            let content = fs::read_to_string(&userdata_path).await?;
            if CloudConfig::is_cloud_config(&content) {
                return CloudConfig::from_yaml(&content).map_err(|e| {
                    CloudInitError::InvalidData(format!("Failed to parse user-data: {}", e))
                });
            }
        }
    }

    // Return empty config if nothing found
    debug!("No cloud-config found, using defaults");
    Ok(CloudConfig::default())
}

/// Apply system configuration (hostname, timezone, locale)
async fn apply_system_config(config: &CloudConfig) -> Result<(), CloudInitError> {
    // Set hostname
    if let Some(ref name) = config.hostname {
        debug!("Setting hostname to: {}", name);
        let manage_hosts = config.manage_etc_hosts.unwrap_or(false);
        if let Err(e) =
            hostname::set_hostname_fqdn(name, config.fqdn.as_deref(), manage_hosts).await
        {
            warn!("Failed to set hostname: {}", e);
        }
    }

    // Set timezone
    if let Some(ref tz) = config.timezone {
        debug!("Setting timezone to: {}", tz);
        if let Err(e) = timezone::set_timezone(tz).await {
            warn!("Failed to set timezone: {}", e);
        }
    }

    // Set locale
    if let Some(ref loc) = config.locale {
        debug!("Setting locale to: {}", loc);
        if let Err(e) = locale::set_locale(loc).await {
            warn!("Failed to set locale: {}", e);
        }
    }

    Ok(())
}

/// Apply group configuration
async fn apply_groups(config: &CloudConfig) -> Result<(), CloudInitError> {
    if config.groups.is_empty() {
        return Ok(());
    }

    debug!("Creating {} groups", config.groups.len());

    if let Err(e) = groups::create_groups(&config.groups).await {
        warn!("Failed to create groups: {}", e);
    }

    Ok(())
}

/// Apply user configuration
async fn apply_users(config: &CloudConfig) -> Result<(), CloudInitError> {
    if config.users.is_empty() {
        return Ok(());
    }

    debug!("Creating {} users", config.users.len());

    if let Err(e) = users::create_users(&config.users).await {
        warn!("Failed to create users: {}", e);
    }

    Ok(())
}

/// Apply write_files configuration
async fn apply_write_files(config: &CloudConfig, deferred: bool) -> Result<(), CloudInitError> {
    let files: Vec<_> = config
        .write_files
        .iter()
        .filter(|f| f.defer.unwrap_or(false) == deferred)
        .collect();

    if files.is_empty() {
        return Ok(());
    }

    debug!(
        "Writing {} {} files",
        files.len(),
        if deferred { "deferred" } else { "immediate" }
    );

    for file_config in files {
        if let Err(e) = write_files::write_file(file_config).await {
            warn!("Failed to write file {}: {}", file_config.path, e);
        }
    }

    Ok(())
}

/// Apply package configuration
async fn apply_packages(config: &CloudConfig) -> Result<(), CloudInitError> {
    // Update package cache if requested
    if config.package_update == Some(true) {
        info!("Updating package cache");
        if let Err(e) = packages::update_package_cache().await {
            warn!("Failed to update package cache: {}", e);
            // Continue anyway - package install might still work
        }
    }

    // Upgrade packages if requested
    if config.package_upgrade == Some(true) {
        info!("Upgrading packages");
        if let Err(e) = packages::upgrade_packages().await {
            warn!("Failed to upgrade packages: {}", e);
        }
    }

    // Install packages
    if !config.packages.is_empty() {
        info!("Installing {} packages", config.packages.len());
        packages::install_packages(&config.packages).await?;
    }

    Ok(())
}
