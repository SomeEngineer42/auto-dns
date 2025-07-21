# Nix Development Environment

This directory contains Nix configuration files to set up a complete development environment for auto-dns.

## Prerequisites

- [Nix package manager](https://nixos.org/download.html) installed
- (Optional) [direnv](https://direnv.net/) for automatic environment activation

## Using Nix Flakes (Recommended)

```bash
# Enter development environment
nix develop

# Build the project
cargo build --release

# Run tests
cargo test

# Install and run locally
cargo run -- --help
```

## Using Traditional Nix

```bash
# Enter development shell
nix-shell

# All cargo commands are now available
cargo build
cargo test
```

## Automatic Environment Activation

If you have direnv installed:

```bash
# Allow direnv to activate the environment
direnv allow

# The environment will now automatically activate when entering the directory
```

## What's Included

The Nix environment provides:

- **Rust Toolchain**: Version 1.75.0 with all components
  - rustc, cargo
  - clippy for linting
  - rustfmt for formatting
  - rust-analyzer for IDE support
  
- **System Dependencies**:
  - OpenSSL development libraries
  - pkg-config
  - Platform-specific dependencies (Security framework on macOS)

- **Development Tools**:
  - cargo-watch for file watching
  - cargo-edit for dependency management
  - cargo-audit for security auditing
  - cargo-outdated for dependency updates
  - git, curl, jq for general development
  - docker and docker-compose for testing

## Building the Package

You can also build the auto-dns package directly with Nix:

```bash
# Build the package
nix build

# The binary will be available in result/bin/auto-dns
./result/bin/auto-dns --help
```

## Cross-Platform Support

The Nix configuration supports:
- Linux (x86_64, aarch64)
- macOS (x86_64, aarch64)
- Windows (via WSL)

Platform-specific dependencies are handled automatically.
