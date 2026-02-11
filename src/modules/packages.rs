//! Package management module
//!
//! Installs packages using the appropriate package manager (apt, yum, dnf, zypper).

use crate::CloudInitError;
use tracing::{debug, info, warn};

/// Detected package manager
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Apt,
    Dnf,
    Yum,
    Zypper,
    Apk,
}

impl PackageManager {
    /// Detect the system's package manager
    pub async fn detect() -> Option<Self> {
        // Check in order of preference
        if command_exists("apt-get").await {
            return Some(Self::Apt);
        }
        if command_exists("dnf").await {
            return Some(Self::Dnf);
        }
        if command_exists("yum").await {
            return Some(Self::Yum);
        }
        if command_exists("zypper").await {
            return Some(Self::Zypper);
        }
        if command_exists("apk").await {
            return Some(Self::Apk);
        }
        None
    }

    fn install_command(&self) -> (&str, Vec<&str>) {
        match self {
            Self::Apt => ("apt-get", vec!["install", "-y"]),
            Self::Dnf => ("dnf", vec!["install", "-y"]),
            Self::Yum => ("yum", vec!["install", "-y"]),
            Self::Zypper => ("zypper", vec!["--non-interactive", "install"]),
            Self::Apk => ("apk", vec!["add", "--no-cache"]),
        }
    }

    fn update_command(&self) -> (&str, Vec<&str>) {
        match self {
            Self::Apt => ("apt-get", vec!["update"]),
            Self::Dnf => ("dnf", vec!["check-update"]),
            Self::Yum => ("yum", vec!["check-update"]),
            Self::Zypper => ("zypper", vec!["--non-interactive", "refresh"]),
            Self::Apk => ("apk", vec!["update"]),
        }
    }

    fn upgrade_command(&self) -> (&str, Vec<&str>) {
        match self {
            Self::Apt => ("apt-get", vec!["upgrade", "-y"]),
            Self::Dnf => ("dnf", vec!["upgrade", "-y"]),
            Self::Yum => ("yum", vec!["update", "-y"]),
            Self::Zypper => ("zypper", vec!["--non-interactive", "update"]),
            Self::Apk => ("apk", vec!["upgrade"]),
        }
    }
}

/// Check if a command exists
async fn command_exists(cmd: &str) -> bool {
    tokio::process::Command::new("which")
        .arg(cmd)
        .output()
        .await
        .is_ok_and(|o| o.status.success())
}

/// Update package cache
pub async fn update_package_cache() -> Result<(), CloudInitError> {
    let pm = PackageManager::detect()
        .await
        .ok_or_else(|| CloudInitError::Module {
            module: "packages".to_string(),
            message: "No supported package manager found".to_string(),
        })?;

    info!("Updating package cache using {:?}", pm);

    let (cmd, args) = pm.update_command();
    let output = tokio::process::Command::new(cmd)
        .args(&args)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    // Note: yum/dnf check-update returns 100 if updates available, which is not an error
    if !output.status.success() && output.status.code() != Some(100) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("Package cache update had issues: {}", stderr);
        // Don't fail - update issues are often non-fatal
    }

    Ok(())
}

/// Upgrade all packages
pub async fn upgrade_packages() -> Result<(), CloudInitError> {
    let pm = PackageManager::detect()
        .await
        .ok_or_else(|| CloudInitError::Module {
            module: "packages".to_string(),
            message: "No supported package manager found".to_string(),
        })?;

    info!("Upgrading packages using {:?}", pm);

    let (cmd, args) = pm.upgrade_command();
    let output = tokio::process::Command::new(cmd)
        .args(&args)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("Package upgrade had issues: {}", stderr);
    }

    Ok(())
}

/// Install packages
pub async fn install_packages(packages: &[String]) -> Result<(), CloudInitError> {
    if packages.is_empty() {
        return Ok(());
    }

    let pm = PackageManager::detect()
        .await
        .ok_or_else(|| CloudInitError::Module {
            module: "packages".to_string(),
            message: "No supported package manager found".to_string(),
        })?;

    info!("Installing {} packages using {:?}", packages.len(), pm);
    debug!("Packages: {:?}", packages);

    let (cmd, base_args) = pm.install_command();
    let mut args: Vec<&str> = base_args;
    for pkg in packages {
        args.push(pkg.as_str());
    }

    let output = tokio::process::Command::new(cmd)
        .args(&args)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::Module {
            module: "packages".to_string(),
            message: format!("Failed to install packages: {}", stderr),
        });
    }

    info!("Successfully installed {} packages", packages.len());
    Ok(())
}

/// Install a single package
pub async fn install_package(package: &str) -> Result<(), CloudInitError> {
    install_packages(&[package.to_string()]).await
}
