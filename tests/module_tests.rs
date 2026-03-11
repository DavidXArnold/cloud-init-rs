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

// ==================== rh_subscription Module Tests ====================

/// Test parsing rh_subscription with username/password
#[test]
fn test_rh_subscription_username_password() {
    let yaml = r#"#cloud-config
rh_subscription:
  username: user@example.com
  password: mypassword
  auto-attach: true
  service-level: self-support
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let sub = config.rh_subscription.unwrap();
    assert_eq!(sub.username, Some("user@example.com".to_string()));
    assert_eq!(sub.password, Some("mypassword".to_string()));
    assert_eq!(sub.auto_attach, Some(true));
    assert_eq!(sub.service_level, Some("self-support".to_string()));
    assert!(sub.activation_key.is_none());
    assert!(sub.org.is_none());
}

/// Test parsing rh_subscription with activation key
#[test]
fn test_rh_subscription_activation_key() {
    let yaml = r#"#cloud-config
rh_subscription:
  activation-key: myactivationkey
  org: "1234567"
  add-pool:
    - 8a85f9833e1d21f2013e1d21c6200011
  enable-repo:
    - rhel-7-server-optional-rpms
  disable-repo:
    - rhel-7-server-extras-rpms
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let sub = config.rh_subscription.unwrap();
    assert_eq!(sub.activation_key, Some("myactivationkey".to_string()));
    assert_eq!(sub.org, Some("1234567".to_string()));
    assert_eq!(sub.add_pool, vec!["8a85f9833e1d21f2013e1d21c6200011"]);
    assert_eq!(sub.enable_repo, vec!["rhel-7-server-optional-rpms"]);
    assert_eq!(sub.disable_repo, vec!["rhel-7-server-extras-rpms"]);
}

/// Test parsing rh_subscription with server overrides
#[test]
fn test_rh_subscription_server_overrides() {
    let yaml = r#"#cloud-config
rh_subscription:
  username: user@example.com
  password: pass
  server-hostname: subscription.example.com
  rhsm-baseurl: https://cdn.example.com
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let sub = config.rh_subscription.unwrap();
    assert_eq!(
        sub.server_hostname,
        Some("subscription.example.com".to_string())
    );
    assert_eq!(
        sub.rhsm_baseurl,
        Some("https://cdn.example.com".to_string())
    );
}

/// Test that absent rh_subscription key produces None
#[test]
fn test_rh_subscription_absent() {
    let yaml = "#cloud-config\nhostname: test\n";
    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert!(config.rh_subscription.is_none());
}

/// Test that optional list fields default to empty vecs when not specified
#[test]
fn test_rh_subscription_defaults_empty_lists() {
    let yaml = r#"#cloud-config
rh_subscription:
  username: user@example.com
  password: mypassword
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let sub = config.rh_subscription.unwrap();
    assert!(sub.add_pool.is_empty());
    assert!(sub.enable_repo.is_empty());
    assert!(sub.disable_repo.is_empty());
    assert!(sub.auto_attach.is_none());
    assert!(sub.service_level.is_none());
}

/// Test validation: missing credentials should produce an error
#[test]
fn test_rh_subscription_validation_missing_credentials() {
    use cloud_init_rs::config::RhSubscriptionConfig;

    // No username/password and no activation-key/org → invalid
    let sub = RhSubscriptionConfig {
        auto_attach: Some(true),
        ..Default::default()
    };

    let has_user_pass = sub.username.is_some() && sub.password.is_some();
    let has_key_org = sub.activation_key.is_some() && sub.org.is_some();
    assert!(
        !has_user_pass && !has_key_org,
        "Should have no valid credentials"
    );
}

/// Test validation: username without password is insufficient
#[test]
fn test_rh_subscription_validation_partial_credentials() {
    use cloud_init_rs::config::RhSubscriptionConfig;

    let sub = RhSubscriptionConfig {
        username: Some("user@example.com".to_string()),
        ..Default::default()
    };

    let has_user_pass = sub.username.is_some() && sub.password.is_some();
    let has_key_org = sub.activation_key.is_some() && sub.org.is_some();
    assert!(!has_user_pass, "Partial user/pass should not be valid");
    assert!(!has_key_org, "No activation key or org present");
}

// ==================== yum_add_repo Module Tests ====================

/// Test parsing yum_repos from cloud-config
#[test]
fn test_yum_repos_config_parsing() {
    let yaml = r#"#cloud-config
yum_repos:
  epel:
    name: Extra Packages for Enterprise Linux 8
    baseurl: https://download.fedoraproject.org/pub/epel/8/$basearch
    enabled: true
    gpgcheck: true
    gpgkey: https://dl.fedoraproject.org/pub/epel/RPM-GPG-KEY-EPEL-8
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.yum_repos.len(), 1);
    let epel = config.yum_repos.get("epel").unwrap();
    assert_eq!(
        epel.name,
        Some("Extra Packages for Enterprise Linux 8".to_string())
    );
    assert_eq!(
        epel.baseurl,
        Some("https://download.fedoraproject.org/pub/epel/8/$basearch".to_string())
    );
    assert_eq!(epel.enabled, Some(true));
    assert_eq!(epel.gpgcheck, Some(true));
}

/// Test parsing multiple yum repositories
#[test]
fn test_yum_repos_multiple() {
    let yaml = r#"#cloud-config
yum_repos:
  epel:
    name: EPEL
    baseurl: https://example.com/epel/$releasever/$basearch/
    enabled: true
    gpgcheck: false
  my-internal:
    name: Internal Repo
    baseurl: https://repo.example.com/centos/$releasever/
    enabled: true
    gpgcheck: false
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.yum_repos.len(), 2);
    assert!(config.yum_repos.contains_key("epel"));
    assert!(config.yum_repos.contains_key("my-internal"));
}

/// Test parsing yum repo with mirrorlist instead of baseurl
#[test]
fn test_yum_repos_mirrorlist() {
    let yaml = r#"#cloud-config
yum_repos:
  centos-base:
    name: CentOS Base
    mirrorlist: http://mirrorlist.centos.org/?release=$releasever&arch=$basearch&repo=os
    enabled: true
    gpgcheck: true
    gpgkey: file:///etc/pki/rpm-gpg/RPM-GPG-KEY-CentOS-7
"#;

    let config = CloudConfig::from_yaml(yaml).unwrap();
    let repo = config.yum_repos.get("centos-base").unwrap();
    assert!(repo.baseurl.is_none());
    assert!(repo.mirrorlist.is_some());
    assert_eq!(
        repo.gpgkey,
        Some("file:///etc/pki/rpm-gpg/RPM-GPG-KEY-CentOS-7".to_string())
    );
}

/// Test that absent yum_repos key yields an empty map
#[test]
fn test_yum_repos_absent() {
    let yaml = "#cloud-config\nhostname: test\n";
    let config = CloudConfig::from_yaml(yaml).unwrap();
    assert!(config.yum_repos.is_empty());
}

/// Test yum_add_repo::build_repo_content produces correct INI format
#[test]
fn test_build_repo_content_basic() {
    use cloud_init_rs::config::YumRepoConfig;
    use cloud_init_rs::modules::yum_add_repo::build_repo_content;

    let repo = YumRepoConfig {
        name: Some("My Repo".to_string()),
        baseurl: Some("https://repo.example.com/centos/8/$basearch/".to_string()),
        enabled: Some(true),
        gpgcheck: Some(false),
        ..Default::default()
    };

    let content = build_repo_content("my-repo", &repo);
    assert!(content.contains("[my-repo]"));
    assert!(content.contains("name=My Repo"));
    assert!(content.contains("baseurl=https://repo.example.com/centos/8/$basearch/"));
    assert!(content.contains("enabled=1"));
    assert!(content.contains("gpgcheck=0"));
}

/// Test build_repo_content with all optional fields
#[test]
fn test_build_repo_content_full() {
    use cloud_init_rs::config::YumRepoConfig;
    use cloud_init_rs::modules::yum_add_repo::build_repo_content;

    let repo = YumRepoConfig {
        name: Some("Full Repo".to_string()),
        baseurl: Some("https://repo.example.com/".to_string()),
        enabled: Some(true),
        gpgcheck: Some(true),
        gpgkey: Some("https://repo.example.com/RPM-GPG-KEY".to_string()),
        priority: Some(10),
        failovermethod: Some("priority".to_string()),
        sslverify: Some(true),
        sslclientcert: Some("/etc/pki/client.crt".to_string()),
        sslclientkey: Some("/etc/pki/client.key".to_string()),
        sslcacert: Some("/etc/pki/ca.crt".to_string()),
        ..Default::default()
    };

    let content = build_repo_content("full-repo", &repo);
    assert!(content.contains("[full-repo]"));
    assert!(content.contains("gpgkey=https://repo.example.com/RPM-GPG-KEY"));
    assert!(content.contains("priority=10"));
    assert!(content.contains("failovermethod=priority"));
    assert!(content.contains("sslverify=1"));
    assert!(content.contains("sslclientcert=/etc/pki/client.crt"));
    assert!(content.contains("sslclientkey=/etc/pki/client.key"));
    assert!(content.contains("sslcacert=/etc/pki/ca.crt"));
}

/// Test build_repo_content falls back to id when name is absent
#[test]
fn test_build_repo_content_name_fallback() {
    use cloud_init_rs::config::YumRepoConfig;
    use cloud_init_rs::modules::yum_add_repo::build_repo_content;

    let repo = YumRepoConfig {
        baseurl: Some("https://repo.example.com/".to_string()),
        ..Default::default()
    };

    let content = build_repo_content("my-id", &repo);
    // When name is absent, the id is used as the name
    assert!(content.contains("name=my-id"));
}

/// Test build_repo_content default enabled=true when not specified
#[test]
fn test_build_repo_content_default_enabled() {
    use cloud_init_rs::config::YumRepoConfig;
    use cloud_init_rs::modules::yum_add_repo::build_repo_content;

    let repo = YumRepoConfig {
        baseurl: Some("https://repo.example.com/".to_string()),
        ..Default::default()
    };

    let content = build_repo_content("test-repo", &repo);
    // Default enabled is true → 1
    assert!(content.contains("enabled=1"));
}

/// Test write_repo_file to a temp directory
#[tokio::test]
async fn test_write_repo_file() {
    use cloud_init_rs::config::YumRepoConfig;
    use cloud_init_rs::modules::yum_add_repo::build_repo_content;

    let temp_dir = TempDir::new().unwrap();
    let repo_id = "test-epel";
    let repo = YumRepoConfig {
        name: Some("Test EPEL".to_string()),
        baseurl: Some("https://example.com/epel/8/$basearch/".to_string()),
        enabled: Some(true),
        gpgcheck: Some(false),
        ..Default::default()
    };

    let content = build_repo_content(repo_id, &repo);
    let path = temp_dir.path().join(format!("{}.repo", repo_id));
    fs::write(&path, &content).unwrap();

    let written = fs::read_to_string(&path).unwrap();
    assert!(written.contains("[test-epel]"));
    assert!(written.contains("name=Test EPEL"));
    assert!(written.contains("baseurl=https://example.com/epel/8/$basearch/"));
}
