//! Package cache update module
//!
//! Updates the package manager cache using the appropriate package manager:
//! - apt:    `apt-get update`
//! - dnf:    `dnf makecache`
//! - yum:    `yum makecache`
//! - zypper: `zypper --non-interactive refresh`
//! - apk:    `apk update`

use crate::CloudInitError;
use crate::modules::packages::{self, PackageManager};
use crate::modules::{Frequency, Module};
use tracing::info;

/// Package cache update module
pub struct PackageUpdate;

impl Module for PackageUpdate {
    fn name(&self) -> &'static str {
        "package_update"
    }

    fn frequency(&self) -> Frequency {
        Frequency::PerInstance
    }
}

/// Run the package_update module
///
/// Refreshes the local package manager cache so that subsequent package
/// install or upgrade operations use up-to-date metadata.
pub async fn run() -> Result<(), CloudInitError> {
    info!("Running package_update module: refreshing package cache");
    packages::update_package_cache().await
}

/// Return the cache-refresh command for the given package manager.
///
/// Exposed for testing and introspection without executing real commands.
pub fn cache_refresh_command(pm: PackageManager) -> (&'static str, Vec<&'static str>) {
    pm.update_command()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::packages::PackageManager;

    #[test]
    fn test_module_name() {
        let m = PackageUpdate;
        assert_eq!(m.name(), "package_update");
    }

    #[test]
    fn test_module_frequency() {
        let m = PackageUpdate;
        assert!(matches!(m.frequency(), Frequency::PerInstance));
    }

    #[test]
    fn test_apt_update_command() {
        let (cmd, args) = cache_refresh_command(PackageManager::Apt);
        assert_eq!(cmd, "apt-get");
        assert_eq!(args, vec!["update"]);
    }

    #[test]
    fn test_dnf_makecache_command() {
        let (cmd, args) = cache_refresh_command(PackageManager::Dnf);
        assert_eq!(cmd, "dnf");
        assert_eq!(args, vec!["makecache"]);
    }

    #[test]
    fn test_yum_makecache_command() {
        let (cmd, args) = cache_refresh_command(PackageManager::Yum);
        assert_eq!(cmd, "yum");
        assert_eq!(args, vec!["makecache"]);
    }

    #[test]
    fn test_zypper_refresh_command() {
        let (cmd, args) = cache_refresh_command(PackageManager::Zypper);
        assert_eq!(cmd, "zypper");
        assert_eq!(args, vec!["--non-interactive", "refresh"]);
    }

    #[test]
    fn test_apk_update_command() {
        let (cmd, args) = cache_refresh_command(PackageManager::Apk);
        assert_eq!(cmd, "apk");
        assert_eq!(args, vec!["update"]);
    }
}
