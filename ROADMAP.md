# cloud-init-rs Roadmap

This roadmap outlines the path to achieving 80%+ compatibility with cloud-init.

## Phase 1: Core Infrastructure
**Status: ✅ Complete**

- [x] Project structure and build system
- [x] CLI with subcommands (init, local, network, config, final)
- [x] Stage execution framework
- [x] Error handling with `thiserror`
- [x] Logging with `tracing`
- [x] Cloud-config YAML parsing basics

## Phase 2: GitHub & CI/CD
**Status: ✅ Complete**

### Repository Setup
- [x] Create GitHub repository (ready to push)
- [ ] Configure branch protection (main) - do after first push
- [x] Add issue templates (bug_report.md, feature_request.md)
- [x] Add PR template
- [x] Add CONTRIBUTING.md
- [x] Add CODE_OF_CONDUCT.md

### CI Workflow (on every PR and push to main)
- [x] `ci.yml` - Main CI pipeline
  - [x] Run on ubuntu-latest
  - [x] Matrix test with stable + MSRV (1.88)
  - [x] `cargo fmt --check`
  - [x] `cargo clippy -- -D warnings`
  - [x] `cargo test`
  - [x] `cargo build --release`
  - [x] Build and upload docs

### Coverage Workflow
- [x] `coverage.yml` - Code coverage reporting
  - [x] Use cargo-llvm-cov
  - [x] Upload to Codecov
  - [ ] Coverage badge in README (after repo created)
  - [ ] Fail PR if coverage drops below threshold (configure in Codecov)

### Release Workflow
- [x] `release.yml` - Automated releases
  - [x] Trigger on tag push (v*.*.*)
  - [x] Build release binaries for:
    - [x] x86_64-unknown-linux-gnu
    - [x] x86_64-unknown-linux-musl (static)
    - [x] aarch64-unknown-linux-gnu
    - [x] x86_64-apple-darwin
    - [x] aarch64-apple-darwin
  - [x] Create GitHub Release with changelog
  - [x] Upload binary artifacts
  - [x] Generate checksums (SHA256)

### Publish Workflow
- [ ] `publish.yml` - Crates.io publishing (deferred to v1.0.0)

### OS Package Publishing Workflow
- [x] `packages.yml` - Build and publish OS packages on release
  - [x] Trigger on GitHub Release (v*.*.*)
  - [x] Debian/Ubuntu packages (.deb)
    - [x] Build using cargo-deb
    - [x] Target: amd64, arm64
    - [x] Include systemd service files
    - [x] Upload to GitHub Release
    - [ ] Publish to PPA or packagecloud.io (deferred)
  - [x] RHEL/Fedora packages (.rpm)
    - [x] Build using cargo-generate-rpm
    - [x] Target: x86_64, aarch64
    - [x] Include systemd service files
    - [x] Upload to GitHub Release
    - [ ] Publish to COPR or packagecloud.io (deferred)
  - [x] Package metadata
    - [x] Proper package description and license
    - [x] Correct dependencies (none for static builds)
    - [x] Post-install scripts for systemd enablement
    - [ ] Changelog generation from git tags (deferred)

### Security
- [x] `audit.yml` - Security scanning
  - [x] Run `cargo audit` weekly
  - [x] Run on Cargo.toml/Cargo.lock changes
  - [x] Dependabot for dependency updates

### Documentation
- [x] Rustdoc built in CI workflow
- [ ] Deploy to GitHub Pages (after repo created)

## Phase 3: Test Infrastructure (High Priority)
**Status: ✅ Complete**

Test coverage is critical for a system-level tool. Tests should be written alongside features.

### Unit Tests (36 tests)
- [x] Cloud-config parsing tests
  - [x] Valid YAML parsing
  - [x] Malformed YAML handling
  - [x] All config field types (users, groups, write_files, runcmd, packages, ssh, etc.)
  - [x] Edge cases (empty, comments-only, unknown fields)
- [x] Datasource tests
  - [x] NoCloud file parsing
  - [x] EC2 IMDS response parsing
  - [x] MockDatasource with builder pattern
- [x] Module tests
  - [x] User creation and configuration
  - [x] File writing with base64/gzip
  - [x] Command execution (shell strings and args arrays)

### Integration Tests (17 tests)
- [x] Mock HTTP server tests (wiremock)
  - [x] EC2 IMDS mock responses (IMDSv1 and IMDSv2)
  - [x] Timeout handling
  - [x] Error responses (403, 404)
- [x] Filesystem tests (tempdir)
  - [x] NoCloud seed directory structure
  - [x] write_files output verification
  - [x] Base64/gzip encoding roundtrips
- [x] Fixture-based tests
  - [x] Parse all fixture YAML files
  - [x] Verify config values

### Test Utilities
- [x] Test fixtures in tests/fixtures/ (8 YAML files)
- [x] MockDatasource implementation (src/datasources/mock.rs)
- [x] Tempdir helper via tempfile crate
- [x] assert_fs and predicates for assertions

### Coverage Infrastructure
- [x] CI integration with coverage reporting (cargo-llvm-cov + Codecov)
- [ ] 80% line coverage for `src/config/` (in progress)
- [ ] 80% line coverage for `src/datasources/` (in progress)
- [ ] 70% line coverage for `src/modules/` (in progress)

## Phase 4: Datasources
**Status: 🔄 In Progress**

### High Priority (80% of cloud deployments)
- [x] NoCloud (local files, ISO)
- [x] EC2 (AWS) - IMDSv1 and IMDSv2
- [x] GCE (Google Cloud Platform)
- [x] Azure (IMDS)
- [x] OpenStack (config-drive and metadata service)

### Medium Priority
- [ ] Oracle Cloud Infrastructure
- [ ] Vultr
- [ ] DigitalOcean
- [ ] Hetzner
- [ ] Scaleway

### Lower Priority
- [ ] CloudStack
- [ ] SmartOS
- [ ] VMware (GuestInfo)
- [ ] LXD
- [ ] MAAS

## Phase 5: Configuration Modules
**Status: ✅ Complete**

### Users and Groups (High Priority)
- [x] `users` - Create users with full options
  - [x] Default user handling
  - [x] SSH key injection
  - [x] sudo configuration (/etc/sudoers.d/)
  - [x] Password setting (pre-hashed)
  - [x] lock_passwd support
  - [x] Group membership
  - [x] primary_group support
- [x] `groups` - Create groups via groupadd
- [x] `ssh_authorized_keys` - Root and user keys

### Files and Content (High Priority)
- [x] `write_files` - Write arbitrary files
  - [x] Basic file writing
  - [x] Base64 decoding
  - [x] Gzip decompression (gz, gzip+base64)
  - [x] Permissions and ownership
  - [x] Deferred writes
- [x] `bootcmd` - Early boot commands
- [x] `runcmd` - Late boot commands
  - [x] Basic command execution
  - [ ] Shell selection
  - [ ] Error handling modes

### System Configuration (High Priority)
- [x] `hostname` - Set hostname
  - [x] Basic implementation
  - [x] hostnamectl support
  - [x] FQDN handling
  - [x] /etc/hosts management
- [x] `timezone` - Set timezone (timedatectl + fallback)
- [x] `locale` - Set system locale (localectl + fallback)
- [ ] `keyboard` - Keyboard layout
- [x] `ntp` - NTP configuration (chrony/timesyncd/ntpd)

### Disk and Filesystem (Medium Priority)
- [ ] `growpart` - Grow partitions
- [ ] `resize_rootfs` - Resize root filesystem
- [ ] `mounts` - Configure mounts
- [ ] `disk_setup` - Partition disks
- [ ] `fs_setup` - Create filesystems

### Package Management (Medium Priority)
- [x] `packages` - Install packages (apt/dnf/yum/zypper/apk)
- [ ] `package_update` - Update package cache
- [ ] `package_upgrade` - Upgrade packages
- [ ] `apt` - APT-specific config
- [ ] `yum_repos` - YUM repositories
- [ ] `snap` - Snap packages

### Network Configuration (Medium Priority)
- [x] Network config v1 parsing (src/network/v1.rs)
- [x] Network config v2 (Netplan) parsing (src/network/mod.rs)
- [x] Renderer: networkd (src/network/render/networkd.rs)
- [x] Renderer: NetworkManager (src/network/render/network_manager.rs)
- [x] Renderer: ENI (Debian) (src/network/render/eni.rs)
- [x] Static IP configuration
- [x] DHCP configuration
- [x] Bonding and VLANs

### Security (Medium Priority)
- [ ] `ca_certs` - CA certificates
- [ ] `ssh` - SSH daemon configuration
- [ ] `disable_root` - Disable root login
- [ ] `random_seed` - Seed random number generator

### Cloud-specific (Lower Priority)
- [ ] `phone_home` - Notify external URL
- [ ] `power_state` - Reboot/shutdown
- [ ] `final_message` - Completion message
- [ ] `scripts_user` - User scripts
- [ ] `scripts_vendor` - Vendor scripts

## Phase 6: Advanced Features
**Status: ✅ Complete**

### Multi-part MIME
- [x] Parse multipart user-data (src/userdata/mime.rs)
- [x] cloud-config merging (src/config/merge.rs)
- [x] Include directives (src/userdata/mod.rs)
- [x] cloud-boothook support

### Jinja Templating
- [x] Instance metadata variables
- [x] ds (datasource) variables
- [x] v1 data variables
- [x] minijinja integration (src/template/mod.rs)

### Cloud-config Merging
- [x] /etc/cloud/cloud.cfg
- [x] /etc/cloud/cloud.cfg.d/*.cfg
- [x] User-data cloud-config
- [x] Vendor-data
- [x] ConfigLoader builder pattern (src/config/loader.rs)

### Instance State
- [x] /var/lib/cloud directory structure
- [x] Instance ID tracking
- [x] Per-instance vs per-boot markers
- [x] sem/ semaphore files
- [x] SemaphoreManager (src/state/semaphore.rs)

## Phase 7: Compatibility Validation

### Cloud Provider Integration Tests
Comprehensive end-to-end testing on real cloud infrastructure.

#### AWS/EC2 Integration Tests
- [ ] IMDSv1 metadata retrieval
- [ ] IMDSv2 with token-based auth
- [ ] Instance identity document validation
- [ ] User-data retrieval (plain, base64, gzip)
- [ ] SSH key injection and verification
- [ ] Spot instance metadata handling
- [ ] Multiple network interface metadata
- [ ] Instance tags via metadata
- [ ] IAM role credential retrieval
- [ ] Placement group and availability zone detection

#### GCE Integration Tests
- [ ] Metadata server connectivity (metadata.google.internal)
- [ ] Project-level vs instance-level metadata
- [ ] Startup-script execution
- [ ] User-data (custom metadata) retrieval
- [ ] SSH key injection via metadata
- [ ] Service account token retrieval
- [ ] Network interface metadata
- [ ] Instance scheduling metadata
- [ ] Preemptible instance detection
- [ ] Zone and region extraction

#### Azure Integration Tests
- [ ] IMDS endpoint connectivity (169.254.169.254)
- [ ] Instance metadata (vmId, location, vmSize)
- [ ] Custom data retrieval and base64 decoding
- [ ] SSH key injection
- [ ] Managed identity token retrieval
- [ ] Availability zone detection
- [ ] Virtual network metadata
- [ ] Scheduled events handling
- [ ] Attestation data retrieval
- [ ] Tag metadata access

#### OpenStack Integration Tests
- [ ] Config-drive detection and mounting
- [ ] Config-drive metadata parsing (meta_data.json)
- [ ] Config-drive user-data retrieval
- [ ] HTTP metadata service fallback
- [ ] Network configuration (network_data.json)
- [ ] Vendor-data handling
- [ ] Instance UUID and hostname
- [ ] SSH key injection
- [ ] Availability zone metadata
- [ ] Nova vs Ironic metadata differences

#### Cross-Cloud Validation
- [ ] Consistent InstanceMetadata across providers
- [ ] UserData parsing consistency
- [ ] Cloud-config execution parity
- [ ] Error handling consistency
- [ ] Timeout behavior validation

### Local/Mock Testing Infrastructure
- [ ] QEMU/KVM test harness
- [ ] Docker-based metadata service mocks
- [ ] Vagrant multi-provider tests
- [ ] LocalStack for AWS testing
- [ ] Fake GCE metadata server
- [ ] Azure metadata emulator

### Compatibility Testing
- [ ] cloud-init test suite adaptation
- [ ] Parity testing with Python cloud-init
- [ ] Real VM tests (QEMU/KVM)
- [ ] Container tests (Docker, Podman)

### Regression Testing
- [ ] Automated nightly test runs
- [ ] Performance benchmarks vs Python cloud-init
- [ ] Boot time measurements

## Phase 8: Production Readiness

### Packaging Infrastructure
- [x] Package build tooling
  - [x] cargo-deb configuration in Cargo.toml
  - [x] cargo-generate-rpm configuration
  - [x] Systemd unit file templates
  - [x] Package post-install/pre-remove scripts

### Debian/Ubuntu Packages (.deb)
- [x] Package structure
  - [x] Binary: /usr/bin/cloud-init-rs
  - [ ] Config: /etc/cloud/cloud.cfg.d/
  - [x] Systemd: /lib/systemd/system/cloud-init*.service
  - [x] Docs: /usr/share/doc/cloud-init-rs/
- [x] Architectures: amd64, arm64
- [ ] Distribution targets
  - [ ] Ubuntu 22.04 LTS (Jammy)
  - [ ] Ubuntu 24.04 LTS (Noble)
  - [ ] Debian 11 (Bullseye)
  - [ ] Debian 12 (Bookworm)
- [ ] Repository hosting (PPA or packagecloud.io)

### RHEL/Fedora Packages (.rpm)
- [x] Package structure
  - [x] Binary: /usr/bin/cloud-init-rs
  - [ ] Config: /etc/cloud/cloud.cfg.d/
  - [x] Systemd: /usr/lib/systemd/system/cloud-init*.service
  - [x] Docs: /usr/share/doc/cloud-init-rs/
- [x] Architectures: x86_64, aarch64
- [ ] Distribution targets
  - [ ] RHEL 8 / Rocky Linux 8 / AlmaLinux 8
  - [ ] RHEL 9 / Rocky Linux 9 / AlmaLinux 9
  - [ ] Amazon Linux 2023
  - [ ] Fedora (latest 2 releases)
- [ ] Repository hosting (COPR or packagecloud.io)

### Alpine APK
- [ ] Alpine package build
- [ ] Target: Alpine 3.18+

### Static Binary Releases
- [x] musl-based static builds (already in release.yml)
- [ ] Portable tarball with systemd units

### Systemd Integration
- [x] cloud-init-local.service
- [x] cloud-init.service
- [x] cloud-config.service
- [x] cloud-final.service
- [x] cloud-init.target
- [x] Ordering dependencies

### Documentation
- [ ] User guide
- [ ] Migration guide from Python cloud-init
- [ ] Configuration reference
- [ ] Datasource documentation

---

# 100% Compatibility Target

The following phases extend beyond 80% to achieve full cloud-init compatibility.

## Phase 9: Extended Datasources
**Status: 🔴 Not Started**

### Remaining Cloud Providers
- [ ] CloudStack
- [ ] SmartOS (Joyent)
- [ ] VMware (GuestInfo, OVF)
- [ ] LXD
- [ ] MAAS
- [ ] Exoscale
- [ ] CloudSigma
- [ ] Bigstep
- [ ] IBMCloud
- [ ] UpCloud

### Specialized Datasources
- [ ] AliYun (Alibaba Cloud)
- [ ] RbxCloud
- [ ] Vagrant
- [ ] WSL (Windows Subsystem for Linux)
- [ ] NWCS (Nifty Cloud)

## Phase 10: Distribution-Specific Modules
**Status: 🔴 Not Started**

### Debian/Ubuntu Specific
- [ ] `apt_configure` - Full APT configuration
- [ ] `apt_pipelining` - APT pipelining settings
- [ ] `apt_source` - APT source management
- [ ] `grub_dpkg` - GRUB configuration
- [ ] `landscape` - Landscape integration
- [ ] `fan` - Ubuntu Fan networking
- [ ] `ubuntu_advantage` - Ubuntu Pro/Advantage
- [ ] `ubuntu_drivers` - Ubuntu drivers

### RHEL/CentOS Specific
- [ ] `rh_subscription` - Red Hat subscription
- [ ] `yum_add_repo` - Full YUM repo management

### SUSE Specific
- [ ] `zypper_add_repo` - Zypper repositories
- [ ] `zypper_configure` - Zypper configuration

### FreeBSD Specific
- [ ] FreeBSD network configuration
- [ ] FreeBSD package management

## Phase 11: Configuration Management Integration
**Status: 🔴 Not Started**

### Chef Integration
- [ ] `chef` module - Chef client bootstrap
- [ ] Chef validation key handling
- [ ] Chef environment configuration

### Puppet Integration
- [ ] `puppet` module - Puppet agent bootstrap
- [ ] Puppet certificate handling
- [ ] Puppet environment configuration

### Ansible Integration
- [ ] `ansible` module - Ansible pull mode
- [ ] Ansible playbook execution

### Salt Integration
- [ ] `salt_minion` module - Salt minion bootstrap
- [ ] Salt master configuration
- [ ] Salt grains and pillars

## Phase 12: Legacy & Deprecated Features
**Status: 🔴 Not Started**

### Legacy Init Systems
- [ ] `emit_upstart` - Upstart event emission
- [ ] SysV init script support

### Deprecated but Supported
- [ ] `mcollective` - MCollective configuration (deprecated)
- [ ] `rightscale_userdata` - RightScale format
- [ ] `byobu` - Byobu configuration

### Backwards Compatibility
- [ ] cloud-config v1 format quirks
- [ ] Legacy datasource formats
- [ ] Python cloud-init bug-for-bug compatibility mode

## Phase 13: Full Test Parity
**Status: 🔴 Not Started**

### Python cloud-init Test Suite
- [ ] Port all unit tests from Python cloud-init
- [ ] Port all integration tests
- [ ] Achieve 95%+ test coverage
- [ ] Fuzz testing for config parsing

### Certification Testing
- [ ] AWS certification tests
- [ ] Azure certification tests
- [ ] GCE certification tests
- [ ] OpenStack certification tests

### Edge Cases
- [ ] Malformed user-data handling (match Python behavior)
- [ ] Network timeout edge cases
- [ ] Filesystem permission edge cases
- [ ] Unicode handling parity

---

## Version Milestones

### v0.1.0 (Current)
- Basic project structure
- NoCloud and EC2 datasources
- Simple cloud-config parsing

### v0.1.1 (Next)
- GitHub repository setup
- CI/CD workflows (test, lint, build)
- Basic test coverage
- Release automation

### v0.2.0
- Full user/group management
- write_files with all encodings
- runcmd with proper error handling
- GCE and Azure datasources
- 60% test coverage

### v0.3.0
- Package management (apt, yum)
- Network configuration
- Disk/filesystem modules
- 70% test coverage

### v0.4.0
- Multi-part MIME support
- Cloud-config merging
- Jinja templating

### v1.0.0
- 80% module compatibility
- All major cloud datasources
- Production-ready packaging
- 80% test coverage
- Published to crates.io

### v1.5.0
- Extended datasources (VMware, LXD, MAAS, etc.)
- Distribution-specific modules (apt_configure, rh_subscription)
- 90% test coverage

### v2.0.0 (100% Compatibility)
- Configuration management integration (Chef, Puppet, Salt, Ansible)
- Legacy/deprecated feature support
- Full Python cloud-init test suite parity
- 95% test coverage
- Cloud provider certification
- Bug-for-bug compatibility mode
