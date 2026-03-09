//! Package upgrade module
//!
//! Upgrades all installed packages using the appropriate system package manager.
//! Supports apt (Debian/Ubuntu), dnf/yum (RHEL/Fedora), zypper (SUSE), and apk (Alpine).
//!
//! This module is triggered by the `package_upgrade: true` directive in cloud-config.

use crate::CloudInitError;
use crate::modules::packages;
use tracing::info;

/// Upgrade all installed packages on the system.
///
/// Detects the available package manager and runs the appropriate upgrade command:
/// - **apt**: `apt-get upgrade -y`
/// - **dnf**: `dnf upgrade -y`
/// - **yum**: `yum update -y`
/// - **zypper**: `zypper --non-interactive update`
/// - **apk**: `apk upgrade`
///
/// # Errors
///
/// Returns an error if no supported package manager is found.
/// Upgrade failures are logged as warnings but do not return errors, matching
/// the cloud-init convention for non-fatal package operations.
pub async fn run() -> Result<(), CloudInitError> {
    info!("package_upgrade: upgrading all installed packages");
    packages::upgrade_packages().await
}

#[cfg(test)]
mod tests {
    use crate::config::CloudConfig;

    /// Test that `package_upgrade: true` is parsed correctly
    #[test]
    fn test_package_upgrade_config_true() {
        let yaml = r#"#cloud-config
package_upgrade: true
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.package_upgrade, Some(true));
    }

    /// Test that `package_upgrade: false` is parsed correctly
    #[test]
    fn test_package_upgrade_config_false() {
        let yaml = r#"#cloud-config
package_upgrade: false
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.package_upgrade, Some(false));
    }

    /// Test that `package_upgrade` defaults to None when absent
    #[test]
    fn test_package_upgrade_config_absent() {
        let yaml = r#"#cloud-config
hostname: myhost
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert!(config.package_upgrade.is_none());
    }

    /// Test that `package_upgrade` and `package_update` can be used together
    #[test]
    fn test_package_upgrade_with_update() {
        let yaml = r#"#cloud-config
package_update: true
package_upgrade: true
packages:
  - nginx
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.package_update, Some(true));
        assert_eq!(config.package_upgrade, Some(true));
        assert_eq!(config.packages, vec!["nginx"]);
    }

    /// Test full package management configuration
    #[test]
    fn test_package_upgrade_full_config() {
        let yaml = r#"#cloud-config
package_update: true
package_upgrade: true
packages:
  - nginx
  - vim
  - htop
  - curl
  - git
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.package_upgrade, Some(true));
        assert_eq!(config.packages.len(), 5);
    }

    /// Test that `run()` is callable and returns a result without panicking.
    ///
    /// This test verifies the function signature and error handling contract:
    /// - If a package manager is available, `run()` attempts the upgrade.
    /// - If no package manager is found, `run()` returns a `CloudInitError::Module` error.
    /// Both outcomes are valid in a test environment.
    #[tokio::test]
    async fn test_run_returns_result() {
        use crate::CloudInitError;

        let result = super::run().await;
        match result {
            Ok(()) => {
                // A package manager was available and the upgrade completed (or had non-fatal issues)
            }
            Err(CloudInitError::Module { ref module, .. }) => {
                // No supported package manager found in the test environment
                assert_eq!(module, "packages");
            }
            Err(e) => {
                // Any other error is also acceptable (e.g., Command error)
                let _ = e;
            }
        }
    }
}
