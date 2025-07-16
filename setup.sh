#!/bin/bash

# auto-dns setup script
set -e

echo "🚀 Setting up auto-dns..."

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed. Please install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Check if AWS CLI is installed
if ! command -v aws &> /dev/null; then
    echo "⚠️  AWS CLI is not installed. You'll need to configure AWS credentials manually."
    echo "   Install AWS CLI: https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html"
fi

# Create config file if it doesn't exist
if [ ! -f "config.toml" ]; then
    echo "📝 Creating config file..."
    cp config.toml.example config.toml
    echo "✅ Created config.toml - please edit it with your domain and hosted zone ID"
fi

# Build the application
echo "🔨 Building auto-dns..."
cargo build --release

echo ""
echo "✅ Setup complete!"
echo ""
echo "Next steps:"
echo "1. Edit config.toml with your domain and AWS hosted zone ID"
echo "2. Configure AWS credentials (use AWS CLI, environment variables, or config file)"
echo "3. Run: ./target/release/auto-dns --once"
echo ""
echo "For package installation (DEB/RPM):"
echo "4. Run: ./build.sh --help"
echo ""
echo "For help, run: ./target/release/auto-dns --help"
