# Contributing to cloud-init-rs

Thank you for your interest in contributing to cloud-init-rs!

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/cloud-init-rs.git`
3. Create a branch: `git checkout -b my-feature`
4. Make your changes
5. Run tests: `cargo test`
6. Push and create a Pull Request

## Development Setup

### Prerequisites
- Rust 1.85 or later
- cargo

### Building
```bash
cargo build
```

### Testing
```bash
cargo test
```

### Linting
```bash
cargo fmt --check
cargo clippy -- -D warnings
```

## Code Style

- Run `cargo fmt` before committing
- Fix all `cargo clippy` warnings
- Use `tracing` for logging, not `println!`
- No `unsafe` code (enforced by `#![forbid(unsafe_code)]`)
- Write tests for new functionality

## Commit Messages

Use clear, descriptive commit messages:
- `feat: add GCE datasource support`
- `fix: handle empty user-data gracefully`
- `docs: update README with new CLI options`
- `test: add integration tests for write_files`
- `refactor: simplify config parsing logic`

## Pull Request Process

1. Update documentation if you're changing behavior
2. Add tests for new features
3. Ensure all CI checks pass
4. Request review from maintainers

## Adding a New Datasource

1. Create `src/datasources/<name>.rs`
2. Implement the `Datasource` trait
3. Add to detection in `src/datasources/mod.rs`
4. Add tests with mocked HTTP responses (use `wiremock`)
5. Update README and ROADMAP

## Adding a New Module

1. Create `src/modules/<name>.rs`
2. Export from `src/modules/mod.rs`
3. Add corresponding fields to `CloudConfig` struct
4. Call from the appropriate stage
5. Add tests

## Reporting Bugs

Please include:
- cloud-init-rs version
- Operating system
- Cloud provider (if applicable)
- Steps to reproduce
- Expected vs actual behavior
- Relevant logs

## Questions?

Open a GitHub Discussion or Issue.
