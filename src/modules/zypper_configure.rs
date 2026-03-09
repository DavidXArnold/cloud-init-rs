//! Zypper configuration module (SUSE/openSUSE)
//!
//! Applies key-value pairs from the `zypper.config` cloud-config section to
//! `/etc/zypp/zypp.conf`, which uses a simple INI-style format.

use crate::CloudInitError;
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Path to the zypper configuration file
const ZYPP_CONF: &str = "/etc/zypp/zypp.conf";

/// Apply zypper configuration settings to `/etc/zypp/zypp.conf`.
///
/// Each entry in `settings` is written as `key = value` under the `[main]`
/// section.  Existing keys are updated in place; new keys are appended.
pub async fn configure_zypper(
    settings: &HashMap<String, serde_yaml::Value>,
) -> Result<(), CloudInitError> {
    if settings.is_empty() {
        return Ok(());
    }

    info!(
        "Applying {} zypper configuration setting(s) to {}",
        settings.len(),
        ZYPP_CONF
    );

    // Read existing file, or start with an empty string if it doesn't exist yet
    let existing = tokio::fs::read_to_string(ZYPP_CONF)
        .await
        .unwrap_or_default();

    let updated = apply_settings(&existing, settings);

    tokio::fs::write(ZYPP_CONF, updated.as_bytes())
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                CloudInitError::Permission(format!("Cannot write {}: {}", ZYPP_CONF, e))
            } else {
                CloudInitError::Io(e)
            }
        })?;

    debug!("Wrote zypper configuration to {}", ZYPP_CONF);
    Ok(())
}

/// Convert a `serde_yaml::Value` scalar to a string suitable for zypp.conf.
fn yaml_value_to_string(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Null => String::new(),
        other => {
            warn!(
                "Unsupported zypper config value type; using raw YAML: {:?}",
                other
            );
            format!("{other:?}")
        }
    }
}

/// Apply `settings` to the text content of zypp.conf, returning the updated
/// content.  This preserves comments and the overall structure of the file.
fn apply_settings(content: &str, settings: &HashMap<String, serde_yaml::Value>) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut remaining: HashMap<&str, String> = settings
        .iter()
        .map(|(k, v)| (k.as_str(), yaml_value_to_string(v)))
        .collect();

    // Update existing keys in-place
    for line in &mut lines {
        let trimmed = line.trim();
        // Skip comments and blank lines
        if trimmed.starts_with('#') || trimmed.starts_with(';') || trimmed.is_empty() {
            continue;
        }
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim();
            if let Some(new_val) = remaining.remove(key) {
                *line = format!("{} = {}", key, new_val);
            }
        }
    }

    // Append keys that were not already present
    if !remaining.is_empty() {
        // Ensure there is a [main] section header if the file was empty
        if lines.is_empty() || !lines.iter().any(|l| l.trim() == "[main]") {
            lines.push("[main]".to_string());
        }
        for (key, val) in &remaining {
            lines.push(format!("{} = {}", key, val));
        }
    }

    let mut result = lines.join("\n");
    // Preserve trailing newline
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml::Value;

    fn string_val(s: &str) -> Value {
        Value::String(s.to_string())
    }

    #[test]
    fn test_apply_settings_updates_existing_key() {
        let content = "[main]\ngpgcheck = 0\n";
        let mut settings = HashMap::new();
        settings.insert("gpgcheck".to_string(), string_val("1"));
        let result = apply_settings(content, &settings);
        assert!(result.contains("gpgcheck = 1"));
        assert!(!result.contains("gpgcheck = 0"));
    }

    #[test]
    fn test_apply_settings_appends_new_key() {
        let content = "[main]\ngpgcheck = 1\n";
        let mut settings = HashMap::new();
        settings.insert("solver.onlyRequires".to_string(), string_val("true"));
        let result = apply_settings(content, &settings);
        assert!(result.contains("solver.onlyRequires = true"));
        // original key preserved
        assert!(result.contains("gpgcheck = 1"));
    }

    #[test]
    fn test_apply_settings_creates_section_on_empty_file() {
        let content = "";
        let mut settings = HashMap::new();
        settings.insert("gpgcheck".to_string(), string_val("1"));
        let result = apply_settings(content, &settings);
        assert!(result.contains("[main]"));
        assert!(result.contains("gpgcheck = 1"));
    }

    #[test]
    fn test_apply_settings_preserves_comments() {
        let content = "# zypper config\n[main]\n# check gpg\ngpgcheck = 0\n";
        let mut settings = HashMap::new();
        settings.insert("gpgcheck".to_string(), string_val("1"));
        let result = apply_settings(content, &settings);
        assert!(result.contains("# zypper config"));
        assert!(result.contains("# check gpg"));
        assert!(result.contains("gpgcheck = 1"));
    }

    #[test]
    fn test_yaml_value_to_string_bool() {
        assert_eq!(yaml_value_to_string(&Value::Bool(true)), "true");
        assert_eq!(yaml_value_to_string(&Value::Bool(false)), "false");
    }

    #[test]
    fn test_yaml_value_to_string_number() {
        assert_eq!(
            yaml_value_to_string(&Value::Number(serde_yaml::Number::from(1))),
            "1"
        );
    }

    #[test]
    fn test_yaml_value_to_string_string() {
        assert_eq!(yaml_value_to_string(&string_val("hello")), "hello");
    }
}
