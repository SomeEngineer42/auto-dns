# auto-dns

A Rust-based application that automatically updates AWS Route53 DNS records with your current public IP address. Perfect for dynamic DNS setups where you need to keep DNS records pointing to your home IP or other dynamic endpoints.

## Features

- 🚀 **Fast and Reliable**: Built in Rust for performance and safety
- 🔄 **Automatic Updates**: Continuously monitors and updates DNS records
- 🌐 **Multiple IP Sources**: Uses multiple IP detection services for reliability
- ⚙️ **Flexible Configuration**: YAML-based configuration with multiple record support
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

### Option 1: Build from Source

```bash
# Clone the repository
git clone https://github.com/SomeEngineer42/auto-dns.git
cd auto-dns

# Build the application
cargo build --release

# The binary will be available at target/release/auto-dns
```

### Option 2: Nix Development Environment

If you use Nix, you can get a complete development environment with all dependencies:

#### Using Nix Flakes (Recommended)

```bash
# Clone the repository
git clone https://github.com/SomeEngineer42/auto-dns.git
cd auto-dns

# Enter the development environment
nix develop

# Or with direnv (if you have it installed)
direnv allow  # This will automatically activate the environment
```

#### Using Traditional Nix

```bash
# Clone the repository
git clone https://github.com/SomeEngineer42/auto-dns.git
cd auto-dns

# Enter the development shell
nix-shell

# Build and test
cargo build --release
cargo test
```

The Nix environment includes:
- Rust 1.88.0 toolchain with clippy, rustfmt, and rust-analyzer
- All required system dependencies (OpenSSL, pkg-config)
- Development tools (cargo-watch, cargo-edit, cargo-audit)
- Docker and Docker Compose for testing

## Configuration

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

# AWS Configuration (optional - can use environment variables or IAM roles instead)
[aws]
access_key_id = "AKIAIOSFODNN7EXAMPLE"
secret_access_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
```

### 2. Find Your Hosted Zone ID

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
  -c, --config <CONFIG>      Configuration file path [default: config.yaml]
      --once                 Run once and exit (don't run continuously)
  -h, --help                 Print help
```

### Examples

```bash
# Run continuously with default settings (check every 5 minutes)
./auto-dns

# Run once and exit
./auto-dns --once

# Use custom config file
./auto-dns --config /path/to/config.yaml
```

### Systemd Service (Linux)

Create a systemd service for automatic startup:

```bash
sudo tee /etc/systemd/system/auto-dns.service > /dev/null <<EOF
[Unit]
Description=Auto DNS Updater
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=auto-dns
Group=auto-dns
WorkingDirectory=/opt/auto-dns
ExecStart=/opt/auto-dns/auto-dns --config /opt/auto-dns/config.yaml
Restart=always
RestartSec=10
Environment=AWS_CONFIG_FILE=/opt/auto-dns/.aws/config
Environment=AWS_SHARED_CREDENTIALS_FILE=/opt/auto-dns/.aws/credentials

[Install]
WantedBy=multi-user.target
EOF

# Enable and start the service
sudo systemctl enable auto-dns
sudo systemctl start auto-dns
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

### Running Tests

```bash
# Run unit tests
cargo test

# Run integration tests (requires AWS credentials)
cargo test -- --ignored

# Run with coverage
cargo tarpaulin --out html
```

### Testing the Install Script

The project includes comprehensive testing for the installation script:

```bash
# Run install script tests locally
./scripts/test-install.sh

# Skip Docker build test (faster, for syntax/validation only)
./scripts/test-install.sh --skip-docker
```

The install script tests validate:
- Script syntax and structure
- Required variable definitions
- Dockerfile generation and content
- Systemd service file generation
- Configuration file templates
- Docker build process
- Binary extraction and functionality

The CI pipeline automatically runs these tests on every pull request to ensure the install script remains functional.

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
   - Check your `config.yaml` file syntax
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
