#!/bin/bash

# auto-dns installer script
# Usage: curl -fsSL https://raw.githubusercontent.com/SomeEngineer42/auto-dns/main/install.sh | bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
GITHUB_REPO="SomeEngineer42/auto-dns"
GITHUB_URL="https://github.com/${GITHUB_REPO}"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/auto-dns"
SYSTEMD_DIR="/etc/systemd/system"
SERVICE_USER="auto-dns"
BUILD_IMAGE="auto-dns-builder"

echo -e "${BLUE}🚀 Auto-DNS Installer${NC}"
echo -e "Installing auto-dns from ${GITHUB_URL}"
echo ""

# Check if running as root for system installation
if [[ $EUID -eq 0 ]]; then
    echo -e "${GREEN}✅ Running as root - can install system-wide${NC}"
    SUDO=""
else
    echo -e "${YELLOW}⚠️  Not running as root - will use sudo for system operations${NC}"
    SUDO="sudo"

    # Check if sudo is available
    if ! command -v sudo &> /dev/null; then
        echo -e "${RED}❌ sudo is required but not available. Please run as root or install sudo.${NC}"
        exit 1
    fi
fi

# Function to print status
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Check if Docker is available
print_status "Checking Docker availability..."
if ! command -v docker &> /dev/null; then
    print_error "Docker is not installed or not in PATH"
    echo "Please install Docker first:"
    echo "  - On Ubuntu/Debian: sudo apt-get install docker.io"
    echo "  - On CentOS/RHEL: sudo yum install docker"
    echo "  - On macOS: Install Docker Desktop"
    echo "  - Or visit: https://docs.docker.com/get-docker/"
    exit 1
fi

# Check if Docker daemon is running
if ! docker info &> /dev/null; then
    print_error "Docker daemon is not running"
    echo "Please start Docker:"
    echo "  sudo systemctl start docker"
    echo "  # or start Docker Desktop on macOS/Windows"
    exit 1
fi

# Check if current user can run Docker
if ! docker ps &> /dev/null; then
    print_error "Current user cannot run Docker commands"
    echo "Please add your user to the docker group:"
    echo "  sudo usermod -aG docker \$USER"
    echo "  # Then log out and log back in"
    echo ""
    echo "Or run this script with sudo if you prefer system-wide installation"
    exit 1
fi

print_success "Docker is available and accessible"

# Create temporary build directory
BUILD_DIR=$(mktemp -d)
trap "rm -rf ${BUILD_DIR}" EXIT

print_status "Creating build environment..."

# Create Dockerfile for building
cat > "${BUILD_DIR}/Dockerfile" << 'EOF'
# Multi-stage Docker build using Nix
FROM nixos/nix:2.18.1 as builder

# Enable experimental features for Nix
RUN echo "experimental-features = nix-command flakes" > /etc/nix/nix.conf

# Install git for fetching repository
RUN nix profile install nixpkgs#git

# Create app directory
WORKDIR /app

# Clone the repository
ARG GITHUB_REPO
RUN git clone https://github.com/${GITHUB_REPO}.git .

# Build using Nix (will use flake.nix configuration)
RUN nix build

# Create output directory and copy binary from Nix result
RUN mkdir -p /output && cp result/bin/auto-dns /output/
EOF

# Build the auto-dns binary
print_status "Building auto-dns in Docker container with Nix..."
docker build \
	--build-arg GITHUB_REPO="${GITHUB_REPO}" \
	-t "${BUILD_IMAGE}" \
	"${BUILD_DIR}" || {
	print_error "Failed to build auto-dns"
	return 1
}

# Extract the binary from the container
print_status "Extracting built binary..."
CONTAINER_ID=$(docker create "${BUILD_IMAGE}")
docker cp "${CONTAINER_ID}:/output/auto-dns" "${BUILD_DIR}/auto-dns"
docker rm "${CONTAINER_ID}" > /dev/null
print_success "Built auto-dns in Docker with Nix"

# Verify binary was created
if [[ ! -f "${BUILD_DIR}/auto-dns" ]]; then
    print_error "Binary was not created successfully"
    exit 1
fi

print_success "Binary built successfully"

# Install binary
print_status "Installing binary to ${INSTALL_DIR}..."
$SUDO mkdir -p "${INSTALL_DIR}"
$SUDO cp "${BUILD_DIR}/auto-dns" "${INSTALL_DIR}/auto-dns"
$SUDO chmod +x "${INSTALL_DIR}/auto-dns"

print_success "Binary installed to ${INSTALL_DIR}/auto-dns"

# Create configuration directory
print_status "Creating configuration directory..."
$SUDO mkdir -p "${CONFIG_DIR}"

# Interactive configuration
echo ""
echo -e "${BLUE}📝 Configuration Setup${NC}"
echo "Please provide the following information for your DNS configuration:"
echo ""

# Collect configuration information
read -p "Domain name to update (e.g., home.example.com): " DOMAIN_NAME
while [[ -z "$DOMAIN_NAME" ]]; do
    print_warning "Domain name cannot be empty"
    read -p "Domain name to update (e.g., home.example.com): " DOMAIN_NAME
done

read -p "AWS Route53 Hosted Zone ID (e.g., Z1234567890ABC): " HOSTED_ZONE_ID
while [[ -z "$HOSTED_ZONE_ID" ]]; do
    print_warning "Hosted Zone ID cannot be empty"
    read -p "AWS Route53 Hosted Zone ID: " HOSTED_ZONE_ID
done

read -p "DNS record TTL in seconds [300]: " TTL
TTL=${TTL:-300}

# AWS Credentials
echo ""
echo -e "${YELLOW}⚙️  AWS Credentials Configuration${NC}"
echo "Please provide your AWS credentials for Route53 access:"
echo ""

read -p "AWS Access Key ID: " AWS_ACCESS_KEY_ID
while [[ -z "$AWS_ACCESS_KEY_ID" ]]; do
    print_warning "AWS Access Key ID cannot be empty"
    read -p "AWS Access Key ID: " AWS_ACCESS_KEY_ID
done

read -s -p "AWS Secret Access Key: " AWS_SECRET_ACCESS_KEY
echo ""
while [[ -z "$AWS_SECRET_ACCESS_KEY" ]]; do
    print_warning "AWS Secret Access Key cannot be empty"
    read -s -p "AWS Secret Access Key: " AWS_SECRET_ACCESS_KEY
    echo ""
done

read -p "AWS Default Region [us-east-1]: " AWS_DEFAULT_REGION
AWS_DEFAULT_REGION=${AWS_DEFAULT_REGION:-us-east-1}

# Create configuration file
print_status "Creating configuration file..."
cat > "${BUILD_DIR}/config.yaml" << EOF
records:
  - name: "${DOMAIN_NAME}"
    hosted_zone_id: "${HOSTED_ZONE_ID}"
    ttl: ${TTL}

aws:
  access_key_id: "${AWS_ACCESS_KEY_ID}"
  secret_access_key: "${AWS_SECRET_ACCESS_KEY}"
  region: "${AWS_DEFAULT_REGION}"
EOF

$SUDO cp "${BUILD_DIR}/config.yaml" "${CONFIG_DIR}/config.yaml"

# Create systemd service file
print_status "Creating systemd service..."

cat > "${BUILD_DIR}/auto-dns.service" << EOF
[Unit]
Description=Auto DNS Updater for AWS Route53
After=network-online.target
Wants=network-online.target
Documentation=https://github.com/${GITHUB_REPO}

[Service]
Type=simple
User=${SERVICE_USER}
Group=${SERVICE_USER}
WorkingDirectory=${CONFIG_DIR}
ExecStart=${INSTALL_DIR}/auto-dns --config ${CONFIG_DIR}/config.yaml
Restart=always
RestartSec=10

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=${CONFIG_DIR}
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true

[Install]
WantedBy=multi-user.target
EOF

# Create service user
print_status "Creating service user..."
if ! id "${SERVICE_USER}" &>/dev/null; then
    $SUDO useradd --system --no-create-home --shell /bin/false "${SERVICE_USER}"
    print_success "Created service user: ${SERVICE_USER}"
else
    print_warning "Service user ${SERVICE_USER} already exists"
fi

# Set configuration file ownership and secure permissions
$SUDO chown -R "${SERVICE_USER}:${SERVICE_USER}" "${CONFIG_DIR}"
$SUDO chmod 600 "${CONFIG_DIR}/config.yaml"  # Protect AWS credentials

$SUDO cp "${BUILD_DIR}/auto-dns.service" "${SYSTEMD_DIR}/auto-dns.service"

# Reload systemd and enable service
print_status "Configuring systemd service..."
$SUDO systemctl daemon-reload
$SUDO systemctl enable auto-dns.service

print_success "Systemd service created and enabled"

# Clean up Docker image
print_status "Cleaning up build artifacts..."
docker rmi "${BUILD_IMAGE}" > /dev/null 2>&1 || true

# Final instructions
echo ""
echo -e "${GREEN}🎉 Installation Complete!${NC}"
echo ""
echo "Auto-DNS has been installed and configured:"
echo -e "  • Binary: ${GREEN}${INSTALL_DIR}/auto-dns${NC}"
echo -e "  • Config: ${GREEN}${CONFIG_DIR}/config.yaml${NC}"
echo -e "  • Service: ${GREEN}auto-dns.service${NC}"
echo ""
echo "Next steps:"
echo -e "${BLUE}1.${NC} Test the configuration:"
echo "   sudo -u ${SERVICE_USER} ${INSTALL_DIR}/auto-dns --config ${CONFIG_DIR}/config.yaml --once"
echo -e "${BLUE}2.${NC} Start the service:"
echo "   sudo systemctl start auto-dns"
echo -e "${BLUE}3.${NC} Check service status:"
echo "   sudo systemctl status auto-dns"
echo -e "${BLUE}4.${NC} View logs:"
echo "   sudo journalctl -u auto-dns -f"
echo ""
echo "The service will automatically start on boot and update your DNS record every 5 minutes."
echo ""
echo -e "${YELLOW}Note:${NC} Make sure your AWS credentials have the required Route53 permissions."
echo "Documentation: ${GITHUB_URL}"
