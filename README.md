<p align="center">
  <img src="assets/cloud-init-rs-logo.png" alt="Cloud-init-rs Logo">
</p>

# cloud-init-rs

A safe Rust implementation of [cloud-init](https://github.com/canonical/cloud-init) focused on fast boot times and memory safety.

## Goals

- **Safety First**: No unsafe code (`#![forbid(unsafe_code)]`)
- **Fast Boot**: Optimized for minimal boot time overhead
- **80% Compatibility**: Support the most commonly used cloud-init features
- **Backwards Compatible**: Parse existing cloud-config YAML formats

## Current Status

**Phase 6 Complete** - Advanced features implemented. See [ROADMAP.md](ROADMAP.md) for full details.

## Features

### Supported Datasources

- [x] NoCloud (local files, ISO)
- [x] EC2 (AWS, compatible clouds) - IMDSv1 and IMDSv2
- [x] GCE (Google Cloud)
- [x] Azure (IMDS)
- [x] OpenStack (config-drive and metadata service)

### Supported Modules

- [x] `users` - Create and configure users with SSH keys, sudo, groups
- [x] `groups` - Create groups with members
- [x] `write_files` - Write files with base64/gzip encoding support
- [x] `runcmd` - Execute commands (shell strings and arg arrays)
- [x] `bootcmd` - Early boot commands
- [x] `packages` - Install packages (apt/dnf/yum/zypper/apk)
- [x] `package_update` - Update package cache
- [x] `package_upgrade` - Upgrade installed packages
- [x] `ssh_authorized_keys` - Configure SSH keys
- [x] `hostname` - Set system hostname with FQDN and /etc/hosts
- [x] `timezone` - Set system timezone
- [x] `locale` - Set system locale
- [x] `ntp` - Configure NTP (chrony/timesyncd/ntpd)
- [ ] `growpart` - Grow partitions (planned)
- [ ] `resize_rootfs` - Resize root filesystem (planned)

### Network Configuration

- [x] Network config v1 (legacy format) parsing
- [x] Network config v2 (Netplan format) parsing
- [x] Bonds, bridges, VLANs, static routes
- [x] Renderer: systemd-networkd
- [x] Renderer: NetworkManager  
- [x] Renderer: Debian ENI (/etc/network/interfaces)

### Advanced Features

- [x] MIME multipart user-data parsing
- [x] Cloud-config merging (cloud.cfg + cloud.cfg.d/*.cfg + user-data)
- [x] Jinja2 templating with instance metadata
- [x] Instance state management (/var/lib/cloud structure)
- [x] Semaphore-based execution control (per-instance, per-boot, per-once)

## Installation

### Debian/Ubuntu (.deb)

Download the latest `.deb` package from the [releases page](https://github.com/DavidXArnold/cloud-init-rs/releases):

```bash
# For amd64 (x86_64)
wget https://github.com/DavidXArnold/cloud-init-rs/releases/latest/download/cloud-init-rs_VERSION_amd64.deb
sudo dpkg -i cloud-init-rs_VERSION_amd64.deb

# For arm64 (aarch64)
wget https://github.com/DavidXArnold/cloud-init-rs/releases/latest/download/cloud-init-rs_VERSION_arm64.deb
sudo dpkg -i cloud-init-rs_VERSION_arm64.deb
```

The package automatically enables the systemd services.

### RHEL/Fedora/CentOS (.rpm)

Download the latest `.rpm` package from the [releases page](https://github.com/DavidXArnold/cloud-init-rs/releases):

```bash
# For x86_64
wget https://github.com/DavidXArnold/cloud-init-rs/releases/latest/download/cloud-init-rs-VERSION-1.x86_64.rpm
sudo rpm -i cloud-init-rs-VERSION-1.x86_64.rpm

# For aarch64
wget https://github.com/DavidXArnold/cloud-init-rs/releases/latest/download/cloud-init-rs-VERSION-1.aarch64.rpm
sudo rpm -i cloud-init-rs-VERSION-1.aarch64.rpm
```

### From Source

```bash
cargo install --path .
```

Or build manually:

```bash
cargo build --release
sudo cp target/release/cloud-init-rs /usr/bin/
```

## Usage

```bash
# Run all stages (equivalent to cloud-init init && cloud-init modules)
cloud-init-rs init

# Run individual stages
cloud-init-rs local    # Pre-network stage
cloud-init-rs network  # Post-network stage
cloud-init-rs config   # Configuration stage
cloud-init-rs final    # Final stage (user scripts)

# Query metadata
cloud-init-rs query instance-id

# Check status
cloud-init-rs status
```

The release binary is optimized for size and speed with LTO enabled.

## Configuration

cloud-init-rs reads configuration from the same locations as cloud-init:

- `/etc/cloud/cloud.cfg`
- `/etc/cloud/cloud.cfg.d/*.cfg`
- User data from datasource

### Example cloud-config

```yaml
#cloud-config
hostname: my-instance
users:
  - name: admin
    groups: sudo
    shell: /bin/bash
    ssh_authorized_keys:
      - ssh-rsa AAAA...

write_files:
  - path: /etc/motd
    content: |
      Welcome to my instance!

runcmd:
  - echo "Hello from cloud-init-rs"
```

## Architecture

```
src/
├── main.rs           # CLI entry point
├── lib.rs            # Library exports
├── error.rs          # Error types
├── config/           # Cloud-config parsing and merging
│   ├── loader.rs     # Config loading from standard locations
│   └── merge.rs      # Config merging logic
├── datasources/      # Metadata sources
│   ├── ec2.rs        # AWS EC2 (IMDSv1/v2)
│   ├── gce.rs        # Google Cloud
│   ├── azure.rs      # Microsoft Azure
│   ├── openstack.rs  # OpenStack
│   └── nocloud.rs    # NoCloud (local/ISO)
├── modules/          # Configuration modules
│   ├── users.rs      # User creation
│   ├── groups.rs     # Group creation
│   ├── hostname.rs   # Hostname configuration
│   ├── packages.rs   # Package management
│   ├── write_files.rs# File writing
│   └── ...           # Other modules
├── network/          # Network configuration
│   ├── mod.rs        # V2 (Netplan) parsing
│   ├── v1.rs         # V1 (legacy) parsing
│   └── render/       # Renderers (networkd, NM, ENI)
├── stages/           # Boot stages
│   ├── local.rs      # Pre-network (disk, network config)
│   ├── network.rs    # Post-network (metadata fetch)
│   ├── config.rs     # Configuration application
│   └── final_stage.rs# User scripts
├── state/            # Instance state management
│   ├── paths.rs      # /var/lib/cloud paths
│   └── semaphore.rs  # Execution frequency control
├── template/         # Jinja2 templating
└── userdata/         # User-data parsing (MIME, types)
```

## License

Apache-2.0 (same as cloud-init)

## Contributing

Contributions are welcome! Please ensure:

1. No `unsafe` code
2. All tests pass
3. Code is formatted with `cargo fmt`
4. No clippy warnings
