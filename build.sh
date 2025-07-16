#!/bin/bash

# auto-dns package build script
# Creates DEB and RPM packages for Debian/Ubuntu and Fedora/RHEL distributions

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Package metadata
PACKAGE_NAME="auto-dns"
VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
DESCRIPTION="Automatically update AWS Route53 DNS records with current public IP"
MAINTAINER="SomeEngineer42"
LICENSE="MIT"
HOMEPAGE="https://github.com/SomeEngineer42/auto-dns"

echo -e "${BLUE}🚀 Building ${PACKAGE_NAME} v${VERSION} packages...${NC}"

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to install cargo packaging tools
install_cargo_tools() {
    echo -e "${YELLOW}📦 Installing packaging tools...${NC}"
    
    if ! command_exists cargo-deb; then
        echo "Installing cargo-deb..."
        cargo install cargo-deb
    fi
    
    if ! command_exists cargo-rpm; then
        echo "Installing cargo-rpm..."
        cargo install cargo-rpm
    fi
}

# Function to check for system dependencies
check_dependencies() {
    echo -e "${YELLOW}🔍 Checking dependencies...${NC}"
    
    local missing_deps=()
    
    # Check for Rust/Cargo
    if ! command_exists cargo; then
        missing_deps+=("cargo (Rust toolchain)")
    fi
    
    # Check for rpmbuild (needed for RPM packages)
    if ! command_exists rpmbuild; then
        echo -e "${YELLOW}⚠️  rpmbuild not found. Installing rpm-build...${NC}"
        if command_exists dnf; then
            sudo dnf install -y rpm-build rpm-devel
        elif command_exists yum; then
            sudo yum install -y rpm-build rpm-devel
        elif command_exists apt; then
            sudo apt update && sudo apt install -y rpm
        else
            missing_deps+=("rpmbuild (install rpm-build package)")
        fi
    fi
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        echo -e "${RED}❌ Missing dependencies:${NC}"
        printf '%s\n' "${missing_deps[@]}"
        exit 1
    fi
}

# Function to add package metadata to Cargo.toml
setup_package_metadata() {
    echo -e "${YELLOW}📝 Setting up package metadata...${NC}"
    
    # Check if [package.metadata.deb] already exists
    if ! grep -q "^\[package\.metadata\.deb\]" Cargo.toml; then
        cat >> Cargo.toml << EOF

[package.metadata.deb]
maintainer = "${MAINTAINER}"
copyright = "2025, ${MAINTAINER} <auto-dns@example.com>"
license-file = ["LICENSE", "4"]
extended-description = """
Auto-DNS is a robust, Rust-based application that automatically updates 
AWS Route53 DNS records with your current public IP address. Perfect for 
dynamic DNS setups where you need to keep DNS records pointing to your 
home IP or other dynamic endpoints.

Features:
- Fast and reliable Rust implementation
- Multiple IP detection services for redundancy  
- Flexible TOML-based configuration
- Docker support with Docker Compose
- Systemd service integration
- Comprehensive logging and error handling
"""
depends = "\$auto"
section = "net"
priority = "optional"
assets = [
    ["target/release/auto-dns", "usr/bin/", "755"],
    ["config.toml.example", "etc/auto-dns/", "644"],
    ["README.md", "usr/share/doc/auto-dns/", "644"],
    ["LICENSE", "usr/share/doc/auto-dns/", "644"],
]
conf-files = ["/etc/auto-dns/config.toml.example"]
maintainer-scripts = "debian/"
systemd-units = { enable = false }

[package.metadata.rpm]
package = "auto-dns"
version = "${VERSION}"
license = "MIT"
summary = "Automatic DNS updater for AWS Route53"
description = """
Auto-DNS automatically updates AWS Route53 DNS records with your current 
public IP address. Built in Rust for performance and reliability.
"""
[package.metadata.rpm.cargo]
buildflags = ["--release"]
[package.metadata.rpm.targets]
auto-dns = { path = "/usr/bin/auto-dns" }
EOF
    fi
}

# Function to create debian maintainer scripts directory
setup_debian_scripts() {
    echo -e "${YELLOW}📝 Setting up Debian maintainer scripts...${NC}"
    
    mkdir -p debian
    
    # Create postinst script
    cat > debian/postinst << 'EOF'
#!/bin/bash
set -e

# Create auto-dns user if it doesn't exist
if ! id "auto-dns" &>/dev/null; then
    useradd --system --home-dir /var/lib/auto-dns --create-home --shell /bin/false auto-dns
fi

# Create config directory
mkdir -p /etc/auto-dns
chown root:auto-dns /etc/auto-dns
chmod 750 /etc/auto-dns

# Set up systemd service if systemctl is available
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload
    echo "Run 'sudo systemctl enable auto-dns' to enable the service"
    echo "Run 'sudo systemctl start auto-dns' to start the service"
fi

echo "Auto-DNS installed successfully!"
echo "Edit /etc/auto-dns/config.toml.example and save as config.toml to configure"
EOF

    # Create prerm script
    cat > debian/prerm << 'EOF'
#!/bin/bash
set -e

# Stop and disable service if running
if command -v systemctl >/dev/null 2>&1; then
    if systemctl is-active --quiet auto-dns; then
        systemctl stop auto-dns
    fi
    if systemctl is-enabled --quiet auto-dns; then
        systemctl disable auto-dns
    fi
fi
EOF

    # Create postrm script
    cat > debian/postrm << 'EOF'
#!/bin/bash
set -e

case "$1" in
    purge)
        # Remove user and home directory
        if id "auto-dns" &>/dev/null; then
            userdel auto-dns
        fi
        
        # Remove config directory if empty
        if [ -d /etc/auto-dns ]; then
            rmdir /etc/auto-dns 2>/dev/null || true
        fi
        
        # Remove systemd service files
        rm -f /etc/systemd/system/auto-dns.service
        if command -v systemctl >/dev/null 2>&1; then
            systemctl daemon-reload
        fi
        ;;
esac
EOF

    chmod +x debian/postinst debian/prerm debian/postrm
}

# Function to create systemd service file
create_systemd_service() {
    echo -e "${YELLOW}📝 Creating systemd service file...${NC}"
    
    mkdir -p systemd
    
    cat > systemd/auto-dns.service << 'EOF'
[Unit]
Description=Auto DNS Updater for AWS Route53
Documentation=https://github.com/SomeEngineer42/auto-dns
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=auto-dns
Group=auto-dns
ExecStart=/usr/bin/auto-dns --config /etc/auto-dns/config.toml --interval 300
Restart=always
RestartSec=10

# Security settings
NoNewPrivileges=true
ProtectHome=true
ProtectSystem=strict
ReadWritePaths=/var/log

# Environment for AWS credentials
Environment=AWS_CONFIG_FILE=/etc/auto-dns/.aws/config
Environment=AWS_SHARED_CREDENTIALS_FILE=/etc/auto-dns/.aws/credentials

[Install]
WantedBy=multi-user.target
EOF
}

# Function to build release binary
build_release() {
    echo -e "${YELLOW}🔨 Building release binary...${NC}"
    cargo build --release
    
    # Verify binary was created
    if [ ! -f "target/release/auto-dns" ]; then
        echo -e "${RED}❌ Failed to build release binary${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✅ Release binary built successfully${NC}"
}

# Function to build DEB package
build_deb() {
    echo -e "${YELLOW}📦 Building DEB package...${NC}"
    
    cargo deb --no-build
    
    # Find the generated DEB file
    DEB_FILE=$(find target/debian -name "*.deb" | head -1)
    
    if [ -n "$DEB_FILE" ]; then
        echo -e "${GREEN}✅ DEB package created: ${DEB_FILE}${NC}"
        
        # Copy to current directory for easy access
        cp "$DEB_FILE" .
        echo -e "${GREEN}📁 Copied to: $(basename "$DEB_FILE")${NC}"
        
        # Show package info
        echo -e "${BLUE}📋 Package information:${NC}"
        dpkg-deb --info "$(basename "$DEB_FILE")" || true
    else
        echo -e "${RED}❌ Failed to create DEB package${NC}"
        return 1
    fi
}

# Function to build RPM package
build_rpm() {
    echo -e "${YELLOW}📦 Building RPM package...${NC}"
    
    # Create RPM build directories
    mkdir -p ~/rpmbuild/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
    
    cargo rpm build --no-cargo-build
    
    # Find the generated RPM file
    RPM_FILE=$(find target/rpm -name "*.rpm" | head -1)
    
    if [ -n "$RPM_FILE" ]; then
        echo -e "${GREEN}✅ RPM package created: ${RPM_FILE}${NC}"
        
        # Copy to current directory for easy access
        cp "$RPM_FILE" .
        echo -e "${GREEN}📁 Copied to: $(basename "$RPM_FILE")${NC}"
        
        # Show package info
        echo -e "${BLUE}📋 Package information:${NC}"
        rpm -qip "$(basename "$RPM_FILE")" || true
    else
        echo -e "${RED}❌ Failed to create RPM package${NC}"
        return 1
    fi
}

# Function to show usage
show_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --deb-only    Build only DEB package"
    echo "  --rpm-only    Build only RPM package"
    echo "  --clean       Clean previous builds"
    echo "  --help        Show this help message"
    echo ""
    echo "By default, builds both DEB and RPM packages."
}

# Function to clean previous builds
clean_builds() {
    echo -e "${YELLOW}🧹 Cleaning previous builds...${NC}"
    
    rm -rf target/debian target/rpm
    rm -f *.deb *.rpm
    rm -rf debian systemd
    
    # Remove metadata from Cargo.toml if it exists
    if grep -q "^\[package\.metadata\.deb\]" Cargo.toml; then
        # Create a backup
        cp Cargo.toml Cargo.toml.backup
        
        # Remove metadata sections
        sed -i '/^\[package\.metadata\.deb\]/,/^$/d' Cargo.toml
        sed -i '/^\[package\.metadata\.rpm\]/,/^$/d' Cargo.toml
        
        echo -e "${GREEN}✅ Cleaned builds and reset Cargo.toml${NC}"
    fi
}

# Main execution
main() {
    local build_deb=true
    local build_rpm=true
    
    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --deb-only)
                build_rpm=false
                shift
                ;;
            --rpm-only)
                build_deb=false
                shift
                ;;
            --clean)
                clean_builds
                exit 0
                ;;
            --help)
                show_usage
                exit 0
                ;;
            *)
                echo -e "${RED}❌ Unknown option: $1${NC}"
                show_usage
                exit 1
                ;;
        esac
    done
    
    # Execute build steps
    check_dependencies
    install_cargo_tools
    setup_package_metadata
    setup_debian_scripts
    create_systemd_service
    build_release
    
    local success=true
    
    if [ "$build_deb" = true ]; then
        build_deb || success=false
    fi
    
    if [ "$build_rpm" = true ]; then
        build_rpm || success=false
    fi
    
    if [ "$success" = true ]; then
        echo ""
        echo -e "${GREEN}🎉 Package build completed successfully!${NC}"
        echo ""
        echo -e "${BLUE}📦 Generated packages:${NC}"
        ls -la *.deb *.rpm 2>/dev/null || echo "No packages found in current directory"
        echo ""
        echo -e "${BLUE}📖 Installation instructions:${NC}"
        if [ "$build_deb" = true ]; then
            echo -e "${YELLOW}Debian/Ubuntu:${NC} sudo dpkg -i auto-dns_${VERSION}_amd64.deb"
        fi
        if [ "$build_rpm" = true ]; then
            echo -e "${YELLOW}Fedora/RHEL:${NC} sudo rpm -i auto-dns-${VERSION}-1.x86_64.rpm"
        fi
        echo ""
        echo -e "${BLUE}📚 Post-installation:${NC}"
        echo "1. Edit /etc/auto-dns/config.toml with your settings"
        echo "2. sudo systemctl enable auto-dns"
        echo "3. sudo systemctl start auto-dns"
    else
        echo -e "${RED}❌ Some packages failed to build${NC}"
        exit 1
    fi
}

# Run main function with all arguments
main "$@"
