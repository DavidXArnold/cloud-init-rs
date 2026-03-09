//! Tests for configuration modules

use cloud_init_rs::config::{CloudConfig, RunCmd, WriteFileConfig};
use cloud_init_rs::modules::resize_rootfs::{FilesystemType, parse_root_from_mounts};
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

// ==================== Resize Rootfs Module Tests ====================

/// Test resize_rootfs enabled parsing
#[test]
fn test_resize_rootfs_enabled() {
    let yaml = r#"#cloud-config
resize_rootfs: true
"#;
    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.resize_rootfs, Some(true));
}

/// Test resize_rootfs disabled parsing
#[test]
fn test_resize_rootfs_disabled() {
    let yaml = r#"#cloud-config
resize_rootfs: false
"#;
    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.resize_rootfs, Some(false));
}

/// Test resize_rootfs not specified (defaults to enabled)
#[test]
fn test_resize_rootfs_not_specified() {
    let yaml = r#"#cloud-config
hostname: test
"#;
    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.resize_rootfs, None);
}

/// Test parse_root_from_mounts with a real-world /proc/mounts-like input
#[test]
fn test_parse_root_from_mounts_with_typical_proc_mounts_format() {
    let mounts = "\
sysfs /sys sysfs rw,nosuid,nodev,noexec,relatime 0 0\n\
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0\n\
devtmpfs /dev devtmpfs rw,nosuid,size=4096k,nr_inodes=4096,mode=755 0 0\n\
/dev/xvda1 / ext4 rw,relatime 0 0\n\
tmpfs /dev/shm tmpfs rw,nosuid,nodev 0 0\n\
";
    let root = parse_root_from_mounts(mounts).unwrap();
    assert_eq!(root.device, "/dev/xvda1");
    assert_eq!(root.fs_type, FilesystemType::Ext4);
}

/// Test FilesystemType detection for known types
#[test]
fn test_resize_rootfs_filesystem_type_detection() {
    let mounts_ext4 = "/dev/sda1 / ext4 rw 0 0\n";
    let mounts_xfs = "/dev/sdb1 / xfs rw 0 0\n";
    let mounts_btrfs = "/dev/sdc1 / btrfs rw 0 0\n";
    let mounts_unknown = "/dev/sdd1 / zfs rw 0 0\n";

    assert_eq!(
        parse_root_from_mounts(mounts_ext4).unwrap().fs_type,
        FilesystemType::Ext4
    );
    assert_eq!(
        parse_root_from_mounts(mounts_xfs).unwrap().fs_type,
        FilesystemType::Xfs
    );
    assert_eq!(
        parse_root_from_mounts(mounts_btrfs).unwrap().fs_type,
        FilesystemType::Btrfs
    );
    assert_eq!(
        parse_root_from_mounts(mounts_unknown).unwrap().fs_type,
        FilesystemType::Unknown("zfs".to_string())
    );
}
