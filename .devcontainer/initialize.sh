#!/bin/bash

# Initialize script to ensure directories exist before mounting
set -e

echo "🔧 Initializing directories for devcontainer mounts..."

# Create .aws directory if it doesn't exist
mkdir -p "${HOME}/.aws"
echo "✅ Created ${HOME}/.aws directory"

# Set proper permissions
chmod 700 "${HOME}/.aws" 2>/dev/null || true

echo "✅ Directory initialization complete"
