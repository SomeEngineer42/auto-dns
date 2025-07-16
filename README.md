# auto-dns

A Rust-based application that automatically updates AWS Route53 DNS records with your current public IP address. Perfect for dynamic DNS setups where you need to keep DNS records pointing to your home IP or other dynamic endpoints.

## Features

- 🚀 **Fast and Reliable**: Built in Rust for performance and safety
- 🔄 **Automatic Updates**: Continuously monitors and updates DNS records
- 🌐 **Multiple IP Sources**: Uses multiple IP detection services for reliability
- ⚙️ **Flexible Configuration**: TOML-based configuration with multiple record support
- 📊 **Comprehensive Logging**: Detailed logging with configurable levels
- 🐳 **Docker Support**: Easy deployment with Docker and Docker Compose
- 🔒 **AWS Integration**: Native AWS SDK integration with proper error handling
- 🛡️ **Robust Error Handling**: Graceful handling of network issues and AWS API errors

## Prerequisites

- AWS account with Route53 access
- Hosted zone configured in Route53
- AWS credentials configured (IAM user, role, or instance profile)

### Required AWS Permissions

Your AWS credentials need the following permissions:

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Effect": "Allow",
            "Action": [
                "route53:GetChange",
                "route53:ListResourceRecordSets",
                "route53:ChangeResourceRecordSets"
            ],
            "Resource": [
                "arn:aws:route53:::hostedzone/YOUR_HOSTED_ZONE_ID",
                "arn:aws:route53:::change/*"
            ]
        }
    ]
}
```

## Installation

### Option 1: Development Container (Recommended)

The easiest way to get started is using the provided devcontainer:

1. Install [VS Code](https://code.visualstudio.com/) and the [Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
2. Clone this repository
3. Open in VS Code and select "Reopen in Container" when prompted
4. Everything will be set up automatically!

See [.devcontainer/README.md](.devcontainer/README.md) for detailed devcontainer documentation.

### Option 2: Build from Source

```bash
# Clone the repository
git clone https://github.com/SomeEngineer42/auto-dns.git
cd auto-dns

# Build the application
cargo build --release

# The binary will be available at target/release/auto-dns
```

### Option 3: Package Installation (DEB/RPM)

```bash
# Clone the repository
git clone https://github.com/SomeEngineer42/auto-dns.git
cd auto-dns

# Build packages for your distribution
./build.sh --help  # Show build options
./build.sh         # Build both DEB and RPM packages

# Install on Debian/Ubuntu
sudo dpkg -i auto-dns_*.deb

# Install on Fedora/RHEL
sudo rpm -i auto-dns-*.rpm
```

### Option 4: Docker

```bash
# Build the Docker image
docker build -t auto-dns .

# Or use Docker Compose
docker-compose up -d
```

## Configuration

The application will look for configuration files in the following order:

1. **Specified path** (if using `--config` option)
2. **Current directory**: `./config.toml` (default)
3. **User's home directory**: `~/.config/auto-dns/config.toml` (fallback)

When creating a new configuration file, if no specific path is provided and the current directory is not writable, the configuration will be created in the fallback location (`~/.config/auto-dns/config.toml`).

### 1. Create Configuration File

Copy the example configuration and modify it for your needs:

```bash
cp config.toml.example config.toml
```

Edit `config.toml`:

```toml
# DNS Records to manage
[[records]]
name = "home.example.com"
hosted_zone_id = "Z1234567890ABC"
ttl = 300

[[records]]
name = "api.example.com"
hosted_zone_id = "Z1234567890ABC"
ttl = 600

# AWS Configuration (optional)
[aws]
access_key_id = "AKIAIOSFODNN7EXAMPLE"
secret_access_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
```

### 2. Configure AWS Credentials

You have several options for providing AWS credentials:

#### Option A: In Configuration File (Recommended for simplicity)

Include your AWS credentials directly in `config.toml` as shown above. This centralizes all configuration in one file.

#### Option B: Environment Variables

```bash
cp .env.example .env
# Edit .env with your AWS credentials
```

#### Option C: AWS CLI Profile

```bash
aws configure
# Follow the prompts to set up your credentials
```

#### Option D: IAM Roles (for EC2 instances)

If running on EC2, you can use IAM instance profiles instead of static credentials.

**Note**: The application will check for credentials in this order:
1. Configuration file (`config.toml`)
2. Environment variables
3. AWS profile
4. IAM roles/instance profiles

### 3. Find Your Hosted Zone ID

```bash
aws route53 list-hosted-zones
```

Look for the `Id` field of your domain's hosted zone (it starts with `/hostedzone/Z...`).

## Usage

### Command Line Options

```bash
auto-dns --help
```

```
Automatically update AWS Route53 DNS records with current public IP

Usage: auto-dns [OPTIONS]

Options:
  -c, --config <CONFIG>      Configuration file path [default: config.toml]
      --once                 Run once and exit (don't run continuously)
      --interval <INTERVAL>  Check interval in seconds [default: 300]
  -v, --verbose              Enable verbose logging
      --skip-systemd-setup   Skip systemd service setup check
  -h, --help                 Print help
```

### Examples

```bash
# Run continuously with default settings (check every 5 minutes)
# Will look for config.toml in current directory, then ~/.config/auto-dns/config.toml
./auto-dns

# Run once and exit
./auto-dns --once

# Use custom config file and check every 2 minutes
./auto-dns --config /path/to/config.toml --interval 120

# Enable verbose logging
./auto-dns --verbose

# Skip automatic systemd service setup
./auto-dns --skip-systemd-setup

# Run as root to enable automatic systemd setup (Linux only)
sudo ./auto-dns
```

### Docker Usage

```bash
# Run with Docker
docker run -d \
  --name auto-dns \
  -v $(pwd)/config.toml:/app/config.toml:ro \
  -e AWS_ACCESS_KEY_ID=your_key \
  -e AWS_SECRET_ACCESS_KEY=your_secret \
  -e AWS_DEFAULT_REGION=us-east-1 \
  auto-dns

# Or use Docker Compose
docker-compose up -d
```

### Systemd Service (Linux)

#### Automatic Setup (Recommended)

When running in continuous mode, auto-dns will automatically detect if systemd is available and set up the service for you:

```bash
# Run as root to enable automatic systemd setup
sudo ./auto-dns

# Or build and run from source
sudo cargo run --release
```

The program will:
- Detect if systemd is available
- Check if you're running as root (required for service installation)
- Create the systemd service file automatically
- Enable and start the service
- Show you how to check status and view logs

To skip this automatic setup, use the `--skip-systemd-setup` flag:

```bash
./auto-dns --skip-systemd-setup
```

#### Manual Setup

If you prefer to set up the systemd service manually:

```bash
sudo tee /etc/systemd/system/auto-dns.service > /dev/null <<EOF
[Unit]
Description=Auto DNS Updater
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/path/to/auto-dns --config /path/to/config.toml --interval 300 --skip-systemd-setup
Restart=always
RestartSec=10
User=root
Group=root

# Environment for AWS credentials
Environment=AWS_CONFIG_FILE=/root/.aws/config
Environment=AWS_SHARED_CREDENTIALS_FILE=/root/.aws/credentials

# Security settings
NoNewPrivileges=true
ProtectHome=true
ProtectSystem=strict
ReadWritePaths=/var/log

[Install]
WantedBy=multi-user.target
EOF

# Enable and start the service
sudo systemctl daemon-reload
sudo systemctl enable auto-dns
sudo systemctl start auto-dns

# Check status
sudo systemctl status auto-dns

# View logs
sudo journalctl -u auto-dns -f
```

## IP Detection Services

The application uses multiple IP detection services for reliability:

1. `https://api.ipify.org`
2. `https://icanhazip.com`
3. `https://ifconfig.me/ip`
4. `https://checkip.amazonaws.com`
5. `https://ipecho.net/plain`

If one service fails, it automatically tries the next one.

## Logging

The application provides structured logging with different levels:

- **Info**: General operation status
- **Warn**: Recoverable issues (e.g., IP service failures)
- **Error**: Serious issues that prevent operation
- **Debug**: Detailed information for troubleshooting

Use the `--verbose` flag or set `RUST_LOG=debug` for detailed logging.

## Error Handling

The application handles various error conditions gracefully:

- **Network connectivity issues**: Retries with different IP detection services
- **AWS API errors**: Detailed error reporting with context
- **DNS propagation delays**: Optional waiting for changes to propagate
- **Configuration errors**: Clear validation and error messages

## Development

### Building Packages

The project includes a build script that creates installable packages for various Linux distributions:

```bash
# Build both DEB and RPM packages
./build.sh

# Build only DEB package (Debian/Ubuntu)
./build.sh --deb-only

# Build only RPM package (Fedora/RHEL/CentOS)
./build.sh --rpm-only

# Clean previous builds
./build.sh --clean

# Show help
./build.sh --help
```

#### Package Build Requirements

The build script automatically installs required tools, but you may need to install system dependencies:

**For DEB packages:**
- `cargo-deb` (automatically installed)
- `dpkg-dev` (usually pre-installed on Debian/Ubuntu)

**For RPM packages:**
- `cargo-rpm` (automatically installed)
- `rpm-build` and `rpm-devel` (automatically installed via package manager)

#### Generated Packages

The packages include:
- Binary installed to `/usr/bin/auto-dns`
- Example configuration in `/etc/auto-dns/config.toml.example`
- Systemd service file for automatic startup
- Dedicated `auto-dns` system user for security
- Proper file permissions and directory structure

**Post-installation steps:**
1. Copy `/etc/auto-dns/config.toml.example` to `/etc/auto-dns/config.toml`
2. Edit the configuration with your settings
3. Enable and start the service: `sudo systemctl enable --now auto-dns`

### Running Tests

```bash
# Run all tests (including integration tests)
cargo test

# Note: Integration tests require AWS credentials and will modify real DNS records
# Integration tests now use your config.toml file

# Run with coverage
cargo tarpaulin --out html
```

#### Configuring Integration Tests

Before running integration tests, you need to:

1. **Configure AWS credentials** (same as for normal operation)
2. **Ensure your `config.toml` has valid values**:
   ```toml
   records:
     - name: "test.yourdomain.com"
       hosted_zone_id: "YOUR_ACTUAL_HOSTED_ZONE_ID"
       ttl: 300
   ```
3. **Ensure the test domain exists in your hosted zone**

**⚠️ Warning**: Integration tests will create/modify real DNS records. The test uses the first record from your `config.toml` file and temporarily sets it to a test IP address (`203.0.113.1`).

### Adding Dependencies

```bash
# Add a new dependency
cargo add dependency_name

# Add a development dependency
cargo add --dev dependency_name
```

### Code Formatting

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt -- --check

# Run clippy for linting
cargo clippy
```

## Troubleshooting

### Common Issues

1. **"No DNS records configured"**
   - Check your `config.toml` file syntax
   - Ensure the `records` array is not empty

2. **"Failed to detect public IP"**
   - Check internet connectivity
   - Some networks block external IP detection services

3. **"Failed to update DNS record"**
   - Verify AWS credentials and permissions
   - Check that the hosted zone ID is correct
   - Ensure the domain name matches exactly

4. **"Invalid IP address"**
   - The IP detection service returned an invalid response
   - Try running with `--verbose` to see which service failed

### Debug Mode

Run with verbose logging to see detailed information:

```bash
RUST_LOG=debug ./auto-dns --verbose
```

### Check AWS Credentials

```bash
aws sts get-caller-identity
```

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Security

- Never commit your actual AWS credentials to version control
- Use IAM roles when possible instead of static credentials
- Regularly rotate your AWS access keys
- Use least-privilege permissions for your AWS credentials

## Roadmap

- [ ] Support for AAAA (IPv6) records
- [ ] Web interface for configuration and monitoring
- [ ] Support for other DNS providers (Cloudflare, Google DNS, etc.)
- [ ] Webhook notifications for IP changes
- [ ] Prometheus metrics endpoint
- [ ] ARM Docker images
