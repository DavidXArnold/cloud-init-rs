//! Disk setup module
//!
//! Partitions disks according to `disk_setup` cloud-config directives.
//!
//! Supports:
//! - Creating MBR (dos) and GPT partition tables
//! - Creating partitions with size percentages and type codes
//! - Device path resolution (direct path, LABEL=, UUID=)
//! - Safety guard: skips already-partitioned disks unless `overwrite: true`

use crate::CloudInitError;
use crate::config::{DiskConfig, PartitionLayout, PartitionSpec};
use std::collections::HashMap;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

/// Set up all disks according to the `disk_setup` configuration.
///
/// Failures on individual disks are logged as warnings and do not abort
/// the overall operation — matching the resilient behaviour of other modules.
pub async fn setup_disks(disk_setup: &HashMap<String, DiskConfig>) -> Result<(), CloudInitError> {
    if disk_setup.is_empty() {
        return Ok(());
    }

    info!("Setting up {} disk(s)", disk_setup.len());

    for (device, config) in disk_setup {
        info!("Processing disk setup for: {}", device);
        if let Err(e) = setup_disk(device, config).await {
            warn!("Failed to setup disk {}: {}", device, e);
        }
    }

    Ok(())
}

/// Set up a single disk.
pub async fn setup_disk(device: &str, config: &DiskConfig) -> Result<(), CloudInitError> {
    // Resolve the device specifier to a concrete path.
    let device_path = resolve_device(device).await?;
    debug!("Resolved device '{}' to '{}'", device, device_path);

    // Safety guard: do not touch disks that are already partitioned unless the
    // caller explicitly requests an overwrite.
    let overwrite = config.overwrite.unwrap_or(false);
    if !overwrite && has_partition_table(&device_path).await? {
        info!(
            "Disk {} already has a partition table and overwrite=false, skipping",
            device_path
        );
        return Ok(());
    }

    // Normalise the table type (default: gpt).
    let table_type = normalize_table_type(config.table_type.as_deref().unwrap_or("gpt"))?;

    info!(
        "Creating {} partition table on {}",
        table_type, device_path
    );

    let script = build_sfdisk_script(table_type, &config.layout);
    debug!("sfdisk script for {}:\n{}", device_path, script);

    run_sfdisk(&device_path, &script).await?;

    info!("Successfully partitioned {}", device_path);
    Ok(())
}

/// Resolve a device specifier to the actual `/dev/…` path.
///
/// Recognised formats:
/// - `/dev/sda` — used as-is if the path exists
/// - `LABEL=boot` — resolved via `findfs`
/// - `UUID=xxxxxxxx-…` — resolved via `findfs`
pub async fn resolve_device(device: &str) -> Result<String, CloudInitError> {
    if let Some(label) = device.strip_prefix("LABEL=") {
        return resolve_by_attribute("LABEL", label).await;
    }

    if let Some(uuid) = device.strip_prefix("UUID=") {
        return resolve_by_attribute("UUID", uuid).await;
    }

    // Direct path — verify existence.
    if std::path::Path::new(device).exists() {
        return Ok(device.to_string());
    }

    Err(CloudInitError::module(
        "disk_setup",
        format!("Device not found: {}", device),
    ))
}

/// Resolve a device by a named attribute using `findfs`.
async fn resolve_by_attribute(attr: &str, value: &str) -> Result<String, CloudInitError> {
    let output = tokio::process::Command::new("findfs")
        .arg(format!("{}={}", attr, value))
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(path);
        }
    }

    Err(CloudInitError::module(
        "disk_setup",
        format!("Could not find device with {}={}", attr, value),
    ))
}

/// Return `true` when the device already contains a recognisable partition table.
pub async fn has_partition_table(device: &str) -> Result<bool, CloudInitError> {
    // `blkid -p` probes the device without using cached data.
    let output = tokio::process::Command::new("blkid")
        .args(["-p", "-s", "PTTYPE", device])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("PTTYPE=") {
            return Ok(true);
        }
    }

    // Fallback: sfdisk --dump succeeds only when a valid partition table exists.
    let output = tokio::process::Command::new("sfdisk")
        .args(["--dump", device])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    Ok(output.status.success())
}

/// Normalise a table-type string to `"gpt"` or `"dos"`.
pub fn normalize_table_type(table_type: &str) -> Result<&'static str, CloudInitError> {
    match table_type.to_lowercase().as_str() {
        "gpt" => Ok("gpt"),
        "mbr" | "msdos" | "dos" => Ok("dos"),
        _ => Err(CloudInitError::module(
            "disk_setup",
            format!("Unknown partition table type: '{}'", table_type),
        )),
    }
}

/// Map a numeric partition type code to the string `sfdisk` expects.
///
/// For GPT labels, common MBR type codes are mapped to their GPT equivalents.
/// For MBR (dos) labels, the hex string of the numeric code is used directly.
pub fn partition_type_for_sfdisk(table_type: &str, type_code: Option<u32>) -> String {
    match table_type {
        "gpt" => match type_code {
            Some(82) => "linux-swap".to_string(),
            Some(0x8e) => "linux-lvm".to_string(),
            Some(0xfd) => "linux-raid".to_string(),
            Some(0xef) => "uefi".to_string(),
            _ => "linux".to_string(),
        },
        _ => match type_code {
            // sfdisk interprets MBR type codes as hex strings, so values the user writes
            // in YAML (e.g. 82 for Linux swap, 83 for Linux) are passed through directly.
            Some(code) => format!("{}", code),
            None => "83".to_string(), // Linux filesystem
        },
    }
}

/// Build the `sfdisk` input script for the given table type and layout.
pub fn build_sfdisk_script(table_type: &str, layout: &Option<PartitionLayout>) -> String {
    let mut script = format!("label: {}\n\n", table_type);

    match layout {
        // No layout specified, or explicitly no partitions — emit only the label line.
        None | Some(PartitionLayout::Simple(false)) => {}

        // Single partition consuming the entire disk.
        Some(PartitionLayout::Simple(true)) => {
            let part_type = partition_type_for_sfdisk(table_type, None);
            script.push_str(&format!("size=+, type={}\n", part_type));
        }

        Some(PartitionLayout::Partitions(partitions)) if partitions.is_empty() => {}

        // Explicit partition list.
        Some(PartitionLayout::Partitions(partitions)) => {
            let last_idx = partitions.len() - 1;
            for (idx, spec) in partitions.iter().enumerate() {
                let (size_pct, type_code) = match spec {
                    PartitionSpec::Size(pct) => (*pct, None),
                    PartitionSpec::SizeAndType(parts) => {
                        let size = parts.first().copied().unwrap_or(0);
                        let type_code = parts.get(1).copied();
                        (size, type_code)
                    }
                };

                let part_type = partition_type_for_sfdisk(table_type, type_code);

                // The last partition takes up all remaining space so that rounding
                // errors in percentage arithmetic do not leave unused sectors.
                if idx == last_idx {
                    script.push_str(&format!("size=+, type={}\n", part_type));
                } else {
                    script.push_str(&format!("size={}%, type={}\n", size_pct, part_type));
                }
            }
        }
    }

    script
}

/// Pipe `script` into `sfdisk <device>` and wait for it to complete.
async fn run_sfdisk(device: &str, script: &str) -> Result<(), CloudInitError> {
    use std::process::Stdio;

    let mut child = tokio::process::Command::new("sfdisk")
        .arg(device)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            CloudInitError::Command(format!("Failed to spawn sfdisk for {}: {}", device, e))
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(script.as_bytes())
            .await
            .map_err(|e| CloudInitError::Command(format!("Failed to write to sfdisk: {}", e)))?;
    }

    let output = child.wait_with_output().await.map_err(|e| {
        CloudInitError::Command(format!("Failed to wait for sfdisk on {}: {}", device, e))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::module(
            "disk_setup",
            format!("sfdisk failed on {}: {}", device, stderr),
        ));
    }

    // Best-effort: inform the kernel about the updated partition table.
    let _ = tokio::process::Command::new("partprobe")
        .arg(device)
        .output()
        .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DiskConfig, PartitionLayout, PartitionSpec};

    // ── normalize_table_type ─────────────────────────────────────────────────

    #[test]
    fn test_normalize_gpt() {
        assert_eq!(normalize_table_type("gpt").unwrap(), "gpt");
        assert_eq!(normalize_table_type("GPT").unwrap(), "gpt");
    }

    #[test]
    fn test_normalize_mbr_variants() {
        for input in &["mbr", "MBR", "msdos", "MSDOS", "dos", "DOS"] {
            assert_eq!(normalize_table_type(input).unwrap(), "dos");
        }
    }

    #[test]
    fn test_normalize_unknown_type() {
        assert!(normalize_table_type("zfs").is_err());
        assert!(normalize_table_type("").is_err());
    }

    // ── partition_type_for_sfdisk ────────────────────────────────────────────

    #[test]
    fn test_gpt_default_type() {
        assert_eq!(partition_type_for_sfdisk("gpt", None), "linux");
        assert_eq!(partition_type_for_sfdisk("gpt", Some(83)), "linux");
    }

    #[test]
    fn test_gpt_swap_type() {
        assert_eq!(partition_type_for_sfdisk("gpt", Some(82)), "linux-swap");
    }

    #[test]
    fn test_gpt_lvm_type() {
        assert_eq!(partition_type_for_sfdisk("gpt", Some(0x8e)), "linux-lvm");
    }

    #[test]
    fn test_gpt_raid_type() {
        assert_eq!(partition_type_for_sfdisk("gpt", Some(0xfd)), "linux-raid");
    }

    #[test]
    fn test_gpt_uefi_type() {
        assert_eq!(partition_type_for_sfdisk("gpt", Some(0xef)), "uefi");
    }

    #[test]
    fn test_dos_default_type() {
        assert_eq!(partition_type_for_sfdisk("dos", None), "83");
    }

    #[test]
    fn test_dos_swap_type() {
        // User writes 82 in YAML (the conventional hex code for Linux swap).
        // sfdisk also expects "82" (interpreted as hex 0x82), so we pass through as-is.
        assert_eq!(partition_type_for_sfdisk("dos", Some(82)), "82");
    }

    #[test]
    fn test_dos_custom_type() {
        // 0x8e decimal = 142; users who want LVM on MBR would supply 142 in YAML.
        assert_eq!(partition_type_for_sfdisk("dos", Some(142)), "142");
    }

    // ── build_sfdisk_script ──────────────────────────────────────────────────

    #[test]
    fn test_script_no_layout() {
        let script = build_sfdisk_script("gpt", &None);
        assert!(script.starts_with("label: gpt\n"));
        // No partition lines should be present.
        assert!(!script.contains("size="));
    }

    #[test]
    fn test_script_layout_false() {
        let script = build_sfdisk_script("gpt", &Some(PartitionLayout::Simple(false)));
        assert!(script.starts_with("label: gpt\n"));
        assert!(!script.contains("size="));
    }

    #[test]
    fn test_script_layout_true_gpt() {
        let script = build_sfdisk_script("gpt", &Some(PartitionLayout::Simple(true)));
        assert!(script.contains("label: gpt"));
        assert!(script.contains("size=+, type=linux"));
    }

    #[test]
    fn test_script_layout_true_dos() {
        let script = build_sfdisk_script("dos", &Some(PartitionLayout::Simple(true)));
        assert!(script.contains("label: dos"));
        assert!(script.contains("size=+, type=83"));
    }

    #[test]
    fn test_script_single_partition() {
        let layout = Some(PartitionLayout::Partitions(vec![PartitionSpec::Size(100)]));
        let script = build_sfdisk_script("gpt", &layout);
        // Only one partition; it should use size=+ (last-partition rule).
        assert!(script.contains("size=+, type=linux"));
        assert!(!script.contains("size=100%"));
    }

    #[test]
    fn test_script_multiple_partitions() {
        let layout = Some(PartitionLayout::Partitions(vec![
            PartitionSpec::Size(25),
            PartitionSpec::SizeAndType(vec![25, 82]),
            PartitionSpec::Size(50),
        ]));
        let script = build_sfdisk_script("gpt", &layout);

        assert!(script.contains("label: gpt"));
        assert!(script.contains("size=25%, type=linux"));
        assert!(script.contains("size=25%, type=linux-swap"));
        // Last partition always gets size=+.
        assert!(script.contains("size=+, type=linux"));
        // The explicit 50% should NOT appear; the last partition uses +.
        assert!(!script.contains("size=50%"));
    }

    #[test]
    fn test_script_mbr_with_type_codes() {
        let layout = Some(PartitionLayout::Partitions(vec![
            PartitionSpec::SizeAndType(vec![50, 83]),
            PartitionSpec::SizeAndType(vec![50, 82]),
        ]));
        let script = build_sfdisk_script("dos", &layout);
        // sfdisk interprets these as hex: 83 → 0x83 = Linux, 82 → 0x82 = Linux swap.
        assert!(script.contains("size=50%, type=83"));
        assert!(script.contains("size=+, type=82"));
    }

    #[test]
    fn test_script_empty_partition_list() {
        let layout = Some(PartitionLayout::Partitions(vec![]));
        let script = build_sfdisk_script("gpt", &layout);
        assert!(script.starts_with("label: gpt\n"));
        assert!(!script.contains("size="));
    }

    // ── DiskConfig serialisation / deserialisation ───────────────────────────

    #[test]
    fn test_disk_config_defaults() {
        let config = DiskConfig {
            table_type: None,
            layout: None,
            overwrite: None,
        };
        assert!(config.table_type.is_none());
        assert!(config.overwrite.is_none());
    }

    #[test]
    fn test_partition_spec_size_only() {
        let spec = PartitionSpec::Size(50);
        match spec {
            PartitionSpec::Size(pct) => assert_eq!(pct, 50),
            _ => panic!("Expected Size variant"),
        }
    }

    #[test]
    fn test_partition_spec_size_and_type() {
        let spec = PartitionSpec::SizeAndType(vec![25, 82]);
        match spec {
            PartitionSpec::SizeAndType(parts) => {
                assert_eq!(parts[0], 25);
                assert_eq!(parts[1], 82);
            }
            _ => panic!("Expected SizeAndType variant"),
        }
    }
}
