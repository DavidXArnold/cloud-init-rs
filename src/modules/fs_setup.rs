//! Filesystem setup module
//!
//! Creates filesystems on devices as specified in the `fs_setup` cloud-config
//! directive. Supports ext4, xfs, btrfs, and swap with optional label, overwrite
//! protection, and partition specification.

use crate::CloudInitError;
use crate::config::{FsPartition, FsSetupConfig};
use tracing::{debug, info, warn};

/// Create filesystems according to the `fs_setup` configuration
pub async fn setup_filesystems(entries: &[FsSetupConfig]) -> Result<(), CloudInitError> {
    for entry in entries {
        if let Err(e) = setup_filesystem(entry).await {
            warn!(
                "Failed to set up filesystem on {}: {}",
                entry.device, e
            );
        }
    }
    Ok(())
}

/// Set up a single filesystem entry
pub async fn setup_filesystem(entry: &FsSetupConfig) -> Result<(), CloudInitError> {
    let fs_type = entry.filesystem.to_lowercase();
    let device = resolve_device(&entry.device, entry.partition.as_ref()).await?;

    info!(
        "Setting up {} filesystem on {}{}",
        fs_type,
        device,
        entry
            .label
            .as_deref()
            .map(|l| format!(" (label: {})", l))
            .unwrap_or_default()
    );

    // Overwrite protection: skip if a filesystem already exists and overwrite is not set
    if !entry.overwrite.unwrap_or(false) {
        if let Some(existing_fs) = detect_filesystem(&device).await? {
            // If replace_fs is set, only overwrite if existing type matches
            if let Some(ref replace_type) = entry.replace_fs {
                if existing_fs.to_lowercase() != replace_type.to_lowercase() {
                    info!(
                        "Skipping {}: existing filesystem '{}' does not match replace_fs '{}'",
                        device, existing_fs, replace_type
                    );
                    return Ok(());
                }
                info!(
                    "Replacing {} filesystem on {} with {}",
                    existing_fs, device, fs_type
                );
            } else {
                info!(
                    "Skipping {}: already contains a '{}' filesystem (overwrite=false)",
                    device, existing_fs
                );
                return Ok(());
            }
        }
    }

    match fs_type.as_str() {
        "ext4" | "ext3" | "ext2" => create_ext_fs(&device, &fs_type, entry).await,
        "xfs" => create_xfs_fs(&device, entry).await,
        "btrfs" => create_btrfs_fs(&device, entry).await,
        "swap" => create_swap(&device, entry).await,
        other => Err(CloudInitError::Module {
            module: "fs_setup".to_string(),
            message: format!("Unsupported filesystem type: '{}'", other),
        }),
    }
}

/// Resolve the actual device path, honoring partition spec
///
/// * `"none"` / `None` – use the device path as-is (whole disk or partition)
/// * `"auto"` – append partition 1 if the path is a raw block device (e.g.
///   `/dev/sdb` → `/dev/sdb1`)
/// * A numeric partition – append the number (e.g. `/dev/sdb` + 2 → `/dev/sdb2`)
async fn resolve_device(
    device: &str,
    partition: Option<&FsPartition>,
) -> Result<String, CloudInitError> {
    match partition {
        None => Ok(device.to_string()),
        Some(FsPartition::Named(s)) if s == "none" => Ok(device.to_string()),
        Some(FsPartition::Named(s)) if s == "auto" => {
            // Use blkid/lsblk to figure out whether 'device' is a disk with no
            // partition table. For simplicity we just append "1".
            let candidate = format!("{}1", device);
            debug!(
                "partition=auto: using first partition candidate '{}'",
                candidate
            );
            Ok(candidate)
        }
        Some(FsPartition::Number(n)) => {
            let resolved = format!("{}{}", device, n);
            debug!("partition={}: using '{}'", n, resolved);
            Ok(resolved)
        }
        Some(FsPartition::Named(s)) => Err(CloudInitError::Module {
            module: "fs_setup".to_string(),
            message: format!("Unknown partition specification: '{}'", s),
        }),
    }
}

/// Detect the filesystem type already present on a device using `blkid`
///
/// Returns `None` when the device has no recognizable filesystem.
async fn detect_filesystem(device: &str) -> Result<Option<String>, CloudInitError> {
    debug!("Detecting existing filesystem on {}", device);

    let output = tokio::process::Command::new("blkid")
        .args(["-o", "value", "-s", "TYPE", device])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if output.status.success() {
        let fs_type = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();
        if fs_type.is_empty() {
            return Ok(None);
        }
        return Ok(Some(fs_type));
    }

    // blkid exits non-zero when no filesystem is found – that is not an error
    Ok(None)
}

/// Create an ext2/ext3/ext4 filesystem
async fn create_ext_fs(
    device: &str,
    fs_type: &str,
    entry: &FsSetupConfig,
) -> Result<(), CloudInitError> {
    let mkfs = format!("mkfs.{}", fs_type);
    let mut args: Vec<String> = Vec::new();

    if let Some(ref label) = entry.label {
        args.push("-L".to_string());
        args.push(label.clone());
    }

    args.extend(entry.extra_opts.iter().cloned());
    args.push(device.to_string());

    run_mkfs(&mkfs, &args, device, fs_type).await
}

/// Create an XFS filesystem
async fn create_xfs_fs(device: &str, entry: &FsSetupConfig) -> Result<(), CloudInitError> {
    let mut args: Vec<String> = Vec::new();

    if let Some(ref label) = entry.label {
        args.push("-L".to_string());
        args.push(label.clone());
    }

    // Force creation even if a filesystem header is detected
    args.push("-f".to_string());
    args.extend(entry.extra_opts.iter().cloned());
    args.push(device.to_string());

    run_mkfs("mkfs.xfs", &args, device, "xfs").await
}

/// Create a Btrfs filesystem
async fn create_btrfs_fs(device: &str, entry: &FsSetupConfig) -> Result<(), CloudInitError> {
    let mut args: Vec<String> = Vec::new();

    if let Some(ref label) = entry.label {
        args.push("--label".to_string());
        args.push(label.clone());
    }

    // Force creation
    args.push("-f".to_string());
    args.extend(entry.extra_opts.iter().cloned());
    args.push(device.to_string());

    run_mkfs("mkfs.btrfs", &args, device, "btrfs").await
}

/// Set up a swap area
async fn create_swap(device: &str, entry: &FsSetupConfig) -> Result<(), CloudInitError> {
    let mut args: Vec<String> = Vec::new();

    if let Some(ref label) = entry.label {
        args.push("-L".to_string());
        args.push(label.clone());
    }

    args.extend(entry.extra_opts.iter().cloned());
    args.push(device.to_string());

    run_mkfs("mkswap", &args, device, "swap").await
}

/// Invoke a mkfs command and handle the result
async fn run_mkfs(
    cmd: &str,
    args: &[String],
    device: &str,
    fs_type: &str,
) -> Result<(), CloudInitError> {
    debug!("Running: {} {:?}", cmd, args);

    let output = tokio::process::Command::new(cmd)
        .args(args)
        .output()
        .await
        .map_err(|e| CloudInitError::Command(format!("Failed to run '{}': {}", cmd, e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::Module {
            module: "fs_setup".to_string(),
            message: format!(
                "Failed to create {} filesystem on {}: {}",
                fs_type, device, stderr
            ),
        });
    }

    info!("Successfully created {} filesystem on {}", fs_type, device);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FsPartition, FsSetupConfig};

    fn make_entry(fs: &str, device: &str) -> FsSetupConfig {
        FsSetupConfig {
            filesystem: fs.to_string(),
            device: device.to_string(),
            label: None,
            partition: None,
            overwrite: None,
            replace_fs: None,
            extra_opts: vec![],
        }
    }

    // ── resolve_device ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_resolve_device_no_partition() {
        let result = resolve_device("/dev/sda1", None).await.unwrap();
        assert_eq!(result, "/dev/sda1");
    }

    #[tokio::test]
    async fn test_resolve_device_partition_none() {
        let part = FsPartition::Named("none".to_string());
        let result = resolve_device("/dev/sdb", Some(&part)).await.unwrap();
        assert_eq!(result, "/dev/sdb");
    }

    #[tokio::test]
    async fn test_resolve_device_partition_auto() {
        let part = FsPartition::Named("auto".to_string());
        let result = resolve_device("/dev/sdb", Some(&part)).await.unwrap();
        assert_eq!(result, "/dev/sdb1");
    }

    #[tokio::test]
    async fn test_resolve_device_partition_number() {
        let part = FsPartition::Number(2);
        let result = resolve_device("/dev/sdb", Some(&part)).await.unwrap();
        assert_eq!(result, "/dev/sdb2");
    }

    #[tokio::test]
    async fn test_resolve_device_unknown_partition_string() {
        let part = FsPartition::Named("bogus".to_string());
        let result = resolve_device("/dev/sdb", Some(&part)).await;
        assert!(result.is_err());
    }

    // ── setup_filesystem (unsupported type) ───────────────────────────────────

    #[tokio::test]
    async fn test_unsupported_filesystem_type() {
        // Use a device path that is guaranteed not to exist so that overwrite
        // protection doesn't short-circuit before we reach the type check.
        let entry = make_entry("zfs", "/dev/cloud-init-rs-no-such-device");
        let result = setup_filesystem(&entry).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Unsupported filesystem type"));
    }

    // ── setup_filesystems (empty list) ────────────────────────────────────────

    #[tokio::test]
    async fn test_setup_filesystems_empty() {
        let result = setup_filesystems(&[]).await;
        assert!(result.is_ok());
    }
}
