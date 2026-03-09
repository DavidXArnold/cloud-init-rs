//! Mounts configuration module
//!
//! Configures filesystem mounts by:
//! - Updating `/etc/fstab` with entries from cloud-config
//! - Creating mount point directories
//! - Mounting the filesystems
//! - Creating and activating swap files or partitions

use crate::CloudInitError;
use crate::config::{MountEntry, MountFieldValue, SwapConfig};
use tokio::fs;
use tracing::{debug, info, warn};

const FSTAB_PATH: &str = "/etc/fstab";

/// Built-in default values for fstab fields (positions 2–5).
const DEFAULT_FS_TYPE: &str = "auto";
const DEFAULT_OPTIONS: &str = "defaults";
const DEFAULT_DUMP: &str = "0";
const DEFAULT_PASS: &str = "2";

/// A fully-resolved mount entry ready to be written to fstab.
#[derive(Debug, Clone)]
struct ResolvedMount {
    device: String,
    mount_point: String,
    fs_type: String,
    options: String,
    dump: String,
    pass: String,
}

impl ResolvedMount {
    /// Format as a single fstab line (tab-separated).
    fn to_fstab_line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            self.device, self.mount_point, self.fs_type, self.options, self.dump, self.pass
        )
    }

    /// Return `true` if this entry represents a swap device.
    fn is_swap(&self) -> bool {
        self.fs_type == "swap" || self.mount_point == "none" || self.mount_point == "swap"
    }
}

/// Resolve a raw `MountEntry` by filling in missing fields from `defaults`
/// and, ultimately, from the built-in fallback values.
///
/// Returns `None` if the required device or mount-point fields are absent.
fn resolve_mount(entry: &MountEntry, defaults: &[Option<String>]) -> Option<ResolvedMount> {
    let fields = entry.fields();

    // Positions 0 and 1 (device, mount point) are required.
    let device = fields
        .first()
        .and_then(|v| v.as_deref())
        .map(str::to_string)?;
    let mount_point = fields
        .get(1)
        .and_then(|v| v.as_deref())
        .map(str::to_string)?;

    // Helper: entry field → user default → built-in default.
    let get_field = |idx: usize, builtin: &str| -> String {
        if let Some(Some(val)) = fields.get(idx) {
            return val.clone();
        }
        if let Some(Some(val)) = defaults.get(idx) {
            return val.clone();
        }
        builtin.to_string()
    };

    Some(ResolvedMount {
        device,
        mount_point,
        fs_type: get_field(2, DEFAULT_FS_TYPE),
        options: get_field(3, DEFAULT_OPTIONS),
        dump: get_field(4, DEFAULT_DUMP),
        pass: get_field(5, DEFAULT_PASS),
    })
}

/// Configure all mounts specified in the cloud-config.
///
/// Steps performed:
/// 1. Resolve mount entries (apply defaults for missing fields).
/// 2. Update `/etc/fstab`.
/// 3. Create mount-point directories for non-swap entries.
/// 4. Mount the filesystems.
/// 5. Configure any swap file or partition.
pub async fn configure_mounts(
    mounts: &[MountEntry],
    mount_default_fields: &[Option<MountFieldValue>],
    swap: Option<&SwapConfig>,
) -> Result<(), CloudInitError> {
    if mounts.is_empty() && swap.is_none() {
        debug!("No mounts or swap configured, skipping");
        return Ok(());
    }

    // Convert MountFieldValue defaults to plain Option<String>.
    let defaults: Vec<Option<String>> = mount_default_fields
        .iter()
        .map(|f| f.as_ref().map(|v| v.as_str_val()))
        .collect();

    if !mounts.is_empty() {
        info!("Configuring {} mount entries", mounts.len());

        let resolved: Vec<ResolvedMount> = mounts
            .iter()
            .filter_map(|m| resolve_mount(m, &defaults))
            .collect();

        update_fstab(&resolved).await?;
        create_mount_points(&resolved).await?;
        mount_filesystems(&resolved).await?;
    }

    if let Some(swap_config) = swap {
        configure_swap(swap_config).await?;
    }

    Ok(())
}

/// Update `/etc/fstab`, replacing any existing entry for the same device or
/// appending a new line when no prior entry exists.
async fn update_fstab(mounts: &[ResolvedMount]) -> Result<(), CloudInitError> {
    debug!("Updating {}", FSTAB_PATH);

    let existing = fs::read_to_string(FSTAB_PATH).await.unwrap_or_default();

    let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();

    for mount in mounts {
        let new_line = mount.to_fstab_line();

        // Find an existing uncommented entry for this device.
        let existing_idx = lines.iter().position(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                return false;
            }
            trimmed
                .split_whitespace()
                .next()
                .map(|d| d == mount.device)
                .unwrap_or(false)
        });

        match existing_idx {
            Some(idx) => {
                debug!("Updating existing fstab entry for {}", mount.device);
                lines[idx] = new_line;
            }
            None => {
                debug!("Adding new fstab entry for {}", mount.device);
                lines.push(new_line);
            }
        }
    }

    let content = lines.join("\n") + "\n";
    fs::write(FSTAB_PATH, &content)
        .await
        .map_err(CloudInitError::Io)?;

    info!("Updated {}", FSTAB_PATH);
    Ok(())
}

/// Create mount-point directories for all non-swap entries.
async fn create_mount_points(mounts: &[ResolvedMount]) -> Result<(), CloudInitError> {
    for mount in mounts {
        if mount.is_swap() {
            continue;
        }
        debug!("Creating mount point: {}", mount.mount_point);
        if let Err(e) = fs::create_dir_all(&mount.mount_point).await {
            warn!("Failed to create mount point {}: {}", mount.mount_point, e);
        }
    }
    Ok(())
}

/// Mount all non-swap filesystems that are not already mounted.
async fn mount_filesystems(mounts: &[ResolvedMount]) -> Result<(), CloudInitError> {
    for mount in mounts {
        if mount.is_swap() {
            continue;
        }

        if is_mounted(&mount.mount_point).await {
            debug!("{} is already mounted", mount.mount_point);
            continue;
        }

        debug!("Mounting: {}", mount.mount_point);

        let output = tokio::process::Command::new("mount")
            .arg(&mount.mount_point)
            .output()
            .await
            .map_err(|e| CloudInitError::Command(e.to_string()))?;

        if output.status.success() {
            info!("Mounted: {}", mount.mount_point);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to mount {}: {}", mount.mount_point, stderr);
        }
    }
    Ok(())
}

/// Return `true` if `path` is currently a mount point.
async fn is_mounted(path: &str) -> bool {
    tokio::process::Command::new("mountpoint")
        .args(["-q", path])
        .output()
        .await
        .is_ok_and(|o| o.status.success())
}

/// Configure a swap file or partition as described in `swap`.
async fn configure_swap(swap: &SwapConfig) -> Result<(), CloudInitError> {
    let filename = swap.filename.as_deref().unwrap_or("/swap.img");

    let size_mb = resolve_swap_size(swap).await;
    if size_mb == 0 {
        warn!("Resolved swap size is 0 MiB, skipping swap configuration");
        return Ok(());
    }

    info!("Configuring swap: {} ({} MiB)", filename, size_mb);

    // Create the swap file only if it does not already exist.
    if !fs::try_exists(filename).await.unwrap_or(false) {
        create_swap_file(filename, size_mb).await?;
    } else {
        debug!("Swap file {} already exists", filename);
    }

    // Add/update the fstab entry for the swap file.
    let swap_entry = ResolvedMount {
        device: filename.to_string(),
        mount_point: "none".to_string(),
        fs_type: "swap".to_string(),
        options: "sw".to_string(),
        dump: "0".to_string(),
        pass: "0".to_string(),
    };
    update_fstab(&[swap_entry]).await?;

    // Activate the swap.
    let output = tokio::process::Command::new("swapon")
        .arg(filename)
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if output.status.success() {
        info!("Swap activated: {}", filename);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("Failed to activate swap {}: {}", filename, stderr);
    }

    Ok(())
}

/// Determine the swap size in MiB from the `SwapConfig`.
///
/// - `"auto"` or absent → total RAM (capped by `maxsize` if set)
/// - numeric string → parsed as MiB (capped by `maxsize` if set)
async fn resolve_swap_size(swap: &SwapConfig) -> u64 {
    let raw_size = match swap.size.as_deref() {
        Some("auto") | None => get_memory_mib().await,
        Some(s) => s.parse::<u64>().unwrap_or_else(|_| {
            warn!("Invalid swap size '{}', falling back to auto", s);
            // Return 0 to trigger the fallback logic below.
            0
        }),
    };

    // If parse returned 0 due to an error, fall back to RAM size.
    let size = if raw_size == 0 && swap.size.as_deref() != Some("0") {
        get_memory_mib().await
    } else {
        raw_size
    };

    match swap.maxsize {
        Some(max) => size.min(max),
        None => size,
    }
}

/// Read total physical memory from `/proc/meminfo` and return it in MiB.
async fn get_memory_mib() -> u64 {
    match fs::read_to_string("/proc/meminfo").await {
        Ok(content) => {
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("MemTotal:") {
                    // Line format: "MemTotal:       8192000 kB"
                    let kib: u64 = rest
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    return kib / 1024;
                }
            }
            warn!("Could not parse MemTotal from /proc/meminfo");
            0
        }
        Err(e) => {
            warn!("Failed to read /proc/meminfo: {}", e);
            0
        }
    }
}

/// Allocate and format a swap file at `path` with the given size in MiB.
async fn create_swap_file(path: &str, size_mib: u64) -> Result<(), CloudInitError> {
    debug!("Creating swap file: {} ({} MiB)", path, size_mib);

    let size_bytes = size_mib.saturating_mul(1024 * 1024);

    // Prefer fallocate (instant, no actual data write).
    let fallocate_ok = tokio::process::Command::new("fallocate")
        .args(["-l", &size_bytes.to_string(), path])
        .output()
        .await
        .is_ok_and(|o| o.status.success());

    if !fallocate_ok {
        // Fallback: dd - slower but universally available.
        debug!("fallocate failed or unavailable, falling back to dd");
        let output = tokio::process::Command::new("dd")
            .args([
                "if=/dev/zero",
                &format!("of={path}"),
                "bs=1M",
                &format!("count={size_mib}"),
            ])
            .output()
            .await
            .map_err(|e| CloudInitError::Command(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CloudInitError::module(
                "mounts",
                format!("Failed to create swap file {path}: {stderr}"),
            ));
        }
    }

    // Restrict access: swap files must not be world-readable.
    set_file_permissions_600(path).await?;

    // Format the file as swap.
    let output = tokio::process::Command::new("mkswap")
        .arg(path)
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::module(
            "mounts",
            format!("mkswap failed for {path}: {stderr}"),
        ));
    }

    info!("Created swap file: {}", path);
    Ok(())
}

/// Set file permissions to `0600` (owner read/write only).
#[cfg(unix)]
async fn set_file_permissions_600(path: &str) -> Result<(), CloudInitError> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, perms)
        .await
        .map_err(CloudInitError::Io)
}

/// No-op on non-Unix platforms (cloud-init only targets Linux).
#[cfg(not(unix))]
async fn set_file_permissions_600(_path: &str) -> Result<(), CloudInitError> {
    Ok(())
}

// ==================== Unit Tests ====================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MountFieldValue;

    fn make_entry(fields: &[&str]) -> MountEntry {
        let vals = fields
            .iter()
            .map(|s| Some(MountFieldValue::Text(s.to_string())))
            .collect();
        MountEntry(vals)
    }

    fn make_entry_with_nulls(fields: &[Option<&str>]) -> MountEntry {
        let vals = fields
            .iter()
            .map(|opt| opt.map(|s| MountFieldValue::Text(s.to_string())))
            .collect();
        MountEntry(vals)
    }

    // ==================== resolve_mount ====================

    #[test]
    fn test_resolve_mount_full_fields() {
        let entry = make_entry(&["/dev/sda1", "/mnt", "ext4", "defaults", "0", "2"]);
        let resolved = resolve_mount(&entry, &[]).unwrap();
        assert_eq!(resolved.device, "/dev/sda1");
        assert_eq!(resolved.mount_point, "/mnt");
        assert_eq!(resolved.fs_type, "ext4");
        assert_eq!(resolved.options, "defaults");
        assert_eq!(resolved.dump, "0");
        assert_eq!(resolved.pass, "2");
    }

    #[test]
    fn test_resolve_mount_uses_builtin_defaults() {
        let entry = make_entry(&["/dev/sda1", "/mnt"]);
        let resolved = resolve_mount(&entry, &[]).unwrap();
        assert_eq!(resolved.fs_type, DEFAULT_FS_TYPE);
        assert_eq!(resolved.options, DEFAULT_OPTIONS);
        assert_eq!(resolved.dump, DEFAULT_DUMP);
        assert_eq!(resolved.pass, DEFAULT_PASS);
    }

    #[test]
    fn test_resolve_mount_uses_user_defaults() {
        let entry =
            make_entry_with_nulls(&[Some("/dev/sda1"), Some("/mnt"), None, None, None, None]);
        let defaults: Vec<Option<String>> = vec![
            None,
            None,
            Some("xfs".to_string()),
            Some("noatime".to_string()),
            Some("1".to_string()),
            Some("1".to_string()),
        ];
        let resolved = resolve_mount(&entry, &defaults).unwrap();
        assert_eq!(resolved.fs_type, "xfs");
        assert_eq!(resolved.options, "noatime");
        assert_eq!(resolved.dump, "1");
        assert_eq!(resolved.pass, "1");
    }

    #[test]
    fn test_resolve_mount_entry_overrides_user_defaults() {
        let entry = make_entry(&["/dev/sda1", "/mnt", "btrfs"]);
        let defaults: Vec<Option<String>> = vec![None, None, Some("xfs".to_string())];
        let resolved = resolve_mount(&entry, &defaults).unwrap();
        // Entry value "btrfs" should beat the user default "xfs".
        assert_eq!(resolved.fs_type, "btrfs");
    }

    #[test]
    fn test_resolve_mount_missing_required_fields_returns_none() {
        // Only one field → mount point missing → None.
        let entry = make_entry(&["/dev/sda1"]);
        assert!(resolve_mount(&entry, &[]).is_none());

        // Empty entry → None.
        let empty = MountEntry(vec![]);
        assert!(resolve_mount(&empty, &[]).is_none());
    }

    #[test]
    fn test_resolve_swap_entry() {
        let entry = make_entry(&["swap", "none", "swap", "sw", "0", "0"]);
        let resolved = resolve_mount(&entry, &[]).unwrap();
        assert!(resolved.is_swap());
    }

    // ==================== ResolvedMount::to_fstab_line ====================

    #[test]
    fn test_to_fstab_line_format() {
        let m = ResolvedMount {
            device: "/dev/sda1".to_string(),
            mount_point: "/mnt/data".to_string(),
            fs_type: "ext4".to_string(),
            options: "defaults".to_string(),
            dump: "0".to_string(),
            pass: "2".to_string(),
        };
        let line = m.to_fstab_line();
        assert_eq!(line, "/dev/sda1\t/mnt/data\text4\tdefaults\t0\t2");
    }

    // ==================== is_swap detection ====================

    #[test]
    fn test_is_swap_by_fstype() {
        let m = ResolvedMount {
            device: "/dev/sdb".to_string(),
            mount_point: "none".to_string(),
            fs_type: "swap".to_string(),
            options: "sw".to_string(),
            dump: "0".to_string(),
            pass: "0".to_string(),
        };
        assert!(m.is_swap());
    }

    #[test]
    fn test_is_not_swap() {
        let m = ResolvedMount {
            device: "/dev/sda1".to_string(),
            mount_point: "/mnt".to_string(),
            fs_type: "ext4".to_string(),
            options: "defaults".to_string(),
            dump: "0".to_string(),
            pass: "2".to_string(),
        };
        assert!(!m.is_swap());
    }

    // ==================== fstab update logic ====================

    #[test]
    fn test_fstab_update_adds_new_entry() {
        let existing = "# /etc/fstab\n/dev/sda1\t/\text4\tdefaults\t0\t1\n";
        let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();

        let new_entry = ResolvedMount {
            device: "/dev/sdb1".to_string(),
            mount_point: "/data".to_string(),
            fs_type: "xfs".to_string(),
            options: "defaults".to_string(),
            dump: "0".to_string(),
            pass: "2".to_string(),
        };

        // Simulate the update_fstab loop.
        let existing_idx = lines.iter().position(|line| {
            let t = line.trim();
            if t.starts_with('#') || t.is_empty() {
                return false;
            }
            t.split_whitespace()
                .next()
                .map(|d| d == new_entry.device)
                .unwrap_or(false)
        });
        if let Some(idx) = existing_idx {
            lines[idx] = new_entry.to_fstab_line();
        } else {
            lines.push(new_entry.to_fstab_line());
        }

        let content = lines.join("\n") + "\n";
        assert!(content.contains("/dev/sdb1\t/data\txfs\tdefaults\t0\t2"));
        assert!(content.contains("/dev/sda1")); // original entry preserved
    }

    #[test]
    fn test_fstab_update_replaces_existing_entry() {
        let existing = "# /etc/fstab\n/dev/sda1\t/\text4\tdefaults\t0\t1\n/dev/sdb1\t/data\txfs\tdefaults\t0\t2\n";
        let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();

        let updated_entry = ResolvedMount {
            device: "/dev/sdb1".to_string(),
            mount_point: "/data".to_string(),
            fs_type: "xfs".to_string(),
            options: "noatime,nodiratime".to_string(),
            dump: "0".to_string(),
            pass: "2".to_string(),
        };

        let existing_idx = lines.iter().position(|line| {
            let t = line.trim();
            if t.starts_with('#') || t.is_empty() {
                return false;
            }
            t.split_whitespace()
                .next()
                .map(|d| d == updated_entry.device)
                .unwrap_or(false)
        });
        if let Some(idx) = existing_idx {
            lines[idx] = updated_entry.to_fstab_line();
        } else {
            lines.push(updated_entry.to_fstab_line());
        }

        let content = lines.join("\n") + "\n";
        assert!(content.contains("noatime,nodiratime"));
        // Old "defaults" for /dev/sdb1 should be gone.
        let sdb1_lines: Vec<&str> = content
            .lines()
            .filter(|l| l.starts_with("/dev/sdb1"))
            .collect();
        assert_eq!(sdb1_lines.len(), 1);
    }

    // ==================== MountFieldValue integer coercion ====================

    #[test]
    fn test_mount_field_value_integer_to_string() {
        let v = MountFieldValue::Integer(0);
        assert_eq!(v.as_str_val(), "0");
        let v = MountFieldValue::Integer(2);
        assert_eq!(v.as_str_val(), "2");
    }

    #[test]
    fn test_mount_entry_fields_mixed() {
        // Simulate an entry where dump/pass are integers (as YAML would parse them).
        let entry = MountEntry(vec![
            Some(MountFieldValue::Text("/dev/sda1".to_string())),
            Some(MountFieldValue::Text("/mnt".to_string())),
            Some(MountFieldValue::Text("ext4".to_string())),
            Some(MountFieldValue::Text("defaults".to_string())),
            Some(MountFieldValue::Integer(0)),
            Some(MountFieldValue::Integer(2)),
        ]);
        let fields = entry.fields();
        assert_eq!(fields[4], Some("0".to_string()));
        assert_eq!(fields[5], Some("2".to_string()));
    }

    // ==================== get_memory_mib parsing ====================

    #[test]
    fn test_meminfo_line_parsing() {
        // Replicate the parsing logic from get_memory_mib.
        let content = "MemTotal:        8192000 kB\nMemFree:         4096000 kB\n";
        let mut result: u64 = 0;
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                let kib: u64 = rest
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                result = kib / 1024;
                break;
            }
        }
        assert_eq!(result, 8000); // 8192000 kB / 1024 = 8000 MiB
    }
}
