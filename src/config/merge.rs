//! Cloud-config merging
//!
//! Implements merging of multiple cloud-config sources with proper precedence:
//! 1. /etc/cloud/cloud.cfg (base)
//! 2. /etc/cloud/cloud.cfg.d/*.cfg (sorted alphabetically)
//! 3. Vendor-data
//! 4. User-data (highest priority)

use super::CloudConfig;
use serde_yaml::Value;
use tracing::debug;

/// Merge strategy for list fields
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListMergeStrategy {
    /// Append new items to existing list
    #[default]
    Append,
    /// Prepend new items to existing list
    Prepend,
    /// Replace entire list
    Replace,
    /// No change (keep original)
    NoReplace,
}

impl ListMergeStrategy {
    /// Parse from string (as used in merge_how)
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "append" => Self::Append,
            "prepend" => Self::Prepend,
            "replace" => Self::Replace,
            "no_replace" | "noreplace" => Self::NoReplace,
            _ => Self::Append,
        }
    }
}

/// Merge two CloudConfig instances
///
/// The `overlay` config takes precedence over `base` for scalar values.
/// List values are merged according to the strategy (default: append).
pub fn merge_configs(base: &CloudConfig, overlay: &CloudConfig) -> CloudConfig {
    // Convert both to YAML values for flexible merging
    let base_yaml = serde_yaml::to_value(base).unwrap_or(Value::Null);
    let overlay_yaml = serde_yaml::to_value(overlay).unwrap_or(Value::Null);

    // Merge the YAML values
    let merged = merge_yaml_values(&base_yaml, &overlay_yaml, ListMergeStrategy::Append);

    // Convert back to CloudConfig
    serde_yaml::from_value(merged).unwrap_or_default()
}

/// Merge two YAML values recursively
pub fn merge_yaml_values(base: &Value, overlay: &Value, list_strategy: ListMergeStrategy) -> Value {
    match (base, overlay) {
        // Both are mappings - merge recursively
        (Value::Mapping(base_map), Value::Mapping(overlay_map)) => {
            let mut result = base_map.clone();

            for (key, overlay_value) in overlay_map {
                if let Some(base_value) = result.get(key) {
                    // Key exists in both - merge recursively
                    let merged = merge_yaml_values(base_value, overlay_value, list_strategy);
                    result.insert(key.clone(), merged);
                } else {
                    // Key only in overlay - add it
                    result.insert(key.clone(), overlay_value.clone());
                }
            }

            Value::Mapping(result)
        }

        // Both are sequences - merge according to strategy
        (Value::Sequence(base_seq), Value::Sequence(overlay_seq)) => match list_strategy {
            ListMergeStrategy::Append => {
                let mut result = base_seq.clone();
                for item in overlay_seq {
                    if !result.contains(item) {
                        result.push(item.clone());
                    }
                }
                Value::Sequence(result)
            }
            ListMergeStrategy::Prepend => {
                let mut result = overlay_seq.clone();
                for item in base_seq {
                    if !result.contains(item) {
                        result.push(item.clone());
                    }
                }
                Value::Sequence(result)
            }
            ListMergeStrategy::Replace => Value::Sequence(overlay_seq.clone()),
            ListMergeStrategy::NoReplace => Value::Sequence(base_seq.clone()),
        },

        // Overlay is null - keep base value
        (base_value, Value::Null) => base_value.clone(),

        // All other cases - overlay wins
        (_, overlay_value) => overlay_value.clone(),
    }
}

/// Merge multiple CloudConfig instances in order (later configs have higher priority)
pub fn merge_all_configs(configs: &[CloudConfig]) -> CloudConfig {
    if configs.is_empty() {
        return CloudConfig::default();
    }

    let mut result = configs[0].clone();
    for config in configs.iter().skip(1) {
        debug!("Merging cloud-config");
        result = merge_configs(&result, config);
    }
    result
}

/// Merge multiple YAML strings into a single CloudConfig
pub fn merge_yaml_strings(yaml_strings: &[String]) -> Result<CloudConfig, serde_yaml::Error> {
    let configs: Result<Vec<CloudConfig>, _> = yaml_strings
        .iter()
        .map(|s| CloudConfig::from_yaml(s))
        .collect();

    Ok(merge_all_configs(&configs?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_hostname() {
        let base = CloudConfig::from_yaml("#cloud-config\nhostname: base-host").unwrap();
        let overlay = CloudConfig::from_yaml("#cloud-config\nhostname: overlay-host").unwrap();

        let merged = merge_configs(&base, &overlay);
        assert_eq!(merged.hostname, Some("overlay-host".to_string()));
    }

    #[test]
    fn test_merge_keeps_base_when_overlay_missing() {
        let base =
            CloudConfig::from_yaml("#cloud-config\nhostname: base-host\ntimezone: UTC").unwrap();
        let overlay = CloudConfig::from_yaml("#cloud-config\nlocale: en_US.UTF-8").unwrap();

        let merged = merge_configs(&base, &overlay);
        assert_eq!(merged.hostname, Some("base-host".to_string()));
        assert_eq!(merged.timezone, Some("UTC".to_string()));
        assert_eq!(merged.locale, Some("en_US.UTF-8".to_string()));
    }

    #[test]
    fn test_merge_packages_append() {
        let base = CloudConfig::from_yaml("#cloud-config\npackages:\n  - nginx\n  - vim").unwrap();
        let overlay =
            CloudConfig::from_yaml("#cloud-config\npackages:\n  - htop\n  - curl").unwrap();

        let merged = merge_configs(&base, &overlay);
        assert_eq!(merged.packages.len(), 4);
        assert!(merged.packages.contains(&"nginx".to_string()));
        assert!(merged.packages.contains(&"htop".to_string()));
    }

    #[test]
    fn test_merge_packages_no_duplicates() {
        let base = CloudConfig::from_yaml("#cloud-config\npackages:\n  - nginx\n  - vim").unwrap();
        let overlay =
            CloudConfig::from_yaml("#cloud-config\npackages:\n  - nginx\n  - curl").unwrap();

        let merged = merge_configs(&base, &overlay);
        // nginx should not be duplicated
        let nginx_count = merged.packages.iter().filter(|&p| p == "nginx").count();
        assert_eq!(nginx_count, 1);
    }

    #[test]
    fn test_merge_runcmd_append() {
        let base = CloudConfig::from_yaml("#cloud-config\nruncmd:\n  - echo base").unwrap();
        let overlay = CloudConfig::from_yaml("#cloud-config\nruncmd:\n  - echo overlay").unwrap();

        let merged = merge_configs(&base, &overlay);
        assert_eq!(merged.runcmd.len(), 2);
    }

    #[test]
    fn test_merge_all_configs() {
        let configs = vec![
            CloudConfig::from_yaml("#cloud-config\nhostname: first").unwrap(),
            CloudConfig::from_yaml("#cloud-config\nhostname: second\nlocale: en_US").unwrap(),
            CloudConfig::from_yaml("#cloud-config\nhostname: third").unwrap(),
        ];

        let merged = merge_all_configs(&configs);
        assert_eq!(merged.hostname, Some("third".to_string()));
        assert_eq!(merged.locale, Some("en_US".to_string()));
    }

    #[test]
    fn test_merge_yaml_strings() {
        let strings = vec![
            "#cloud-config\nhostname: test\npackages:\n  - nginx".to_string(),
            "#cloud-config\npackages:\n  - vim".to_string(),
        ];

        let merged = merge_yaml_strings(&strings).unwrap();
        assert_eq!(merged.hostname, Some("test".to_string()));
        assert_eq!(merged.packages.len(), 2);
    }

    #[test]
    fn test_list_merge_strategy_parse() {
        assert_eq!(
            ListMergeStrategy::parse("append"),
            ListMergeStrategy::Append
        );
        assert_eq!(
            ListMergeStrategy::parse("PREPEND"),
            ListMergeStrategy::Prepend
        );
        assert_eq!(
            ListMergeStrategy::parse("replace"),
            ListMergeStrategy::Replace
        );
        assert_eq!(
            ListMergeStrategy::parse("no_replace"),
            ListMergeStrategy::NoReplace
        );
    }

    #[test]
    fn test_merge_yaml_values_replace() {
        let base = serde_yaml::from_str::<Value>("[1, 2, 3]").unwrap();
        let overlay = serde_yaml::from_str::<Value>("[4, 5]").unwrap();

        let merged = merge_yaml_values(&base, &overlay, ListMergeStrategy::Replace);
        let seq = merged.as_sequence().unwrap();
        assert_eq!(seq.len(), 2);
    }

    #[test]
    fn test_merge_empty_configs() {
        let empty: Vec<CloudConfig> = vec![];
        let merged = merge_all_configs(&empty);
        assert!(merged.hostname.is_none());
    }

    #[test]
    fn test_merge_write_files() {
        let base = CloudConfig::from_yaml(
            r#"#cloud-config
write_files:
  - path: /etc/file1
    content: content1
"#,
        )
        .unwrap();
        let overlay = CloudConfig::from_yaml(
            r#"#cloud-config
write_files:
  - path: /etc/file2
    content: content2
"#,
        )
        .unwrap();

        let merged = merge_configs(&base, &overlay);
        assert_eq!(merged.write_files.len(), 2);
    }
}
