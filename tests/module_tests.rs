//! Tests for configuration modules

use cloud_init_rs::config::{CloudConfig, MountFieldValue, RunCmd, WriteFileConfig};
use std::fs;
use tempfile::TempDir;

// ==================== Write Files Module Tests ====================

/// Test basic file writing
#[test]
fn test_write_file_basic() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");

    let content = "Hello, World!";
    fs::write(&file_path, content).unwrap();

    let read_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(read_content, content);
}

/// Test file writing with newlines preserved
#[test]
fn test_write_file_multiline() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("config.yaml");

    let content = "server:\n  port: 8080\n  host: 0.0.0.0\n";
    fs::write(&file_path, content).unwrap();

    let read_content = fs::read_to_string(&file_path).unwrap();
    assert!(read_content.contains("port: 8080"));
    assert!(read_content.contains("host: 0.0.0.0"));
}

/// Test base64 decoding for write_files
#[test]
fn test_write_file_base64_decode() {
    use base64::Engine;

    let original = "#!/bin/bash\necho 'Hello World'\n";
    let encoded = base64::engine::general_purpose::STANDARD.encode(original);

    // Simulate WriteFileConfig with base64 encoding
    let config = WriteFileConfig {
        path: "/tmp/script.sh".to_string(),
        content: encoded.clone(),
        encoding: Some("base64".to_string()),
        owner: None,
        permissions: Some("0755".to_string()),
        append: None,
        defer: None,
    };

    assert_eq!(config.encoding, Some("base64".to_string()));

    // Decode content
    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(&config.content)
        .unwrap();
    let decoded = String::from_utf8(decoded_bytes).unwrap();
    assert_eq!(decoded, original);
}

/// Test file append mode
#[test]
fn test_write_file_append() {
    use std::fs::OpenOptions;
    use std::io::Write;

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("motd");

    // Write initial content
    fs::write(&file_path, "Welcome!\n").unwrap();

    // Append more content
    let mut file = OpenOptions::new().append(true).open(&file_path).unwrap();
    writeln!(file, "Extra message").unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("Welcome!"));
    assert!(content.contains("Extra message"));
}

/// Test creating parent directories
#[test]
fn test_write_file_create_parents() {
    let temp_dir = TempDir::new().unwrap();
    let nested_path = temp_dir.path().join("a/b/c/config.txt");

    // Create parent directories
    fs::create_dir_all(nested_path.parent().unwrap()).unwrap();
    fs::write(&nested_path, "nested content").unwrap();

    assert!(nested_path.exists());
}

/// Test parsing WriteFileConfig from cloud-config
#[test]
fn test_parse_write_files_config() {
    let yaml = r#"#cloud-config
write_files:
  - path: /etc/myapp.conf
    content: |
      [server]
      port = 8080
    owner: root:root
    permissions: '0644'
  - path: /opt/script.sh
    content: IyEvYmluL2Jhc2g=
    encoding: base64
    permissions: '0755'
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.write_files.len(), 2);

    let first = &config.write_files[0];
    assert_eq!(first.path, "/etc/myapp.conf");
    assert_eq!(first.permissions, Some("0644".to_string()));
    assert!(first.content.contains("[server]"));

    let second = &config.write_files[1];
    assert_eq!(second.encoding, Some("base64".to_string()));
}

// ==================== Hostname Module Tests ====================

/// Test hostname parsing
#[test]
fn test_hostname_config() {
    let yaml = r#"#cloud-config
hostname: my-server
fqdn: my-server.example.com
manage_etc_hosts: true
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.hostname, Some("my-server".to_string()));
    assert_eq!(config.fqdn, Some("my-server.example.com".to_string()));
    assert_eq!(config.manage_etc_hosts, Some(true));
}

/// Test hostname with special characters
#[test]
fn test_hostname_special_chars() {
    // Valid hostnames: alphanumeric and hyphens
    let yaml = r#"#cloud-config
hostname: web-server-01
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.hostname, Some("web-server-01".to_string()));
}

/// Test hostname file simulation
#[test]
fn test_hostname_file_write() {
    let temp_dir = TempDir::new().unwrap();
    let hostname_file = temp_dir.path().join("hostname");

    let hostname = "production-server";
    fs::write(&hostname_file, format!("{}\n", hostname)).unwrap();

    let content = fs::read_to_string(&hostname_file).unwrap();
    assert_eq!(content.trim(), hostname);
}

/// Test /etc/hosts generation
#[test]
fn test_etc_hosts_generation() {
    let hostname = "my-server";
    let fqdn = "my-server.example.com";

    let hosts_entry = format!("127.0.1.1 {} {}\n", fqdn, hostname);

    assert!(hosts_entry.contains("127.0.1.1"));
    assert!(hosts_entry.contains(hostname));
    assert!(hosts_entry.contains(fqdn));
}

// ==================== Runcmd Module Tests ====================

/// Test runcmd parsing - shell strings
#[test]
fn test_runcmd_shell_strings() {
    let yaml = r#"#cloud-config
runcmd:
  - echo "Hello"
  - apt-get update
  - systemctl start nginx
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.runcmd.len(), 3);

    for cmd in &config.runcmd {
        assert!(matches!(cmd, RunCmd::Shell(_)));
    }
}

/// Test runcmd parsing - argument arrays
#[test]
fn test_runcmd_arg_arrays() {
    let yaml = r#"#cloud-config
runcmd:
  - [mkdir, -p, /opt/myapp]
  - [touch, /tmp/marker]
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.runcmd.len(), 2);

    match &config.runcmd[0] {
        RunCmd::Args(args) => {
            assert_eq!(args[0], "mkdir");
            assert_eq!(args[1], "-p");
            assert_eq!(args[2], "/opt/myapp");
        }
        _ => panic!("Expected Args variant"),
    }
}

/// Test runcmd with mixed formats
#[test]
fn test_runcmd_mixed() {
    let yaml = r#"#cloud-config
runcmd:
  - echo "shell command"
  - [docker, run, -d, nginx]
  - systemctl daemon-reload
  - ["bash", "-c", "for i in 1 2 3; do echo $i; done"]
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.runcmd.len(), 4);

    assert!(matches!(&config.runcmd[0], RunCmd::Shell(_)));
    assert!(matches!(&config.runcmd[1], RunCmd::Args(_)));
    assert!(matches!(&config.runcmd[2], RunCmd::Shell(_)));
    assert!(matches!(&config.runcmd[3], RunCmd::Args(_)));
}

/// Test command with complex arguments
#[test]
fn test_runcmd_complex_args() {
    let yaml = r#"#cloud-config
runcmd:
  - ["bash", "-c", "echo 'complex \"quoted\" string'"]
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();

    match &config.runcmd[0] {
        RunCmd::Args(args) => {
            assert_eq!(args[0], "bash");
            assert_eq!(args[1], "-c");
            assert!(args[2].contains("complex"));
        }
        _ => panic!("Expected Args variant"),
    }
}

// ==================== SSH Keys Module Tests ====================

/// Test SSH authorized keys parsing
#[test]
fn test_ssh_authorized_keys() {
    let yaml = r#"#cloud-config
ssh_authorized_keys:
  - ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAAB... user1@host
  - ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAA... user2@host
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.ssh_authorized_keys.len(), 2);
    assert!(config.ssh_authorized_keys[0].starts_with("ssh-rsa"));
    assert!(config.ssh_authorized_keys[1].starts_with("ssh-ed25519"));
}

/// Test authorized_keys file format
#[test]
fn test_authorized_keys_file_format() {
    let keys = [
        "ssh-rsa AAAAB3... user1@host",
        "ssh-ed25519 AAAAC3... user2@host",
    ];

    let file_content = keys.join("\n");
    assert!(file_content.contains("ssh-rsa"));
    assert!(file_content.contains("ssh-ed25519"));

    // Each key on its own line
    let lines: Vec<&str> = file_content.lines().collect();
    assert_eq!(lines.len(), 2);
}

// ==================== User Module Tests ====================

/// Test user configuration parsing
#[test]
fn test_user_config_full() {
    let yaml = r#"#cloud-config
users:
  - name: deploy
    gecos: Deploy User
    homedir: /home/deploy
    shell: /bin/bash
    primary_group: deploy
    groups:
      - sudo
      - docker
    sudo: ALL=(ALL) NOPASSWD:ALL
    lock_passwd: true
    ssh_authorized_keys:
      - ssh-ed25519 AAAAC3... deploy@company
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.users.len(), 1);
}

/// Test useradd command generation
#[test]
fn test_useradd_command_generation() {
    let username = "testuser";
    let shell = "/bin/bash";
    let homedir = "/home/testuser";
    let gecos = "Test User";
    let groups = ["sudo", "docker"];

    // Build useradd command
    let mut cmd = vec!["useradd".to_string()];
    cmd.extend(["-m".to_string()]); // Create home directory
    cmd.extend(["-s".to_string(), shell.to_string()]);
    cmd.extend(["-d".to_string(), homedir.to_string()]);
    cmd.extend(["-c".to_string(), gecos.to_string()]);
    cmd.extend(["-G".to_string(), groups.join(",")]);
    cmd.push(username.to_string());

    assert!(cmd.contains(&"useradd".to_string()));
    assert!(cmd.contains(&shell.to_string()));
    assert!(cmd.contains(&"sudo,docker".to_string()));
}

// ==================== Package Module Tests ====================

/// Test package list parsing
#[test]
fn test_packages_config() {
    let yaml = r#"#cloud-config
package_update: true
package_upgrade: true
packages:
  - nginx
  - vim
  - git
  - curl
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.package_update, Some(true));
    assert_eq!(config.package_upgrade, Some(true));
    assert_eq!(config.packages.len(), 4);
    assert!(config.packages.contains(&"nginx".to_string()));
}

// ==================== Mounts Module Tests ====================

/// Test parsing basic mount entries from cloud-config
#[test]
fn test_mounts_basic_parsing() {
    let yaml = r#"#cloud-config
mounts:
  - [/dev/sda1, /mnt/data, ext4, defaults, 0, 2]
  - [/dev/sdb, /mnt/backup, xfs, "defaults,noatime", 0, 2]
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.mounts.len(), 2);

    let fields0 = config.mounts[0].fields();
    assert_eq!(fields0[0], Some("/dev/sda1".to_string()));
    assert_eq!(fields0[1], Some("/mnt/data".to_string()));
    assert_eq!(fields0[2], Some("ext4".to_string()));
    assert_eq!(fields0[3], Some("defaults".to_string()));
    assert_eq!(fields0[4], Some("0".to_string()));
    assert_eq!(fields0[5], Some("2".to_string()));
}

/// Test that integer fields (dump, pass) are coerced to strings
#[test]
fn test_mounts_integer_fields_coerced() {
    let yaml = r#"#cloud-config
mounts:
  - [/dev/sda1, /mnt, ext4, defaults, 0, 2]
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let fields = config.mounts[0].fields();
    assert_eq!(fields[4], Some("0".to_string()));
    assert_eq!(fields[5], Some("2".to_string()));
}

/// Test parsing a swap entry
#[test]
fn test_mounts_swap_entry() {
    let yaml = r#"#cloud-config
mounts:
  - [swap, none, swap, sw, 0, 0]
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.mounts.len(), 1);
    let fields = config.mounts[0].fields();
    assert_eq!(fields[0], Some("swap".to_string()));
    assert_eq!(fields[2], Some("swap".to_string()));
}

/// Test null fields in a mount entry
#[test]
fn test_mounts_null_fields() {
    let yaml = r#"#cloud-config
mounts:
  - [/dev/sdc, /data, ~, ~, ~, ~]
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let fields = config.mounts[0].fields();
    assert_eq!(fields[0], Some("/dev/sdc".to_string()));
    assert_eq!(fields[1], Some("/data".to_string()));
    // Null fields should yield None
    assert_eq!(fields[2], None);
    assert_eq!(fields[3], None);
}

/// Test mount entry with only device and mount point
#[test]
fn test_mounts_minimal_entry() {
    let yaml = r#"#cloud-config
mounts:
  - [/dev/sda2, /opt]
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let fields = config.mounts[0].fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0], Some("/dev/sda2".to_string()));
    assert_eq!(fields[1], Some("/opt".to_string()));
}

/// Test mount entry using a UUID device identifier
#[test]
fn test_mounts_uuid_device() {
    let yaml = r#"#cloud-config
mounts:
  - [UUID=1234-5678, /boot, vfat, defaults, 0, 1]
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let fields = config.mounts[0].fields();
    assert_eq!(fields[0], Some("UUID=1234-5678".to_string()));
    assert_eq!(fields[5], Some("1".to_string()));
}

/// Test mount_default_fields parsing
#[test]
fn test_mount_default_fields() {
    let yaml = r#"#cloud-config
mount_default_fields: [~, ~, auto, defaults, '0', '2']
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.mount_default_fields.len(), 6);

    let defaults: Vec<Option<String>> = config
        .mount_default_fields
        .iter()
        .map(|f| f.as_ref().map(|v| v.as_str_val()))
        .collect();
    assert_eq!(defaults[0], None);
    assert_eq!(defaults[1], None);
    assert_eq!(defaults[2], Some("auto".to_string()));
    assert_eq!(defaults[3], Some("defaults".to_string()));
    assert_eq!(defaults[4], Some("0".to_string()));
    assert_eq!(defaults[5], Some("2".to_string()));
}

/// Test swap config parsing - auto size
#[test]
fn test_swap_config_auto() {
    let yaml = r#"#cloud-config
swap:
  filename: /swap.img
  size: auto
  maxsize: 4096
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let swap = config.swap.unwrap();
    assert_eq!(swap.filename, Some("/swap.img".to_string()));
    assert_eq!(swap.size, Some("auto".to_string()));
    assert_eq!(swap.maxsize, Some(4096));
}

/// Test swap config parsing - numeric size
#[test]
fn test_swap_config_numeric_size() {
    let yaml = r#"#cloud-config
swap:
  size: '2048'
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let swap = config.swap.unwrap();
    assert_eq!(swap.size, Some("2048".to_string()));
    assert_eq!(swap.filename, None); // defaults to /swap.img in configure_swap at runtime
    assert_eq!(swap.maxsize, None);
}

/// Test MountFieldValue integer-to-string conversion
#[test]
fn test_mount_field_value_int_to_str() {
    let v = MountFieldValue::Integer(0);
    assert_eq!(v.as_str_val(), "0");
    let v = MountFieldValue::Integer(2);
    assert_eq!(v.as_str_val(), "2");
    let v = MountFieldValue::Text("auto".to_string());
    assert_eq!(v.as_str_val(), "auto");
}

/// Test that empty mounts list is accepted
#[test]
fn test_mounts_empty_list() {
    let yaml = r#"#cloud-config
hostname: test
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert!(config.mounts.is_empty());
    assert!(config.swap.is_none());
    assert!(config.mount_default_fields.is_empty());
}

/// Test fstab line format produced by mount entries
#[test]
fn test_mount_fstab_format_simulation() {
    let yaml = r#"#cloud-config
mounts:
  - [/dev/nvme0n1p1, /data, ext4, "defaults,noatime", 0, 2]
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let fields = config.mounts[0].fields();

    // Build the expected fstab line from the parsed fields.
    let device = fields[0].as_deref().unwrap_or("");
    let mp = fields[1].as_deref().unwrap_or("");
    let fs = fields[2].as_deref().unwrap_or("auto");
    let opts = fields[3].as_deref().unwrap_or("defaults");
    let dump = fields[4].as_deref().unwrap_or("0");
    let pass = fields[5].as_deref().unwrap_or("2");

    let line = format!("{device}\t{mp}\t{fs}\t{opts}\t{dump}\t{pass}");
    assert_eq!(line, "/dev/nvme0n1p1\t/data\text4\tdefaults,noatime\t0\t2");
}

/// Test creating mount point directories
#[test]
fn test_mount_point_directory_creation() {
    let temp_dir = TempDir::new().unwrap();
    let mount_point = temp_dir.path().join("mnt/data");

    fs::create_dir_all(&mount_point).unwrap();
    assert!(mount_point.exists());
    assert!(mount_point.is_dir());
}

/// Test that a complete mounts config round-trips through YAML
#[test]
fn test_mounts_full_config_round_trip() {
    let yaml = r#"#cloud-config
mounts:
  - [/dev/sda1, /mnt/data, ext4, defaults, 0, 2]
  - [swap, none, swap, sw, 0, 0]

mount_default_fields: [~, ~, auto, defaults, '0', '2']

swap:
  filename: /swap.img
  size: auto
  maxsize: 2048
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.mounts.len(), 2);
    assert_eq!(config.mount_default_fields.len(), 6);

    let swap = config.swap.unwrap();
    assert_eq!(swap.filename, Some("/swap.img".to_string()));
    assert_eq!(swap.maxsize, Some(2048));
}
