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

// ==================== YUM Repos Module Tests ====================

/// Test parsing a basic yum_repos cloud-config entry
#[test]
fn test_yum_repos_parse_basic() {
    let yaml = r#"#cloud-config
yum_repos:
  epel:
    name: Extra Packages for Enterprise Linux 9
    baseurl: https://download.fedoraproject.org/pub/epel/9/Everything/x86_64/
    enabled: true
    gpgcheck: true
    gpgkey: https://download.fedoraproject.org/pub/epel/RPM-GPG-KEY-EPEL-9
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.yum_repos.len(), 1);

    let repo = config.yum_repos.get("epel").unwrap();
    assert_eq!(
        repo.name,
        Some("Extra Packages for Enterprise Linux 9".to_string())
    );
    assert_eq!(
        repo.baseurl,
        Some("https://download.fedoraproject.org/pub/epel/9/Everything/x86_64/".to_string())
    );
    assert_eq!(repo.enabled, Some(true));
    assert_eq!(repo.gpgcheck, Some(true));
    assert_eq!(
        repo.gpgkey,
        Some("https://download.fedoraproject.org/pub/epel/RPM-GPG-KEY-EPEL-9".to_string())
    );
}

/// Test parsing multiple repositories
#[test]
fn test_yum_repos_parse_multiple() {
    let yaml = r#"#cloud-config
yum_repos:
  epel:
    name: EPEL
    baseurl: https://dl.fedoraproject.org/pub/epel/9/Everything/x86_64/
    enabled: true
    gpgcheck: true
  myrepo:
    name: My Custom Repo
    baseurl: https://example.com/repo/
    enabled: false
    gpgcheck: false
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.yum_repos.len(), 2);
    assert!(config.yum_repos.contains_key("epel"));
    assert!(config.yum_repos.contains_key("myrepo"));

    let myrepo = config.yum_repos.get("myrepo").unwrap();
    assert_eq!(myrepo.enabled, Some(false));
    assert_eq!(myrepo.gpgcheck, Some(false));
}

/// Test parsing repository with mirrorlist
#[test]
fn test_yum_repos_parse_mirrorlist() {
    let yaml = r#"#cloud-config
yum_repos:
  centos-appstream:
    name: CentOS Stream AppStream
    mirrorlist: https://mirrors.centos.org/mirrorlist?repo=centos-appstream-9&arch=x86_64
    enabled: true
    gpgcheck: true
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let repo = config.yum_repos.get("centos-appstream").unwrap();
    assert!(repo.baseurl.is_none());
    assert!(repo.mirrorlist.is_some());
    assert!(
        repo.mirrorlist
            .as_ref()
            .unwrap()
            .contains("centos-appstream-9")
    );
}

/// Test parsing repository with all optional fields
#[test]
fn test_yum_repos_parse_full_config() {
    let yaml = r#"#cloud-config
yum_repos:
  custom-repo:
    name: Custom Repository
    baseurl: https://example.com/repo/
    enabled: true
    gpgcheck: true
    gpgkey: https://example.com/gpg-key.asc
    skip_if_unavailable: true
    failovermethod: priority
    priority: 10
    sslverify: true
    exclude: "pkg1 pkg2"
    includepkgs: "pkg3 pkg4"
    type: rpm-md
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let repo = config.yum_repos.get("custom-repo").unwrap();
    assert_eq!(repo.skip_if_unavailable, Some(true));
    assert_eq!(repo.failovermethod, Some("priority".to_string()));
    assert_eq!(repo.priority, Some(10));
    assert_eq!(repo.sslverify, Some(true));
    assert_eq!(repo.exclude, Some("pkg1 pkg2".to_string()));
    assert_eq!(repo.includepkgs, Some("pkg3 pkg4".to_string()));
    assert_eq!(repo.repo_type, Some("rpm-md".to_string()));
}

/// Test empty yum_repos produces empty map
#[test]
fn test_yum_repos_empty() {
    let yaml = r#"#cloud-config
hostname: myhost
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert!(config.yum_repos.is_empty());
}

/// Test repo file content generation for a basic repository
#[test]
fn test_yum_repos_generate_content_basic() {
    use cloud_init_rs::config::YumRepoConfig;
    use cloud_init_rs::modules::yum_repos::generate_repo_content;

    let config = YumRepoConfig {
        name: Some("Extra Packages for Enterprise Linux 9".to_string()),
        baseurl: Some(
            "https://download.fedoraproject.org/pub/epel/9/Everything/x86_64/".to_string(),
        ),
        enabled: Some(true),
        gpgcheck: Some(true),
        gpgkey: Some("https://download.fedoraproject.org/pub/epel/RPM-GPG-KEY-EPEL-9".to_string()),
        ..Default::default()
    };

    let content = generate_repo_content("epel", &config);

    assert!(content.starts_with("[epel]\n"));
    assert!(content.contains("name=Extra Packages for Enterprise Linux 9\n"));
    assert!(
        content
            .contains("baseurl=https://download.fedoraproject.org/pub/epel/9/Everything/x86_64/\n")
    );
    assert!(content.contains("enabled=1\n"));
    assert!(content.contains("gpgcheck=1\n"));
    assert!(
        content.contains("gpgkey=https://download.fedoraproject.org/pub/epel/RPM-GPG-KEY-EPEL-9\n")
    );
}

/// Test that boolean fields are rendered as 1/0
#[test]
fn test_yum_repos_boolean_rendering() {
    use cloud_init_rs::config::YumRepoConfig;
    use cloud_init_rs::modules::yum_repos::generate_repo_content;

    let enabled = YumRepoConfig {
        enabled: Some(true),
        gpgcheck: Some(false),
        skip_if_unavailable: Some(true),
        sslverify: Some(false),
        ..Default::default()
    };
    let content = generate_repo_content("test", &enabled);
    assert!(content.contains("enabled=1\n"));
    assert!(content.contains("gpgcheck=0\n"));
    assert!(content.contains("skip_if_unavailable=1\n"));
    assert!(content.contains("sslverify=0\n"));
}

/// Test that absent optional fields are omitted from the output
#[test]
fn test_yum_repos_omit_absent_fields() {
    use cloud_init_rs::config::YumRepoConfig;
    use cloud_init_rs::modules::yum_repos::generate_repo_content;

    let config = YumRepoConfig {
        name: Some("Minimal Repo".to_string()),
        baseurl: Some("https://example.com/".to_string()),
        ..Default::default()
    };

    let content = generate_repo_content("minimal", &config);

    assert!(content.contains("[minimal]\n"));
    assert!(content.contains("name=Minimal Repo\n"));
    assert!(content.contains("baseurl=https://example.com/\n"));
    // Fields not set should not appear
    assert!(!content.contains("mirrorlist="));
    assert!(!content.contains("gpgcheck="));
    assert!(!content.contains("enabled="));
    assert!(!content.contains("gpgkey="));
}

/// Test that newlines in string values are stripped to prevent INI injection
#[test]
fn test_yum_repos_newline_sanitization() {
    use cloud_init_rs::config::YumRepoConfig;
    use cloud_init_rs::modules::yum_repos::generate_repo_content;

    let config = YumRepoConfig {
        name: Some("Evil\nRepo[injected]\nname=pwned".to_string()),
        baseurl: Some("https://example.com/".to_string()),
        ..Default::default()
    };

    let content = generate_repo_content("test", &config);
    // Newlines in values are replaced with spaces — [injected] must not appear
    // as a standalone INI section header on its own line.
    assert!(!content.contains("\n[injected]"));
    // A rogue `name=pwned` key must not appear on its own line either.
    assert!(!content.contains("\nname=pwned"));
}

/// Test file writing to a temporary directory
#[tokio::test]
async fn test_yum_repos_write_file() {
    use cloud_init_rs::config::YumRepoConfig;
    use cloud_init_rs::modules::yum_repos::generate_repo_content;
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let repo_id = "epel";
    let config = YumRepoConfig {
        name: Some("EPEL".to_string()),
        baseurl: Some("https://example.com/epel/".to_string()),
        enabled: Some(true),
        gpgcheck: Some(true),
        ..Default::default()
    };

    let content = generate_repo_content(repo_id, &config);
    let file_path = temp_dir.path().join(format!("{}.repo", repo_id));
    fs::write(&file_path, &content).unwrap();

    let written = fs::read_to_string(&file_path).unwrap();
    assert_eq!(written, content);
    assert!(written.starts_with("[epel]\n"));
    assert!(written.contains("enabled=1\n"));
}
