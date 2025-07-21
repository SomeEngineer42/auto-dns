# Legacy shell.nix for compatibility
# Use `nix-shell` or `nix develop` to enter the development environment

{ pkgs ? import <nixpkgs> { overlays = [ (import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz")) ]; } }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    # Rust toolchain - latest stable version
    (rust-bin.stable.latest.default.override {
      extensions = [ "rust-src" "clippy" "rustfmt" "rust-analyzer" ];
    })

    # Build dependencies
    pkg-config
    openssl

    # Development tools
    cargo-watch
    cargo-edit
    cargo-audit
    git
    curl
    jq
    docker
    docker-compose
  ] ++ lib.optionals stdenv.isDarwin [
    # macOS specific dependencies
    darwin.apple_sdk.frameworks.Security
    darwin.apple_sdk.frameworks.SystemConfiguration
  ];

  shellHook = ''
    echo "🦀 Rust development environment for auto-dns (via shell.nix)"
    echo "Rust version: $(rustc --version)"
    echo ""
    echo "Available commands:"
    echo "  cargo build - Build the project"
    echo "  cargo test  - Run tests"
    echo "  cargo run   - Run the application"
    echo ""
  '';
}
