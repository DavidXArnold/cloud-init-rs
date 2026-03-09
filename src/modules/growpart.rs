//! Growpart module - grow partitions to fill available disk space
//!
//! Supports growing partitions using the `growpart` utility or `sgdisk` for GPT
//! partitions. Runs in the local stage (before network) to ensure disk space is
//! available for subsequent operations.

use crate::CloudInitError;
use crate::config::GrowpartConfig;
use std::path::Path;
use tokio::fs;
use tracing::{debug, info, warn};

/// Partition growth mode
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrowMode {
    /// Auto-detect the best available tool (growpart preferred, then sgdisk)
    Auto,
    /// Use the `growpart` utility (handles both MBR and GPT)
    GrowPart,
    /// Use `sgdisk` (GPT only)
    Sgdisk,
    /// Disable partition growing
    Off,
}

impl GrowMode {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "growpart" => Self::GrowPart,
            "sgdisk" => Self::Sgdisk,
            "off" | "false" | "disabled" => Self::Off,
            _ => Self::Auto, // covers "auto" and unknown values
        }
    }
}

/// Grow partitions according to the provided configuration
///
/// Checks for disable markers, resolves mountpoints to block devices, and
/// invokes the appropriate grow tool for each configured device.
pub async fn grow_partitions(config: &GrowpartConfig) -> Result<(), CloudInitError> {
    let mode = config
        .mode
        .as_deref()
        .map(GrowMode::from_str)
        .unwrap_or(GrowMode::Auto);

    if mode == GrowMode::Off {
        debug!("Growpart disabled via configuration (mode=off)");
        return Ok(());
    }

    // Honor the disable marker files unless overridden
    if !config.ignore_growroot_disabled.unwrap_or(false) {
        for marker in &["/etc/growroot-disabled", "/run/growroot-disabled"] {
            if Path::new(marker).exists() {
                info!("Growpart disabled by {}", marker);
                return Ok(());
            }
        }
    }

    let default_devices = vec!["/".to_string()];
    let devices = config.devices.as_ref().unwrap_or(&default_devices);

    if devices.is_empty() {
        debug!("No devices configured for growpart");
        return Ok(());
    }

    for device in devices {
        debug!("Processing growpart for device: {}", device);
        if let Err(e) = grow_device(device, &mode).await {
            warn!("Failed to grow device {}: {}", device, e);
        }
    }

    Ok(())
}

/// Grow a single device or mountpoint
async fn grow_device(device: &str, mode: &GrowMode) -> Result<(), CloudInitError> {
    // Resolve a mountpoint (e.g. "/") to the underlying block device
    let partition = if device.starts_with('/') && !device.starts_with("/dev/") {
        resolve_mountpoint(device).await?
    } else {
        device.to_string()
    };

    debug!("Resolved device: {} -> {}", device, partition);

    let (disk, part_num) = parse_partition_device(&partition)?;
    info!("Growing partition {} on disk {}", part_num, disk);

    match mode {
        GrowMode::Auto => grow_auto(&disk, part_num).await,
        GrowMode::GrowPart => grow_with_growpart(&disk, part_num).await,
        GrowMode::Sgdisk => grow_with_sgdisk(&disk, part_num).await,
        GrowMode::Off => Ok(()),
    }
}

/// Auto-detect the best available tool and grow the partition
async fn grow_auto(disk: &str, partition_num: u32) -> Result<(), CloudInitError> {
    if command_exists("growpart").await {
        return grow_with_growpart(disk, partition_num).await;
    }

    // Fall back to sgdisk for GPT disks
    if command_exists("sgdisk").await {
        if detect_partition_table(disk).await.as_deref() == Some("gpt") {
            return grow_with_sgdisk(disk, partition_num).await;
        }
    }

    warn!(
        "No supported partition grow tool found (growpart or sgdisk). \
         Install cloud-guest-utils (growpart) or gdisk (sgdisk)."
    );
    Ok(())
}

/// Grow a partition using the `growpart` utility
///
/// `growpart` supports both MBR and GPT partition tables.
pub async fn grow_with_growpart(disk: &str, partition_num: u32) -> Result<(), CloudInitError> {
    info!(
        "Growing partition {} on {} using growpart",
        partition_num, disk
    );

    let output = tokio::process::Command::new("growpart")
        .arg(disk)
        .arg(partition_num.to_string())
        .output()
        .await
        .map_err(|e| CloudInitError::Command(format!("Failed to execute growpart: {}", e)))?;

    if output.status.success() {
        info!("Successfully grew partition {} on {}", partition_num, disk);
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // growpart exits 1 with "NOCHANGE" when the partition is already at maximum size
    if stdout.contains("NOCHANGE") || stderr.contains("NOCHANGE") {
        debug!(
            "Partition {} on {} is already at maximum size (NOCHANGE)",
            partition_num, disk
        );
        return Ok(());
    }

    Err(CloudInitError::Module {
        module: "growpart".to_string(),
        message: format!("growpart failed: {}", stderr),
    })
}

/// Grow a GPT partition using `sgdisk`
///
/// Moves the secondary GPT header to end of disk, then deletes and recreates
/// the partition extending it to the end of available space.
pub async fn grow_with_sgdisk(disk: &str, partition_num: u32) -> Result<(), CloudInitError> {
    info!(
        "Growing GPT partition {} on {} using sgdisk",
        partition_num, disk
    );

    // Step 1: Move the secondary GPT header to the physical end of the disk
    let move_out = tokio::process::Command::new("sgdisk")
        .args(["--move-second-header", disk])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(format!("Failed to execute sgdisk: {}", e)))?;

    if !move_out.status.success() {
        let stderr = String::from_utf8_lossy(&move_out.stderr);
        warn!(
            "sgdisk --move-second-header returned non-zero (non-fatal): {}",
            stderr
        );
    }

    // Step 2: Read the partition's start sector so we can recreate it
    let start_sector = get_partition_start(disk, partition_num).await?;

    // Step 3: Delete the old partition entry and recreate it ending at the last sector (0 = end)
    let output = tokio::process::Command::new("sgdisk")
        .args([
            "--delete",
            &partition_num.to_string(),
            "--new",
            &format!("{}:{}:0", partition_num, start_sector),
            disk,
        ])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(format!("Failed to execute sgdisk: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::Module {
            module: "growpart".to_string(),
            message: format!("sgdisk failed to grow partition: {}", stderr),
        });
    }

    // Step 4: Notify the kernel about the updated partition table
    let _ = tokio::process::Command::new("partx")
        .args(["-u", "--nr", &partition_num.to_string(), disk])
        .output()
        .await;

    info!(
        "Successfully grew GPT partition {} on {} using sgdisk",
        partition_num, disk
    );
    Ok(())
}

/// Retrieve the first sector of a partition via `sgdisk --info`
async fn get_partition_start(disk: &str, partition_num: u32) -> Result<u64, CloudInitError> {
    let output = tokio::process::Command::new("sgdisk")
        .args(["--info", &partition_num.to_string(), disk])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(format!("Failed to run sgdisk --info: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::Module {
            module: "growpart".to_string(),
            message: format!("sgdisk --info failed: {}", stderr),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // sgdisk --info prints: "First sector: NNNNNN (at ...)"
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("First sector:") {
            if let Some(sector_str) = rest.split_whitespace().next() {
                if let Ok(sector) = sector_str.parse::<u64>() {
                    return Ok(sector);
                }
            }
        }
    }

    Err(CloudInitError::Module {
        module: "growpart".to_string(),
        message: format!(
            "Could not parse start sector from sgdisk --info for partition {} on {}",
            partition_num, disk
        ),
    })
}

/// Resolve a mountpoint to its underlying block device
///
/// Uses `findmnt` when available, falling back to `/proc/mounts`.
pub async fn resolve_mountpoint(mountpoint: &str) -> Result<String, CloudInitError> {
    // Prefer findmnt as it handles bind mounts correctly
    let output = tokio::process::Command::new("findmnt")
        .args(["--noheadings", "--output", "SOURCE", mountpoint])
        .output()
        .await;

    if let Ok(out) = output {
        if out.status.success() {
            let device = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !device.is_empty() && device.starts_with("/dev/") {
                return Ok(device);
            }
        }
    }

    // Fallback: scan /proc/mounts
    let mounts = fs::read_to_string("/proc/mounts")
        .await
        .map_err(|e| CloudInitError::InvalidData(format!("Cannot read /proc/mounts: {}", e)))?;

    for line in mounts.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == mountpoint && parts[0].starts_with("/dev/") {
            return Ok(parts[0].to_string());
        }
    }

    Err(CloudInitError::Module {
        module: "growpart".to_string(),
        message: format!("Cannot find block device for mountpoint: {}", mountpoint),
    })
}

/// Parse a partition device path into `(disk, partition_number)`
///
/// Handles standard SCSI/SATA (`/dev/sda1`), NVMe (`/dev/nvme0n1p1`),
/// eMMC/SD (`/dev/mmcblk0p1`), and loop devices (`/dev/loop0p1`).
///
/// # Examples
/// ```
/// # use cloud_init_rs::modules::growpart::parse_partition_device;
/// assert_eq!(parse_partition_device("/dev/sda1").unwrap(),  ("/dev/sda".to_string(),  1));
/// assert_eq!(parse_partition_device("/dev/xvda2").unwrap(), ("/dev/xvda".to_string(), 2));
/// assert_eq!(parse_partition_device("/dev/nvme0n1p3").unwrap(), ("/dev/nvme0n1".to_string(), 3));
/// assert_eq!(parse_partition_device("/dev/mmcblk0p2").unwrap(), ("/dev/mmcblk0".to_string(), 2));
/// ```
pub fn parse_partition_device(device: &str) -> Result<(String, u32), CloudInitError> {
    // Devices whose partition suffix uses a 'p' separator: nvme, mmcblk, loop
    // e.g. /dev/nvme0n1p1, /dev/mmcblk0p2, /dev/loop0p1
    let uses_p_separator = device.contains("nvme")
        || device.contains("mmcblk")
        || device.contains("loop")
        || device.contains("nbd");

    if uses_p_separator {
        if let Some(p_pos) = device.rfind('p') {
            let suffix = &device[p_pos + 1..];
            if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(part_num) = suffix.parse::<u32>() {
                    let disk = device[..p_pos].to_string();
                    return Ok((disk, part_num));
                }
            }
        }
    }

    // Standard devices: trailing digits are the partition number
    // e.g. /dev/sda1 -> (/dev/sda, 1), /dev/xvda12 -> (/dev/xvda, 12)
    let digits_start = device
        .rfind(|c: char| !c.is_ascii_digit())
        .map(|i| i + 1)
        .ok_or_else(|| CloudInitError::Module {
            module: "growpart".to_string(),
            message: format!("Cannot parse partition device: {}", device),
        })?;

    if digits_start >= device.len() {
        return Err(CloudInitError::Module {
            module: "growpart".to_string(),
            message: format!("Device has no trailing partition number: {}", device),
        });
    }

    let disk = device[..digits_start].to_string();
    let part_num = device[digits_start..].parse::<u32>().map_err(|_| {
        CloudInitError::Module {
            module: "growpart".to_string(),
            message: format!("Cannot parse partition number from: {}", device),
        }
    })?;

    Ok((disk, part_num))
}

/// Detect the partition table type on a disk (`"gpt"` or `"dos"`/`"mbr"`)
///
/// Tries `blkid -p` first (most reliable), falls back to `lsblk`.
pub async fn detect_partition_table(disk: &str) -> Option<String> {
    // blkid -p probes for partition-table type
    let output = tokio::process::Command::new("blkid")
        .args(["-p", "-o", "value", "-s", "PTTYPE", disk])
        .output()
        .await;

    if let Ok(out) = output {
        if out.status.success() {
            let pt = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
            if !pt.is_empty() {
                return Some(pt);
            }
        }
    }

    // Fallback: lsblk
    let output = tokio::process::Command::new("lsblk")
        .args(["--noheadings", "--output", "PTTYPE", disk])
        .output()
        .await;

    if let Ok(out) = output {
        if out.status.success() {
            let pt = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
            if !pt.is_empty() && pt != "pttype" {
                return Some(pt);
            }
        }
    }

    None
}

/// Check whether a command is available in `PATH`
async fn command_exists(cmd: &str) -> bool {
    tokio::process::Command::new("which")
        .arg(cmd)
        .output()
        .await
        .is_ok_and(|o| o.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== GrowMode Parsing ====================

    #[test]
    fn test_grow_mode_auto() {
        assert_eq!(GrowMode::from_str("auto"), GrowMode::Auto);
        assert_eq!(GrowMode::from_str("AUTO"), GrowMode::Auto);
        assert_eq!(GrowMode::from_str("unknown"), GrowMode::Auto);
    }

    #[test]
    fn test_grow_mode_growpart() {
        assert_eq!(GrowMode::from_str("growpart"), GrowMode::GrowPart);
        assert_eq!(GrowMode::from_str("GROWPART"), GrowMode::GrowPart);
    }

    #[test]
    fn test_grow_mode_sgdisk() {
        assert_eq!(GrowMode::from_str("sgdisk"), GrowMode::Sgdisk);
        assert_eq!(GrowMode::from_str("SGDISK"), GrowMode::Sgdisk);
    }

    #[test]
    fn test_grow_mode_off() {
        assert_eq!(GrowMode::from_str("off"), GrowMode::Off);
        assert_eq!(GrowMode::from_str("false"), GrowMode::Off);
        assert_eq!(GrowMode::from_str("disabled"), GrowMode::Off);
        assert_eq!(GrowMode::from_str("OFF"), GrowMode::Off);
    }

    // ==================== Partition Device Parsing ====================

    #[test]
    fn test_parse_standard_device_sda() {
        let (disk, part) = parse_partition_device("/dev/sda1").unwrap();
        assert_eq!(disk, "/dev/sda");
        assert_eq!(part, 1);
    }

    #[test]
    fn test_parse_standard_device_xvda() {
        let (disk, part) = parse_partition_device("/dev/xvda2").unwrap();
        assert_eq!(disk, "/dev/xvda");
        assert_eq!(part, 2);
    }

    #[test]
    fn test_parse_standard_device_vda() {
        let (disk, part) = parse_partition_device("/dev/vda3").unwrap();
        assert_eq!(disk, "/dev/vda");
        assert_eq!(part, 3);
    }

    #[test]
    fn test_parse_nvme_device() {
        let (disk, part) = parse_partition_device("/dev/nvme0n1p1").unwrap();
        assert_eq!(disk, "/dev/nvme0n1");
        assert_eq!(part, 1);
    }

    #[test]
    fn test_parse_nvme_device_second_partition() {
        let (disk, part) = parse_partition_device("/dev/nvme1n1p3").unwrap();
        assert_eq!(disk, "/dev/nvme1n1");
        assert_eq!(part, 3);
    }

    #[test]
    fn test_parse_mmcblk_device() {
        let (disk, part) = parse_partition_device("/dev/mmcblk0p2").unwrap();
        assert_eq!(disk, "/dev/mmcblk0");
        assert_eq!(part, 2);
    }

    #[test]
    fn test_parse_loop_device() {
        let (disk, part) = parse_partition_device("/dev/loop0p1").unwrap();
        assert_eq!(disk, "/dev/loop0");
        assert_eq!(part, 1);
    }

    #[test]
    fn test_parse_multi_digit_partition() {
        let (disk, part) = parse_partition_device("/dev/sda12").unwrap();
        assert_eq!(disk, "/dev/sda");
        assert_eq!(part, 12);
    }

    #[test]
    fn test_parse_device_no_partition_number() {
        // A disk without a partition number should return an error
        let result = parse_partition_device("/dev/sda");
        assert!(result.is_err());
    }

    // ==================== grow_partitions with mode=off ====================

    #[tokio::test]
    async fn test_grow_partitions_mode_off() {
        let config = GrowpartConfig {
            mode: Some("off".to_string()),
            devices: Some(vec!["/dev/sda1".to_string()]),
            ignore_growroot_disabled: None,
        };
        // Should return Ok(()) immediately without doing anything
        let result = grow_partitions(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_grow_partitions_mode_false() {
        let config = GrowpartConfig {
            mode: Some("false".to_string()),
            devices: None,
            ignore_growroot_disabled: None,
        };
        let result = grow_partitions(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_grow_partitions_empty_devices() {
        let config = GrowpartConfig {
            mode: Some("auto".to_string()),
            devices: Some(vec![]),
            ignore_growroot_disabled: Some(true),
        };
        // Empty device list should return Ok(()) without error
        let result = grow_partitions(&config).await;
        assert!(result.is_ok());
    }
}
