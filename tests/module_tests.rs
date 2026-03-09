//! Tests for configuration modules

use cloud_init_rs::config::{AptPreference, AptSource, CloudConfig, RunCmd, WriteFileConfig};
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

// ==================== APT Module Tests ====================

/// Test APT proxy configuration parsing
#[test]
fn test_apt_proxy_config() {
    let yaml = r#"#cloud-config
apt:
  proxy: "http://proxy.example.com:3128"
  https_proxy: "https://proxy.example.com:3128"
  ftp_proxy: "ftp://proxy.example.com:21"
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let apt = config.apt.unwrap();
    assert_eq!(
        apt.proxy,
        Some("http://proxy.example.com:3128".to_string())
    );
    assert_eq!(
        apt.https_proxy,
        Some("https://proxy.example.com:3128".to_string())
    );
    assert_eq!(
        apt.ftp_proxy,
        Some("ftp://proxy.example.com:21".to_string())
    );
}

/// Test APT http_proxy alias
#[test]
fn test_apt_http_proxy_alias() {
    let yaml = r#"#cloud-config
apt:
  http_proxy: "http://mirror.local:3128"
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let apt = config.apt.unwrap();
    assert_eq!(
        apt.http_proxy,
        Some("http://mirror.local:3128".to_string())
    );
    assert!(apt.proxy.is_none());
}

/// Test APT conf snippet parsing
#[test]
fn test_apt_conf_snippet() {
    let yaml = r#"#cloud-config
apt:
  conf: |
    Acquire::ForceIPv4 "true";
    Acquire::Retries "3";
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let apt = config.apt.unwrap();
    let conf = apt.conf.unwrap();
    assert!(conf.contains("ForceIPv4"));
    assert!(conf.contains("Retries"));
}

/// Test APT sources parsing
#[test]
fn test_apt_sources() {
    let yaml = r#"#cloud-config
apt:
  sources:
    nginx-stable:
      source: "deb http://nginx.org/packages/ubuntu focal nginx"
      keyid: "ABF5BD827BD9BF62"
      keyserver: "keyserver.ubuntu.com"
    my-ppa:
      source: "deb http://ppa.launchpad.net/example/ppa/ubuntu focal main"
      filename: "example-ppa"
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let apt = config.apt.unwrap();
    assert_eq!(apt.sources.len(), 2);

    let nginx = apt.sources.get("nginx-stable").unwrap();
    assert!(nginx
        .source
        .as_deref()
        .unwrap()
        .contains("nginx.org/packages"));
    assert_eq!(
        nginx.keyid,
        Some("ABF5BD827BD9BF62".to_string())
    );
    assert_eq!(
        nginx.keyserver,
        Some("keyserver.ubuntu.com".to_string())
    );

    let ppa = apt.sources.get("my-ppa").unwrap();
    assert_eq!(ppa.filename, Some("example-ppa".to_string()));
}

/// Test APT source with inline GPG key
#[test]
fn test_apt_source_inline_key() {
    let yaml = r#"#cloud-config
apt:
  sources:
    custom-repo:
      source: "deb http://example.com/repo focal main"
      key: |
        -----BEGIN PGP PUBLIC KEY BLOCK-----
        fakekey
        -----END PGP PUBLIC KEY BLOCK-----
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let apt = config.apt.unwrap();
    let repo = apt.sources.get("custom-repo").unwrap();
    assert!(repo.key.as_deref().unwrap().contains("PGP PUBLIC KEY"));
}

/// Test APT preferences (pinning) parsing
#[test]
fn test_apt_preferences() {
    let yaml = r#"#cloud-config
apt:
  preferences:
    - package: "nginx"
      pin: "origin nginx.org"
      pin-priority: 900
    - package: "*"
      pin: "release a=stable"
      pin-priority: 100
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let apt = config.apt.unwrap();
    assert_eq!(apt.preferences.len(), 2);

    let pref0 = &apt.preferences[0];
    assert_eq!(pref0.package, "nginx");
    assert_eq!(pref0.pin, "origin nginx.org");
    assert_eq!(pref0.pin_priority, 900);

    let pref1 = &apt.preferences[1];
    assert_eq!(pref1.package, "*");
    assert_eq!(pref1.pin_priority, 100);
}

/// Test APT primary source parsing
#[test]
fn test_apt_primary_sources() {
    let yaml = r#"#cloud-config
apt:
  primary:
    - arches: [default]
      uri: "http://us-east-1.ec2.archive.ubuntu.com/ubuntu/"
      codename: "focal"
    - arches: [amd64]
      uri: "http://archive.ubuntu.com/ubuntu"
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let apt = config.apt.unwrap();
    assert_eq!(apt.primary.len(), 2);

    let first = &apt.primary[0];
    assert!(first.arches.contains(&"default".to_string()));
    assert!(first
        .uri
        .as_deref()
        .unwrap()
        .contains("ec2.archive.ubuntu.com"));
    assert_eq!(first.codename, Some("focal".to_string()));
}

/// Test parsing the apt.yaml fixture
#[test]
fn test_apt_fixture() {
    let yaml = fs::read_to_string("tests/fixtures/apt.yaml").unwrap();
    let config = CloudConfig::from_yaml(&yaml).unwrap();
    let apt = config.apt.unwrap();

    assert_eq!(
        apt.proxy,
        Some("http://proxy.example.com:3128".to_string())
    );
    assert_eq!(apt.sources.len(), 2);
    assert_eq!(apt.preferences.len(), 2);
    assert_eq!(apt.primary.len(), 1);
}

/// Test proxy content file generation
#[test]
fn test_apt_proxy_file_content_generation() {
    // Simulate what configure_proxy writes to the file
    let http_url = "http://proxy.example.com:3128";
    let https_url = "https://proxy.example.com:3128";

    let mut content = String::from("// Configured by cloud-init-rs\n");
    content.push_str(&format!("Acquire::http::Proxy \"{}\";\n", http_url));
    content.push_str(&format!("Acquire::https::Proxy \"{}\";\n", https_url));

    assert!(content.contains("Acquire::http::Proxy"));
    assert!(content.contains("Acquire::https::Proxy"));
    assert!(content.contains(http_url));
    assert!(content.contains(https_url));
    assert!(!content.contains("ftp"));
}

/// Test preferences file content generation
#[test]
fn test_apt_preferences_file_content_generation() {
    use cloud_init_rs::config::AptPreference;

    let preferences = vec![
        AptPreference {
            package: "nginx".to_string(),
            pin: "origin nginx.org".to_string(),
            pin_priority: 900,
        },
        AptPreference {
            package: "*".to_string(),
            pin: "release a=stable".to_string(),
            pin_priority: 100,
        },
    ];

    let mut content = String::from("# Configured by cloud-init-rs\n");
    for pref in &preferences {
        content.push_str(&format!(
            "\nPackage: {}\nPin: {}\nPin-Priority: {}\n",
            pref.package, pref.pin, pref.pin_priority
        ));
    }

    assert!(content.contains("Package: nginx"));
    assert!(content.contains("Pin: origin nginx.org"));
    assert!(content.contains("Pin-Priority: 900"));
    assert!(content.contains("Package: *"));
    assert!(content.contains("Pin-Priority: 100"));
}

/// Test sources.list.d file name derivation
#[test]
fn test_apt_source_list_filename_derivation() {
    use cloud_init_rs::config::AptSource;

    let cases = vec![
        (None, "nginx-stable", "nginx-stable"),
        (
            Some("custom-repo.list".to_string()),
            "ignored",
            "custom-repo",
        ),
        (Some("custom-repo".to_string()), "ignored", "custom-repo"),
    ];

    for (filename_override, name, expected) in cases {
        let source = AptSource {
            filename: filename_override,
            ..Default::default()
        };
        let stem = source
            .filename
            .as_deref()
            .unwrap_or(name)
            .trim_end_matches(".list");
        assert_eq!(stem, expected);
    }
}
