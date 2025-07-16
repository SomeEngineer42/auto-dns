FROM mcr.microsoft.com/devcontainers/rust:1-1-bookworm

# Rename vscode user to SomeEngineer
RUN usermod -l SomeEngineer vscode && \
    groupmod -n SomeEngineer vscode && \
    usermod -d /home/SomeEngineer -m SomeEngineer && \
    ln -s /home/SomeEngineer /home/vscode

# Install additional dependencies
RUN apt-get update && apt-get install -y \
    curl \
    git \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Switch to the renamed user
USER SomeEngineer
WORKDIR /home/SomeEngineer

# Install Rust components
RUN rustup component add clippy rustfmt rust-src

# Pre-install cargo-watch for development
RUN cargo install cargo-watch
