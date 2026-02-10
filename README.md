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

## Building

```bash
cargo build --release
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
