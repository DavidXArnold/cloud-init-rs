//! Cloud-config parsing and types
//!
//! Handles parsing of cloud-config YAML format used by cloud-init.

use serde::{Deserialize, Serialize};

/// Main cloud-config structure
///
/// Represents the parsed cloud-config YAML that begins with `#cloud-config`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CloudConfig {
    /// Hostname to set
    pub hostname: Option<String>,

    /// Fully qualified domain name
    pub fqdn: Option<String>,

    /// Whether to manage /etc/hosts
    pub manage_etc_hosts: Option<bool>,

    /// Users to create
    #[serde(default)]
    pub users: Vec<UserConfig>,

    /// Groups to create
    #[serde(default)]
    pub groups: Vec<GroupConfig>,

    /// Files to write
    #[serde(default)]
    pub write_files: Vec<WriteFileConfig>,

    /// Commands to run
    #[serde(default)]
    pub runcmd: Vec<RunCmd>,

    /// Packages to install
    #[serde(default)]
    pub packages: Vec<String>,

    /// Whether to upgrade packages
    pub package_upgrade: Option<bool>,

    /// Package update on first boot
    pub package_update: Option<bool>,

    /// SSH configuration
    pub ssh: Option<SshConfig>,

    /// SSH authorized keys for default user
    #[serde(default)]
    pub ssh_authorized_keys: Vec<String>,

    /// Timezone to set
    pub timezone: Option<String>,

    /// Locale to set
    pub locale: Option<String>,

    /// Growpart configuration
    pub growpart: Option<GrowpartConfig>,

    /// Resize rootfs configuration
    pub resize_rootfs: Option<bool>,

    /// Phone home configuration
    pub phone_home: Option<PhoneHomeConfig>,

    /// Final message template
    pub final_message: Option<String>,
}

/// User configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserConfig {
    /// Simple user name
    Name(String),
    /// Full user configuration
    Full(UserFullConfig),
}

/// Full user configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UserFullConfig {
    pub name: String,
    pub gecos: Option<String>,
    pub homedir: Option<String>,
    pub primary_group: Option<String>,
    #[serde(default)]
    pub groups: Vec<String>,
    pub shell: Option<String>,
    pub sudo: Option<String>,
    pub lock_passwd: Option<bool>,
    pub passwd: Option<String>,
    #[serde(default)]
    pub ssh_authorized_keys: Vec<String>,
    pub ssh_import_id: Option<Vec<String>>,
    pub system: Option<bool>,
    pub uid: Option<u32>,
}

/// Group configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GroupConfig {
    /// Simple group name
    Name(String),
    /// Group with members
    WithMembers { name: String, members: Vec<String> },
}

/// File to write
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteFileConfig {
    pub path: String,
    #[serde(default)]
    pub content: String,
    pub encoding: Option<String>,
    pub owner: Option<String>,
    pub permissions: Option<String>,
    pub append: Option<bool>,
    pub defer: Option<bool>,
}

/// Command to run (can be string or list of args)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RunCmd {
    /// Shell command as string
    Shell(String),
    /// Command with arguments
    Args(Vec<String>),
}

/// SSH configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SshConfig {
    pub emit_keys_to_console: Option<bool>,
    #[serde(default)]
    pub ssh_authorized_keys: Vec<String>,
}

/// Growpart configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrowpartConfig {
    pub mode: Option<String>,
    pub devices: Option<Vec<String>>,
    pub ignore_growroot_disabled: Option<bool>,
}

/// Phone home configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoneHomeConfig {
    pub url: String,
    pub post: Option<Vec<String>>,
    pub tries: Option<u32>,
}

impl CloudConfig {
    /// Parse cloud-config from YAML string
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        // Strip #cloud-config header if present
        let yaml = yaml
            .strip_prefix("#cloud-config")
            .map(|s| s.trim_start())
            .unwrap_or(yaml);

        serde_yaml::from_str(yaml)
    }

    /// Check if this looks like a cloud-config (starts with #cloud-config)
    pub fn is_cloud_config(data: &str) -> bool {
        data.trim_start().starts_with("#cloud-config")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_cloud_config() {
        let yaml = r#"
#cloud-config
hostname: test-instance
packages:
  - nginx
  - vim
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.hostname, Some("test-instance".to_string()));
        assert_eq!(config.packages, vec!["nginx", "vim"]);
    }

    #[test]
    fn test_is_cloud_config() {
        assert!(CloudConfig::is_cloud_config("#cloud-config\nhostname: test"));
        assert!(CloudConfig::is_cloud_config("  #cloud-config\n"));
        assert!(!CloudConfig::is_cloud_config("#!/bin/bash\necho hello"));
    }
}
