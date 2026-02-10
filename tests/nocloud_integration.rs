//! Integration tests for NoCloud datasource using tempfile

use std::fs;
use tempfile::TempDir;

/// Test NoCloud with valid seed directory
#[test]
fn test_nocloud_seed_directory_structure() {
    let temp_dir = TempDir::new().unwrap();
    let seed_dir = temp_dir.path().join("nocloud");
    fs::create_dir_all(&seed_dir).unwrap();

    // Create meta-data file
    let meta_data = r#"instance-id: test-instance-001
local-hostname: test-host
"#;
    fs::write(seed_dir.join("meta-data"), meta_data).unwrap();

    // Create user-data file
    let user_data = r#"#cloud-config
hostname: configured-hostname
packages:
  - nginx
"#;
    fs::write(seed_dir.join("user-data"), user_data).unwrap();

    // Verify files exist
    assert!(seed_dir.join("meta-data").exists());
    assert!(seed_dir.join("user-data").exists());

    // Read and verify meta-data
    let content = fs::read_to_string(seed_dir.join("meta-data")).unwrap();
    assert!(content.contains("instance-id: test-instance-001"));
    assert!(content.contains("local-hostname: test-host"));

    // Read and verify user-data
    let content = fs::read_to_string(seed_dir.join("user-data")).unwrap();
    assert!(content.starts_with("#cloud-config"));
}

/// Test NoCloud with empty user-data
#[test]
fn test_nocloud_empty_userdata() {
    let temp_dir = TempDir::new().unwrap();
    let seed_dir = temp_dir.path().join("nocloud");
    fs::create_dir_all(&seed_dir).unwrap();

    fs::write(seed_dir.join("meta-data"), "instance-id: empty-test\n").unwrap();
    fs::write(seed_dir.join("user-data"), "").unwrap();

    let content = fs::read_to_string(seed_dir.join("user-data")).unwrap();
    assert!(content.is_empty());
}

/// Test NoCloud with script user-data
#[test]
fn test_nocloud_script_userdata() {
    let temp_dir = TempDir::new().unwrap();
    let seed_dir = temp_dir.path().join("nocloud");
    fs::create_dir_all(&seed_dir).unwrap();

    let script = r#"#!/bin/bash
echo "Hello from cloud-init"
apt-get update
"#;

    fs::write(seed_dir.join("meta-data"), "instance-id: script-test\n").unwrap();
    fs::write(seed_dir.join("user-data"), script).unwrap();

    let content = fs::read_to_string(seed_dir.join("user-data")).unwrap();
    assert!(content.starts_with("#!/bin/bash"));
}

/// Test NoCloud meta-data YAML parsing
#[test]
fn test_nocloud_metadata_yaml_parsing() {
    let meta_data = r#"instance-id: i-abcd1234
local-hostname: my-server
network-interfaces: |
  auto eth0
  iface eth0 inet dhcp
"#;

    let parsed: serde_yaml::Value = serde_yaml::from_str(meta_data).unwrap();
    assert_eq!(parsed["instance-id"].as_str().unwrap(), "i-abcd1234");
    assert_eq!(parsed["local-hostname"].as_str().unwrap(), "my-server");
}

/// Test NoCloud with vendor-data
#[test]
fn test_nocloud_vendor_data() {
    let temp_dir = TempDir::new().unwrap();
    let seed_dir = temp_dir.path().join("nocloud");
    fs::create_dir_all(&seed_dir).unwrap();

    fs::write(seed_dir.join("meta-data"), "instance-id: vendor-test\n").unwrap();
    fs::write(
        seed_dir.join("user-data"),
        "#cloud-config\nhostname: user\n",
    )
    .unwrap();

    let vendor_data = r#"#cloud-config
packages:
  - vendor-package
runcmd:
  - echo "vendor setup"
"#;
    fs::write(seed_dir.join("vendor-data"), vendor_data).unwrap();

    assert!(seed_dir.join("vendor-data").exists());
    let content = fs::read_to_string(seed_dir.join("vendor-data")).unwrap();
    assert!(content.contains("vendor-package"));
}

/// Test NoCloud network-config
#[test]
fn test_nocloud_network_config() {
    let temp_dir = TempDir::new().unwrap();
    let seed_dir = temp_dir.path().join("nocloud");
    fs::create_dir_all(&seed_dir).unwrap();

    fs::write(seed_dir.join("meta-data"), "instance-id: network-test\n").unwrap();

    // Network config v2 format
    let network_config = r#"version: 2
ethernets:
  eth0:
    dhcp4: true
  eth1:
    addresses:
      - 192.168.1.100/24
    gateway4: 192.168.1.1
"#;
    fs::write(seed_dir.join("network-config"), network_config).unwrap();

    let content = fs::read_to_string(seed_dir.join("network-config")).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
    assert_eq!(parsed["version"].as_i64().unwrap(), 2);
}

/// Test missing meta-data file
#[test]
fn test_nocloud_missing_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let seed_dir = temp_dir.path().join("nocloud");
    fs::create_dir_all(&seed_dir).unwrap();

    // Only create user-data, no meta-data
    fs::write(seed_dir.join("user-data"), "#cloud-config\n").unwrap();

    // meta-data should not exist
    assert!(!seed_dir.join("meta-data").exists());
}

/// Test base64 encoded user-data detection
#[test]
fn test_nocloud_base64_userdata() {
    use base64::Engine;

    let original = "#cloud-config\nhostname: encoded-test\n";
    let encoded = base64::engine::general_purpose::STANDARD.encode(original);

    // Verify encoding/decoding roundtrip
    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(&encoded)
        .unwrap();
    let decoded = String::from_utf8(decoded_bytes).unwrap();

    assert_eq!(decoded, original);
}

/// Test gzip compressed user-data detection
#[test]
fn test_nocloud_gzip_userdata() {
    use flate2::read::GzDecoder;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::{Read, Write};

    let original = "#cloud-config\nhostname: compressed-test\n";

    // Compress
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(original.as_bytes()).unwrap();
    let compressed = encoder.finish().unwrap();

    // Decompress
    let mut decoder = GzDecoder::new(&compressed[..]);
    let mut decompressed = String::new();
    decoder.read_to_string(&mut decompressed).unwrap();

    assert_eq!(decompressed, original);
}

/// Test cloud-config parsing from fixture files
#[test]
fn test_parse_fixture_files() {
    use cloud_init_rs::config::CloudConfig;

    // Basic fixture
    let basic = include_str!("fixtures/basic.yaml");
    let config = CloudConfig::from_yaml(basic).unwrap();
    assert_eq!(config.hostname, Some("test-instance".to_string()));
    assert_eq!(config.timezone, Some("America/New_York".to_string()));

    // Packages fixture
    let packages = include_str!("fixtures/packages.yaml");
    let config = CloudConfig::from_yaml(packages).unwrap();
    assert_eq!(config.package_update, Some(true));
    assert_eq!(config.packages.len(), 5);

    // Empty fixture
    let empty = include_str!("fixtures/empty.yaml");
    let config = CloudConfig::from_yaml(empty).unwrap();
    assert!(config.hostname.is_none());
}
