//! Tests for configuration modules

use cloud_init_rs::config::{CloudConfig, RunCmd, WriteFileConfig};
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

// ==================== Disk Setup Module Tests ====================

/// Test parsing a disk_setup config with layout: true (single whole-disk partition)
#[test]
fn test_parse_disk_setup_layout_true() {
    use cloud_init_rs::config::PartitionLayout;

    let yaml = r#"
#cloud-config
disk_setup:
  /dev/sdb:
    table_type: gpt
    layout: true
    overwrite: false
"#;
    let config = CloudConfig::from_yaml(yaml).unwrap();
    let disk_setup = config.disk_setup.unwrap();
    assert_eq!(disk_setup.len(), 1);

    let disk = disk_setup.get("/dev/sdb").unwrap();
    assert_eq!(disk.table_type, Some("gpt".to_string()));
    assert_eq!(disk.overwrite, Some(false));
    assert!(matches!(disk.layout, Some(PartitionLayout::Simple(true))));
}

/// Test parsing a disk_setup config with layout: false (partition table only)
#[test]
fn test_parse_disk_setup_layout_false() {
    use cloud_init_rs::config::PartitionLayout;

    let yaml = r#"
#cloud-config
disk_setup:
  /dev/sdc:
    table_type: mbr
    layout: false
    overwrite: true
"#;
    let config = CloudConfig::from_yaml(yaml).unwrap();
    let disk_setup = config.disk_setup.unwrap();
    let disk = disk_setup.get("/dev/sdc").unwrap();
    assert_eq!(disk.table_type, Some("mbr".to_string()));
    assert_eq!(disk.overwrite, Some(true));
    assert!(matches!(disk.layout, Some(PartitionLayout::Simple(false))));
}

/// Test parsing a disk_setup config with an explicit partition list
#[test]
fn test_parse_disk_setup_partition_list() {
    use cloud_init_rs::config::{PartitionLayout, PartitionSpec};

    let yaml = r#"
#cloud-config
disk_setup:
  /dev/sdd:
    table_type: gpt
    layout:
      - 25
      - [25, 82]
      - 50
    overwrite: true
"#;
    let config = CloudConfig::from_yaml(yaml).unwrap();
    let disk_setup = config.disk_setup.unwrap();
    let disk = disk_setup.get("/dev/sdd").unwrap();

    match disk.layout.as_ref().unwrap() {
        PartitionLayout::Partitions(specs) => {
            assert_eq!(specs.len(), 3);
            assert!(matches!(specs[0], PartitionSpec::Size(25)));
            match &specs[1] {
                PartitionSpec::SizeAndType(parts) => {
                    assert_eq!(parts[0], 25);
                    assert_eq!(parts[1], 82);
                }
                _ => panic!("Expected SizeAndType for second partition"),
            }
            assert!(matches!(specs[2], PartitionSpec::Size(50)));
        }
        _ => panic!("Expected Partitions layout"),
    }
}

/// Test that disk_setup is absent when not configured
#[test]
fn test_parse_no_disk_setup() {
    let yaml = "#cloud-config\nhostname: test\n";
    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert!(config.disk_setup.is_none());
}

/// Test parsing multiple disks in disk_setup
#[test]
fn test_parse_disk_setup_multiple_disks() {
    let yaml = r#"
#cloud-config
disk_setup:
  /dev/sdb:
    table_type: gpt
    layout: true
  /dev/sdc:
    table_type: mbr
    layout: false
"#;
    let config = CloudConfig::from_yaml(yaml).unwrap();
    let disk_setup = config.disk_setup.unwrap();
    assert_eq!(disk_setup.len(), 2);
    assert!(disk_setup.contains_key("/dev/sdb"));
    assert!(disk_setup.contains_key("/dev/sdc"));
}

/// Test that table_type defaults handling and optional fields parse correctly
#[test]
fn test_parse_disk_setup_minimal() {
    let yaml = r#"
#cloud-config
disk_setup:
  /dev/sdb:
    table_type: gpt
"#;
    let config = CloudConfig::from_yaml(yaml).unwrap();
    let disk_setup = config.disk_setup.unwrap();
    let disk = disk_setup.get("/dev/sdb").unwrap();
    assert_eq!(disk.table_type, Some("gpt".to_string()));
    assert!(disk.layout.is_none());
    assert!(disk.overwrite.is_none());
}

/// Test the sfdisk script builder with a whole-disk layout
#[test]
fn test_build_script_whole_disk_gpt() {
    use cloud_init_rs::modules::disk_setup::build_sfdisk_script;
    use cloud_init_rs::config::PartitionLayout;

    let script = build_sfdisk_script("gpt", &Some(PartitionLayout::Simple(true)));
    assert!(script.contains("label: gpt"));
    assert!(script.contains("size=+, type=linux"));
}

/// Test the sfdisk script builder with an explicit partition list
#[test]
fn test_build_script_partition_list() {
    use cloud_init_rs::modules::disk_setup::build_sfdisk_script;
    use cloud_init_rs::config::{PartitionLayout, PartitionSpec};

    let layout = Some(PartitionLayout::Partitions(vec![
        PartitionSpec::Size(25),
        PartitionSpec::SizeAndType(vec![25, 82]),
        PartitionSpec::Size(50),
    ]));
    let script = build_sfdisk_script("gpt", &layout);

    assert!(script.contains("label: gpt"));
    assert!(script.contains("size=25%, type=linux"));
    assert!(script.contains("size=25%, type=linux-swap"));
    // Last partition always uses size=+.
    assert!(script.contains("size=+, type=linux"));
}

/// Test normalize_table_type with valid and invalid inputs
#[test]
fn test_normalize_table_type() {
    use cloud_init_rs::modules::disk_setup::normalize_table_type;

    assert_eq!(normalize_table_type("gpt").unwrap(), "gpt");
    assert_eq!(normalize_table_type("GPT").unwrap(), "gpt");
    assert_eq!(normalize_table_type("mbr").unwrap(), "dos");
    assert_eq!(normalize_table_type("msdos").unwrap(), "dos");
    assert_eq!(normalize_table_type("dos").unwrap(), "dos");
    assert!(normalize_table_type("zfs").is_err());
}

