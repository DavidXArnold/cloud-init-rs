# cloud-init-rs Roadmap

This roadmap outlines the path to achieving 80%+ compatibility with cloud-init.

## Phase 1: Core Infrastructure (Current)
**Status: ✅ Complete**

- [x] Project structure and build system
- [x] CLI with subcommands (init, local, network, config, final)
- [x] Stage execution framework
- [x] Error handling with `thiserror`
- [x] Logging with `tracing`
- [x] Cloud-config YAML parsing basics

## Phase 2: Test Infrastructure (High Priority)
**Status: 🔴 Not Started**

Test coverage is critical for a system-level tool. Tests should be written alongside features.

### Unit Tests
- [ ] Cloud-config parsing tests
  - [ ] Valid YAML parsing
  - [ ] Malformed YAML handling
  - [ ] All config field types
  - [ ] Edge cases (empty, comments-only)
- [ ] Datasource tests
  - [ ] NoCloud file parsing
  - [ ] EC2 IMDS response parsing
  - [ ] Datasource detection logic
- [ ] Module tests
  - [ ] User creation commands
  - [ ] File writing with permissions
  - [ ] Command execution

### Integration Tests
- [ ] Mock HTTP server tests (wiremock)
  - [ ] EC2 IMDS mock responses
  - [ ] Timeout handling
  - [ ] Error responses (404, 500)
- [ ] Filesystem tests (tempdir)
  - [ ] NoCloud seed directory
  - [ ] write_files output
  - [ ] Permission verification
- [ ] End-to-end stage tests
  - [ ] Local stage with NoCloud
  - [ ] Config stage with mock data

### Test Utilities
- [ ] Test fixtures for cloud-config samples
- [ ] Mock datasource implementation
- [ ] Tempdir helper for filesystem tests
- [ ] Assertion helpers for file content/permissions

### Coverage Goals
- [ ] 80% line coverage for `src/config/`
- [ ] 80% line coverage for `src/datasources/`
- [ ] 70% line coverage for `src/modules/`
- [ ] CI integration with coverage reporting

## Phase 3: Datasources
**Status: 🔄 In Progress**

### High Priority (80% of cloud deployments)
- [x] NoCloud (local files, ISO)
- [x] EC2 (AWS) - IMDSv1 and IMDSv2
- [ ] GCE (Google Cloud Platform)
- [ ] Azure (IMDS)
- [ ] OpenStack (config-drive and metadata service)

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

## Phase 4: Configuration Modules
**Status: 🔄 In Progress**

### Users and Groups (High Priority)
- [ ] `users` - Create users with full options
  - [ ] Default user creation
  - [ ] SSH key injection
  - [ ] sudo configuration
  - [ ] Password hashing
  - [ ] Group membership
- [ ] `groups` - Create groups
- [ ] `ssh_authorized_keys` - Root and user keys

### Files and Content (High Priority)
- [ ] `write_files` - Write arbitrary files
  - [x] Basic file writing
  - [x] Base64 decoding
  - [ ] Gzip decompression
  - [ ] Permissions and ownership
  - [ ] Deferred writes
- [ ] `bootcmd` - Early boot commands
- [ ] `runcmd` - Late boot commands
  - [x] Basic command execution
  - [ ] Shell selection
  - [ ] Error handling modes

### System Configuration (High Priority)
- [ ] `hostname` - Set hostname
  - [x] Basic implementation
  - [ ] FQDN handling
  - [ ] /etc/hosts management
- [ ] `timezone` - Set timezone
- [ ] `locale` - Set system locale
- [ ] `keyboard` - Keyboard layout
- [ ] `ntp` - NTP configuration

### Disk and Filesystem (Medium Priority)
- [ ] `growpart` - Grow partitions
- [ ] `resize_rootfs` - Resize root filesystem
- [ ] `mounts` - Configure mounts
- [ ] `disk_setup` - Partition disks
- [ ] `fs_setup` - Create filesystems

### Package Management (Medium Priority)
- [ ] `packages` - Install packages
- [ ] `package_update` - Update package cache
- [ ] `package_upgrade` - Upgrade packages
- [ ] `apt` - APT-specific config
- [ ] `yum_repos` - YUM repositories
- [ ] `snap` - Snap packages

### Network Configuration (Medium Priority)
- [ ] Network config v1 parsing
- [ ] Network config v2 (Netplan) parsing
- [ ] Renderer: networkd
- [ ] Renderer: NetworkManager
- [ ] Renderer: ENI (Debian)
- [ ] Static IP configuration
- [ ] DHCP configuration
- [ ] Bonding and VLANs

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

## Phase 5: Advanced Features

### Multi-part MIME
- [ ] Parse multipart user-data
- [ ] cloud-config merging
- [ ] Include directives
- [ ] cloud-boothook support

### Jinja Templating
- [ ] Instance metadata variables
- [ ] ds (datasource) variables
- [ ] v1 data variables

### Cloud-config Merging
- [ ] /etc/cloud/cloud.cfg
- [ ] /etc/cloud/cloud.cfg.d/*.cfg
- [ ] User-data cloud-config
- [ ] Vendor-data

### Instance State
- [ ] /var/lib/cloud directory structure
- [ ] Instance ID tracking
- [ ] Per-instance vs per-boot markers
- [ ] sem/ semaphore files

## Phase 6: Compatibility Validation

### Compatibility Testing
- [ ] cloud-init test suite adaptation
- [ ] Parity testing with Python cloud-init
- [ ] Real VM tests (QEMU/KVM)
- [ ] Container tests (Docker, Podman)
- [ ] Cloud provider testing (AWS, GCE, Azure)

### Regression Testing
- [ ] Automated nightly test runs
- [ ] Performance benchmarks vs Python cloud-init
- [ ] Boot time measurements

## Phase 7: Production Readiness

### Packaging
- [ ] Debian/Ubuntu packages
- [ ] RPM packages (RHEL, Fedora)
- [ ] Alpine APK
- [ ] Static binary releases

### Systemd Integration
- [ ] cloud-init-local.service
- [ ] cloud-init.service
- [ ] cloud-config.service
- [ ] cloud-final.service
- [ ] Ordering dependencies

### Documentation
- [ ] User guide
- [ ] Migration guide from Python cloud-init
- [ ] Configuration reference
- [ ] Datasource documentation

## Non-Goals (Out of Scope)

The following features are explicitly out of scope for the 80% compatibility target:

- **Chef/Puppet/Salt integration** - Use runcmd instead
- **Landscape integration** - Ubuntu-specific
- **Fan networking** - Ubuntu-specific
- **apt_pipelining** - Too distribution-specific
- **byobu** - Interactive tool, not boot-time
- **emit_upstart** - Legacy init system
- **grub_dpkg** - Debian-specific
- **mcollective** - Deprecated

## Version Milestones

### v0.1.0 (Current)
- Basic project structure
- NoCloud and EC2 datasources
- Simple cloud-config parsing

### v0.2.0 (Next)
- Full user/group management
- write_files with all encodings
- runcmd with proper error handling
- GCE and Azure datasources

### v0.3.0
- Package management (apt, yum)
- Network configuration
- Disk/filesystem modules

### v0.4.0
- Multi-part MIME support
- Cloud-config merging
- Jinja templating

### v1.0.0
- 80% module compatibility
- All major cloud datasources
- Production-ready packaging
- Comprehensive testing
