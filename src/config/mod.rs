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
    Full(Box<UserFullConfig>),
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

    // ==================== Basic Parsing Tests ====================

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
        assert!(CloudConfig::is_cloud_config(
            "#cloud-config\nhostname: test"
        ));
        assert!(CloudConfig::is_cloud_config("  #cloud-config\n"));
        assert!(!CloudConfig::is_cloud_config("#!/bin/bash\necho hello"));
        assert!(!CloudConfig::is_cloud_config(""));
        assert!(!CloudConfig::is_cloud_config("hostname: test"));
    }

    #[test]
    fn test_parse_without_header() {
        let yaml = "hostname: test-instance\nlocale: en_US.UTF-8";
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.hostname, Some("test-instance".to_string()));
        assert_eq!(config.locale, Some("en_US.UTF-8".to_string()));
    }

    #[test]
    fn test_parse_empty_config() {
        let yaml = "#cloud-config\n";
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert!(config.hostname.is_none());
        assert!(config.packages.is_empty());
    }

    #[test]
    fn test_parse_comments_only() {
        let yaml = "#cloud-config\n# comment\n# another comment";
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert!(config.hostname.is_none());
    }

    // ==================== System Configuration Tests ====================

    #[test]
    fn test_parse_hostname_config() {
        let yaml = r#"
#cloud-config
hostname: my-server
fqdn: my-server.example.com
manage_etc_hosts: true
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.hostname, Some("my-server".to_string()));
        assert_eq!(config.fqdn, Some("my-server.example.com".to_string()));
        assert_eq!(config.manage_etc_hosts, Some(true));
    }

    #[test]
    fn test_parse_timezone_locale() {
        let yaml = r#"
#cloud-config
timezone: America/New_York
locale: en_US.UTF-8
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.timezone, Some("America/New_York".to_string()));
        assert_eq!(config.locale, Some("en_US.UTF-8".to_string()));
    }

    // ==================== User Configuration Tests ====================

    #[test]
    fn test_parse_simple_user() {
        let yaml = r#"
#cloud-config
users:
  - testuser
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.users.len(), 1);
        match &config.users[0] {
            UserConfig::Name(name) => assert_eq!(name, "testuser"),
            _ => panic!("Expected simple user name"),
        }
    }

    #[test]
    fn test_parse_full_user() {
        let yaml = r#"
#cloud-config
users:
  - name: deploy
    gecos: Deploy User
    shell: /bin/bash
    groups:
      - sudo
      - docker
    sudo: ALL=(ALL) NOPASSWD:ALL
    lock_passwd: true
    ssh_authorized_keys:
      - ssh-rsa AAAAB3... key1
      - ssh-ed25519 AAAAC3... key2
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.users.len(), 1);
        match &config.users[0] {
            UserConfig::Full(user) => {
                assert_eq!(user.name, "deploy");
                assert_eq!(user.gecos, Some("Deploy User".to_string()));
                assert_eq!(user.shell, Some("/bin/bash".to_string()));
                assert_eq!(user.groups, vec!["sudo", "docker"]);
                assert_eq!(user.sudo, Some("ALL=(ALL) NOPASSWD:ALL".to_string()));
                assert_eq!(user.lock_passwd, Some(true));
                assert_eq!(user.ssh_authorized_keys.len(), 2);
            }
            _ => panic!("Expected full user config"),
        }
    }

    #[test]
    fn test_parse_mixed_users() {
        let yaml = r#"
#cloud-config
users:
  - default
  - name: admin
    sudo: ALL=(ALL) NOPASSWD:ALL
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.users.len(), 2);
        assert!(matches!(&config.users[0], UserConfig::Name(n) if n == "default"));
        assert!(matches!(&config.users[1], UserConfig::Full(_)));
    }

    // ==================== Group Configuration Tests ====================

    #[test]
    fn test_parse_simple_group() {
        let yaml = r#"
#cloud-config
groups:
  - docker
  - admin
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.groups.len(), 2);
    }

    // ==================== Write Files Tests ====================

    #[test]
    fn test_parse_write_files() {
        let yaml = r#"
#cloud-config
write_files:
  - path: /etc/myconfig.yaml
    content: |
      key: value
    owner: root:root
    permissions: '0644'
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.write_files.len(), 1);
        let file = &config.write_files[0];
        assert_eq!(file.path, "/etc/myconfig.yaml");
        assert_eq!(file.owner, Some("root:root".to_string()));
        assert_eq!(file.permissions, Some("0644".to_string()));
        assert!(file.content.contains("key: value"));
    }

    #[test]
    fn test_parse_write_files_base64() {
        let yaml = r#"
#cloud-config
write_files:
  - path: /opt/script.sh
    content: IyEvYmluL2Jhc2gKZWNobyBoZWxsbw==
    encoding: base64
    permissions: '0755'
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        let file = &config.write_files[0];
        assert_eq!(file.encoding, Some("base64".to_string()));
    }

    #[test]
    fn test_parse_write_files_append() {
        let yaml = r#"
#cloud-config
write_files:
  - path: /etc/motd
    content: Welcome!
    append: true
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.write_files[0].append, Some(true));
    }

    #[test]
    fn test_parse_write_files_defer() {
        let yaml = r#"
#cloud-config
write_files:
  - path: /etc/late-config
    content: deferred content
    defer: true
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.write_files[0].defer, Some(true));
    }

    // ==================== Runcmd Tests ====================

    #[test]
    fn test_parse_runcmd_strings() {
        let yaml = r#"
#cloud-config
runcmd:
  - echo hello
  - apt-get update
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.runcmd.len(), 2);
        assert!(matches!(&config.runcmd[0], RunCmd::Shell(s) if s == "echo hello"));
    }

    #[test]
    fn test_parse_runcmd_arrays() {
        let yaml = r#"
#cloud-config
runcmd:
  - [mkdir, -p, /opt/myapp]
  - ["bash", "-c", "echo test"]
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.runcmd.len(), 2);
        match &config.runcmd[0] {
            RunCmd::Args(args) => {
                assert_eq!(args, &vec!["mkdir", "-p", "/opt/myapp"]);
            }
            _ => panic!("Expected args array"),
        }
    }

    #[test]
    fn test_parse_runcmd_mixed() {
        let yaml = r#"
#cloud-config
runcmd:
  - echo "shell command"
  - [docker, run, nginx]
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.runcmd.len(), 2);
        assert!(matches!(&config.runcmd[0], RunCmd::Shell(_)));
        assert!(matches!(&config.runcmd[1], RunCmd::Args(_)));
    }

    // ==================== Package Tests ====================

    #[test]
    fn test_parse_packages() {
        let yaml = r#"
#cloud-config
package_update: true
package_upgrade: false
packages:
  - nginx
  - vim
  - htop
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.package_update, Some(true));
        assert_eq!(config.package_upgrade, Some(false));
        assert_eq!(config.packages, vec!["nginx", "vim", "htop"]);
    }

    // ==================== SSH Configuration Tests ====================

    #[test]
    fn test_parse_ssh_keys() {
        let yaml = r#"
#cloud-config
ssh_authorized_keys:
  - ssh-rsa AAAAB3... admin@host
  - ssh-ed25519 AAAAC3... user@host
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.ssh_authorized_keys.len(), 2);
    }

    #[test]
    fn test_parse_ssh_config() {
        let yaml = r#"
#cloud-config
ssh:
  emit_keys_to_console: false
  ssh_authorized_keys:
    - ssh-rsa AAAAB3... key
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        let ssh = config.ssh.unwrap();
        assert_eq!(ssh.emit_keys_to_console, Some(false));
        assert_eq!(ssh.ssh_authorized_keys.len(), 1);
    }

    // ==================== Advanced Configuration Tests ====================

    #[test]
    fn test_parse_growpart() {
        let yaml = r#"
#cloud-config
growpart:
  mode: auto
  devices:
    - /
    - /dev/sda1
  ignore_growroot_disabled: false
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        let growpart = config.growpart.unwrap();
        assert_eq!(growpart.mode, Some("auto".to_string()));
        assert_eq!(growpart.devices.unwrap().len(), 2);
    }

    #[test]
    fn test_parse_resize_rootfs() {
        let yaml = r#"
#cloud-config
resize_rootfs: true
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.resize_rootfs, Some(true));
    }

    #[test]
    fn test_parse_phone_home() {
        let yaml = r#"
#cloud-config
phone_home:
  url: https://example.com/phone-home
  post:
    - instance_id
    - hostname
  tries: 10
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        let phone_home = config.phone_home.unwrap();
        assert_eq!(phone_home.url, "https://example.com/phone-home");
        assert_eq!(phone_home.tries, Some(10));
    }

    #[test]
    fn test_parse_final_message() {
        let yaml = r#"
#cloud-config
final_message: |
  Cloud-init completed!
  Hostname: $HOSTNAME
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert!(config
            .final_message
            .unwrap()
            .contains("Cloud-init completed"));
    }

    // ==================== Error Handling Tests ====================

    #[test]
    fn test_parse_malformed_yaml() {
        let yaml = "#cloud-config\nhostname: [invalid";
        let result = CloudConfig::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_wrong_type() {
        let yaml = r#"
#cloud-config
hostname:
  nested: value
"#;
        let result = CloudConfig::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unknown_fields_ignored() {
        let yaml = r#"
#cloud-config
hostname: test
unknown_field: should_be_ignored
another_unknown:
  - list
  - of
  - values
"#;
        // With default serde behavior, unknown fields are ignored
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.hostname, Some("test".to_string()));
    }

    // ==================== Full Config Tests ====================

    #[test]
    fn test_parse_full_config() {
        let yaml = r#"
#cloud-config
hostname: production-server
fqdn: production-server.example.com
manage_etc_hosts: true
timezone: UTC
locale: en_US.UTF-8

users:
  - default
  - name: deploy
    groups:
      - sudo
    ssh_authorized_keys:
      - ssh-ed25519 AAAAC3... deploy@company

package_update: true
packages:
  - docker
  - nginx

write_files:
  - path: /etc/config.yaml
    content: test
    permissions: '0644'

runcmd:
  - systemctl start docker
"#;
        let config = CloudConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.hostname, Some("production-server".to_string()));
        assert_eq!(
            config.fqdn,
            Some("production-server.example.com".to_string())
        );
        assert_eq!(config.manage_etc_hosts, Some(true));
        assert_eq!(config.timezone, Some("UTC".to_string()));
        assert_eq!(config.locale, Some("en_US.UTF-8".to_string()));
        assert_eq!(config.users.len(), 2);
        assert_eq!(config.package_update, Some(true));
        assert_eq!(config.packages.len(), 2);
        assert_eq!(config.write_files.len(), 1);
        assert_eq!(config.runcmd.len(), 1);
    }
}
