{
  description = "auto-dns - AWS Route53 DNS updater in Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        
        # Use the same Rust version as specified in rust-toolchain.toml
        rustToolchain = pkgs.rust-bin.stable."1.75.0".default.override {
          extensions = [ "rust-src" "clippy" "rustfmt" "rust-analyzer" ];
        };

        # Native dependencies needed for building
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
        ];

        # Runtime dependencies
        buildInputs = with pkgs; [
          openssl
        ] ++ lib.optionals stdenv.isDarwin [
          # macOS specific dependencies
          darwin.apple_sdk.frameworks.Security
          darwin.apple_sdk.frameworks.SystemConfiguration
        ];

        # Development tools
        devTools = with pkgs; [
          # Rust development
          cargo-watch
          cargo-edit
          cargo-audit
          cargo-outdated
          
          # General development tools
          git
          curl
          jq
          
          # For running the install script
          docker
          docker-compose
        ];

      in
      {
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;
          packages = devTools;
          
          # Environment variables
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          
          # Shell hook to set up the environment
          shellHook = ''
            echo "🦀 Rust development environment for auto-dns"
            echo "Rust version: $(rustc --version)"
            echo "Cargo version: $(cargo --version)"
            echo ""
            echo "Available commands:"
            echo "  cargo build          - Build the project"
            echo "  cargo test           - Run tests"
            echo "  cargo run            - Run the application"
            echo "  cargo watch -x test  - Watch and test on changes"
            echo "  ./install.sh         - Install using Docker"
            echo ""
          '';
        };

        # Package definition for building auto-dns
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "auto-dns";
          version = "0.1.0";
          
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          inherit nativeBuildInputs buildInputs;

          # Skip tests during build (they can be run separately)
          doCheck = false;

          meta = with pkgs.lib; {
            description = "AWS Route53 DNS updater that keeps DNS records in sync with public IP";
            homepage = "https://github.com/SomeEngineer42/auto-dns";
            license = licenses.mit;
            maintainers = [ ];
            platforms = platforms.unix;
          };
        };

        # Formatter for nix files
        formatter = pkgs.nixpkgs-fmt;
      });
}
