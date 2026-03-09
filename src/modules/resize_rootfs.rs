//! Resize root filesystem module
//!
//! Detects the root filesystem type and runs the appropriate resize tool
//! after partition growth (growpart). Supports ext4, xfs, and btrfs.

use crate::CloudInitError;
use tracing::{debug, info, warn};

/// Filesystem types supported for resizing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemType {
    Ext4,
    Xfs,
    Btrfs,
    Unknown(String),
}

impl FilesystemType {
    fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "ext4" => Self::Ext4,
            "ext3" => Self::Ext4,
            "ext2" => Self::Ext4,
            "xfs" => Self::Xfs,
            "btrfs" => Self::Btrfs,
            other => Self::Unknown(other.to_string()),
        }
    }
}

/// Information about the root filesystem
#[derive(Debug, Clone)]
pub struct RootFilesystem {
    pub device: String,
    pub fs_type: FilesystemType,
}

/// Resize the root filesystem if `resize_rootfs` is enabled (or not explicitly disabled)
pub async fn resize_rootfs(enabled: Option<bool>) -> Result<(), CloudInitError> {
    // Default is enabled (true) if not specified
    if enabled == Some(false) {
        debug!("resize_rootfs is disabled, skipping");
        return Ok(());
    }

    info!("resize_rootfs: detecting root filesystem");

    let root_fs = match detect_root_filesystem().await {
        Ok(fs) => fs,
        Err(e) => {
            warn!("resize_rootfs: could not detect root filesystem: {}", e);
            return Ok(());
        }
    };

    info!(
        "resize_rootfs: root filesystem is {:?} on {}",
        root_fs.fs_type, root_fs.device
    );

    match &root_fs.fs_type {
        FilesystemType::Ext4 => resize_ext4(&root_fs.device).await,
        FilesystemType::Xfs => resize_xfs().await,
        FilesystemType::Btrfs => resize_btrfs().await,
        FilesystemType::Unknown(fs) => {
            warn!(
                "resize_rootfs: unsupported filesystem type '{}', skipping",
                fs
            );
            Ok(())
        }
    }
}

/// Detect the root filesystem device and type by reading /proc/mounts
pub async fn detect_root_filesystem() -> Result<RootFilesystem, CloudInitError> {
    // Try findmnt first (most reliable, handles overlays and bind mounts)
    if let Ok(root_fs) = detect_via_findmnt().await {
        return Ok(root_fs);
    }

    // Fall back to /proc/mounts
    detect_via_proc_mounts().await
}

/// Detect root filesystem using findmnt command
async fn detect_via_findmnt() -> Result<RootFilesystem, CloudInitError> {
    debug!("resize_rootfs: trying findmnt for root filesystem detection");

    let output = tokio::process::Command::new("findmnt")
        .args(["-n", "-o", "SOURCE,FSTYPE", "/"])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        return Err(CloudInitError::Command(
            "findmnt returned non-zero exit code".to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.trim();
    if line.is_empty() {
        return Err(CloudInitError::InvalidData(
            "findmnt returned empty output".to_string(),
        ));
    }

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(CloudInitError::InvalidData(format!(
            "unexpected findmnt output: {}",
            line
        )));
    }

    Ok(RootFilesystem {
        device: parts[0].to_string(),
        fs_type: FilesystemType::from_str(parts[1]),
    })
}

/// Detect root filesystem by parsing /proc/mounts
async fn detect_via_proc_mounts() -> Result<RootFilesystem, CloudInitError> {
    debug!("resize_rootfs: reading /proc/mounts for root filesystem detection");

    let content = tokio::fs::read_to_string("/proc/mounts")
        .await
        .map_err(CloudInitError::Io)?;

    parse_root_from_mounts(&content)
        .ok_or_else(|| CloudInitError::InvalidData(
            "root filesystem not found in /proc/mounts - verify that /proc/mounts is accessible and a root filesystem is mounted".to_string()
        ))
}

/// Parse /proc/mounts content to find the root filesystem entry
pub fn parse_root_from_mounts(mounts_content: &str) -> Option<RootFilesystem> {
    // /proc/mounts format: <device> <mountpoint> <fstype> <options> <dump> <pass>
    // We want the entry where mountpoint == "/"
    // If there are multiple entries for "/", prefer the last one (most recent mount wins)
    let mut result: Option<RootFilesystem> = None;

    for line in mounts_content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }

        let device = parts[0];
        let mountpoint = parts[1];
        let fstype = parts[2];

        if mountpoint == "/" {
            result = Some(RootFilesystem {
                device: device.to_string(),
                fs_type: FilesystemType::from_str(fstype),
            });
        }
    }

    result
}

/// Resize an ext2/ext3/ext4 filesystem using resize2fs
async fn resize_ext4(device: &str) -> Result<(), CloudInitError> {
    info!("resize_rootfs: resizing ext4 filesystem on {}", device);

    let output = tokio::process::Command::new("resize2fs")
        .arg(device)
        .output()
        .await
        .map_err(|e| CloudInitError::Command(format!("Failed to run resize2fs: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("resize2fs failed: {}", stderr);
        return Err(CloudInitError::Command(format!(
            "resize2fs failed: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        debug!("resize2fs output: {}", stdout.trim());
    }

    info!("resize_rootfs: ext4 filesystem resized successfully");
    Ok(())
}

/// Resize an XFS filesystem using xfs_growfs
async fn resize_xfs() -> Result<(), CloudInitError> {
    info!("resize_rootfs: resizing xfs filesystem at /");

    // xfs_growfs operates on the mount point, not the device
    let output = tokio::process::Command::new("xfs_growfs")
        .arg("/")
        .output()
        .await
        .map_err(|e| CloudInitError::Command(format!("Failed to run xfs_growfs: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("xfs_growfs failed: {}", stderr);
        return Err(CloudInitError::Command(format!(
            "xfs_growfs failed: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        debug!("xfs_growfs output: {}", stdout.trim());
    }

    info!("resize_rootfs: xfs filesystem resized successfully");
    Ok(())
}

/// Resize a Btrfs filesystem using btrfs filesystem resize
async fn resize_btrfs() -> Result<(), CloudInitError> {
    info!("resize_rootfs: resizing btrfs filesystem at /");

    // "max" tells btrfs to use all available space
    let output = tokio::process::Command::new("btrfs")
        .args(["filesystem", "resize", "max", "/"])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(format!("Failed to run btrfs: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("btrfs filesystem resize failed: {}", stderr);
        return Err(CloudInitError::Command(format!(
            "btrfs filesystem resize failed: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        debug!("btrfs output: {}", stdout.trim());
    }

    info!("resize_rootfs: btrfs filesystem resized successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_type_from_str() {
        assert_eq!(FilesystemType::from_str("ext4"), FilesystemType::Ext4);
        assert_eq!(FilesystemType::from_str("ext3"), FilesystemType::Ext4);
        assert_eq!(FilesystemType::from_str("ext2"), FilesystemType::Ext4);
        assert_eq!(FilesystemType::from_str("xfs"), FilesystemType::Xfs);
        assert_eq!(FilesystemType::from_str("btrfs"), FilesystemType::Btrfs);
        assert_eq!(
            FilesystemType::from_str("tmpfs"),
            FilesystemType::Unknown("tmpfs".to_string())
        );
        assert_eq!(
            FilesystemType::from_str("vfat"),
            FilesystemType::Unknown("vfat".to_string())
        );
    }

    #[test]
    fn test_filesystem_type_case_insensitive() {
        assert_eq!(FilesystemType::from_str("EXT4"), FilesystemType::Ext4);
        assert_eq!(FilesystemType::from_str("XFS"), FilesystemType::Xfs);
        assert_eq!(FilesystemType::from_str("BTRFS"), FilesystemType::Btrfs);
    }

    #[test]
    fn test_parse_root_from_mounts_ext4() {
        let mounts = "\
sysfs /sys sysfs rw,nosuid,nodev,noexec,relatime 0 0
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
/dev/sda1 / ext4 rw,relatime 0 0
tmpfs /tmp tmpfs rw,nosuid,nodev 0 0
";
        let root = parse_root_from_mounts(mounts).unwrap();
        assert_eq!(root.device, "/dev/sda1");
        assert_eq!(root.fs_type, FilesystemType::Ext4);
    }

    #[test]
    fn test_parse_root_from_mounts_xfs() {
        let mounts = "\
/dev/nvme0n1p1 / xfs rw,relatime 0 0
tmpfs /tmp tmpfs rw 0 0
";
        let root = parse_root_from_mounts(mounts).unwrap();
        assert_eq!(root.device, "/dev/nvme0n1p1");
        assert_eq!(root.fs_type, FilesystemType::Xfs);
    }

    #[test]
    fn test_parse_root_from_mounts_btrfs() {
        let mounts = "\
/dev/vda1 / btrfs rw,relatime,compress=zstd 0 0
";
        let root = parse_root_from_mounts(mounts).unwrap();
        assert_eq!(root.device, "/dev/vda1");
        assert_eq!(root.fs_type, FilesystemType::Btrfs);
    }

    #[test]
    fn test_parse_root_from_mounts_last_entry_wins() {
        // When root appears multiple times, the last entry should be used
        let mounts = "\
/dev/sda1 / ext4 rw,relatime 0 0
/dev/sda2 / btrfs rw,relatime 0 0
";
        let root = parse_root_from_mounts(mounts).unwrap();
        assert_eq!(root.device, "/dev/sda2");
        assert_eq!(root.fs_type, FilesystemType::Btrfs);
    }

    #[test]
    fn test_parse_root_from_mounts_no_root() {
        let mounts = "\
tmpfs /tmp tmpfs rw 0 0
proc /proc proc rw 0 0
";
        let root = parse_root_from_mounts(mounts);
        assert!(root.is_none());
    }

    #[test]
    fn test_parse_root_from_mounts_empty() {
        let root = parse_root_from_mounts("");
        assert!(root.is_none());
    }

    #[test]
    fn test_parse_root_from_mounts_skips_comments() {
        let mounts = "\
# This is a comment
/dev/sda1 / ext4 rw,relatime 0 0
";
        let root = parse_root_from_mounts(mounts).unwrap();
        assert_eq!(root.device, "/dev/sda1");
    }

    #[test]
    fn test_parse_root_from_mounts_uuid_device() {
        let mounts = "\
UUID=abc123 / ext4 rw,relatime 0 0
";
        let root = parse_root_from_mounts(mounts).unwrap();
        assert_eq!(root.device, "UUID=abc123");
        assert_eq!(root.fs_type, FilesystemType::Ext4);
    }

    #[test]
    fn test_parse_root_from_mounts_skips_short_lines() {
        let mounts = "\
/dev/sda1 /
/dev/sda2 / ext4 rw,relatime 0 0
";
        let root = parse_root_from_mounts(mounts).unwrap();
        assert_eq!(root.device, "/dev/sda2");
    }
}
