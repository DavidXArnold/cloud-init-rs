# cloud-init-rs

A safe Rust implementation of [cloud-init](https://github.com/canonical/cloud-init) focused on fast boot times and memory safety.

## Goals

- **Safety First**: No unsafe code (`#![forbid(unsafe_code)]`)
- **Fast Boot**: Optimized for minimal boot time overhead
- **80% Compatibility**: Support the most commonly used cloud-init features
- **Backwards Compatible**: Parse existing cloud-config YAML formats

## Features

### Supported Datasources

- [ ] NoCloud (local files, ISO)
- [ ] EC2 (AWS, compatible clouds)
- [ ] GCE (Google Cloud)
- [ ] Azure
- [ ] OpenStack

### Supported Modules

- [ ] `users` - Create and configure users
- [ ] `groups` - Create groups
- [ ] `write_files` - Write files with specified content
- [ ] `runcmd` - Execute commands
- [ ] `packages` - Install packages
- [ ] `ssh_authorized_keys` - Configure SSH keys
- [ ] `hostname` - Set system hostname
- [ ] `growpart` - Grow partitions
- [ ] `resize_rootfs` - Resize root filesystem

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
├── config/           # Cloud-config parsing
├── datasources/      # Metadata sources (EC2, GCE, etc.)
├── modules/          # Configuration modules
├── network/          # Network configuration
└── stages/           # Boot stages (local, network, config, final)
```

## License

Apache-2.0 (same as cloud-init)

## Contributing

Contributions are welcome! Please ensure:

1. No `unsafe` code
2. All tests pass
3. Code is formatted with `cargo fmt`
4. No clippy warnings
