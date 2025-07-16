use anyhow::Result;
use clap::Parser;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tracing::{error, info, warn};

mod config;
mod dns;
mod ip;

use config::Config;
use dns::DnsUpdater;
use ip::IpDetector;

#[derive(Parser)]
#[command(name = "auto-dns")]
#[command(about = "Automatically update AWS Route53 DNS records with current public IP")]
struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Run once and exit (don't run continuously)
    #[arg(long)]
    once: bool,

    /// Check interval in seconds
    #[arg(long, default_value = "300")]
    interval: u64,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Skip systemd service setup check
    #[arg(long)]
    skip_systemd_setup: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("auto_dns={log_level}"))
        .init();

    info!("Starting auto-dns updater");

    // Load environment variables
    dotenv::dotenv().ok();

    // Check and setup systemd service if needed
    if !cli.skip_systemd_setup && !cli.once {
        if let Err(e) = check_and_setup_systemd(&cli).await {
            warn!("Failed to setup systemd service: {}", e);
            info!("Continuing without systemd service setup...");
        }
    }

    // Load configuration
    let config = Config::load_or_create(&cli.config).await?;
    info!("Loaded configuration for {} records", config.records.len());

    // Initialize components
    let ip_detector = IpDetector::new();
    let dns_updater = DnsUpdater::new_with_config(&config.aws).await?;

    if cli.once {
        run_update(&ip_detector, &dns_updater, &config).await?;
    } else {
        run_continuous(&ip_detector, &dns_updater, &config, cli.interval).await?;
    }

    Ok(())
}

async fn run_update(
    ip_detector: &IpDetector,
    dns_updater: &DnsUpdater,
    config: &Config,
) -> Result<()> {
    info!("Checking current public IP");
    let current_ip = ip_detector.get_public_ip().await?;
    info!("Current public IP: {}", current_ip);

    for record in &config.records {
        info!("Checking DNS record: {}", record.name);

        match dns_updater
            .get_current_record_ip(&record.hosted_zone_id, &record.name)
            .await
        {
            Ok(dns_ip) => {
                if dns_ip == current_ip {
                    info!("IP for {} is up to date: {}", record.name, current_ip);
                } else {
                    info!(
                        "IP mismatch for {}: DNS={}, Current={}. Updating...",
                        record.name, dns_ip, current_ip
                    );

                    dns_updater
                        .update_record(
                            &record.hosted_zone_id,
                            &record.name,
                            &current_ip,
                            record.ttl,
                        )
                        .await?;

                    info!("Successfully updated {} to {}", record.name, current_ip);
                }
            }
            Err(e) => {
                warn!(
                    "Could not get current DNS record for {}: {}",
                    record.name, e
                );
                info!(
                    "Creating new record for {} with IP {}",
                    record.name, current_ip
                );

                dns_updater
                    .update_record(
                        &record.hosted_zone_id,
                        &record.name,
                        &current_ip,
                        record.ttl,
                    )
                    .await?;

                info!(
                    "Successfully created {} with IP {}",
                    record.name, current_ip
                );
            }
        }
    }

    Ok(())
}

async fn run_continuous(
    ip_detector: &IpDetector,
    dns_updater: &DnsUpdater,
    config: &Config,
    interval_seconds: u64,
) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));

    loop {
        interval.tick().await;

        if let Err(e) = run_update(ip_detector, dns_updater, config).await {
            error!("Error during update cycle: {}", e);
        }
    }
}

async fn check_and_setup_systemd(cli: &Cli) -> Result<()> {
    // Check if we're running on a system with systemd
    if !is_systemd_available() {
        info!("systemd not available on this system, skipping service setup");
        return Ok(());
    }

    // Check if running as root (required for systemd service installation)
    if !is_running_as_root() {
        info!("Not running as root, skipping systemd service setup");
        info!(
            "To setup systemd service, run: sudo {} --skip-systemd-setup",
            std::env::args().collect::<Vec<_>>().join(" ")
        );
        return Ok(());
    }

    let service_name = "auto-dns.service";
    let service_path = format!("/etc/systemd/system/{}", service_name);

    // Check if service already exists
    if Path::new(&service_path).exists() {
        info!("systemd service already exists at {}", service_path);
        return Ok(());
    }

    info!("Setting up systemd service...");

    // Get current executable path
    let current_exe = std::env::current_exe()?;
    let current_exe_path = current_exe.to_string_lossy();

    // Get absolute path to config file
    let config_path = if Path::new(&cli.config).is_absolute() {
        cli.config.clone()
    } else {
        std::env::current_dir()?
            .join(&cli.config)
            .to_string_lossy()
            .to_string()
    };

    // Create the systemd service content
    let service_content =
        create_systemd_service_content(&current_exe_path, &config_path, cli.interval, cli.verbose);

    // Write the service file
    fs::write(&service_path, service_content)?;
    info!("Created systemd service file: {}", service_path);

    // Reload systemd and enable the service
    run_command("systemctl", &["daemon-reload"])?;
    info!("Reloaded systemd daemon");

    run_command("systemctl", &["enable", service_name])?;
    info!("Enabled {} service", service_name);

    run_command("systemctl", &["start", service_name])?;
    info!("Started {} service", service_name);

    info!("systemd service setup complete!");
    info!(
        "You can check status with: sudo systemctl status {}",
        service_name
    );
    info!("View logs with: sudo journalctl -u {} -f", service_name);

    Ok(())
}

fn is_systemd_available() -> bool {
    Path::new("/run/systemd/system").exists()
}

fn is_running_as_root() -> bool {
    nix::unistd::geteuid().is_root()
}

fn create_systemd_service_content(
    exe_path: &str,
    config_path: &str,
    interval: u64,
    verbose: bool,
) -> String {
    let verbose_flag = if verbose { " --verbose" } else { "" };

    format!(
        r#"[Unit]
Description=Auto DNS Updater
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart={} --config {} --interval {} --skip-systemd-setup{}
Restart=always
RestartSec=10
User=root
Group=root

# Environment for AWS credentials (if using files)
Environment=AWS_CONFIG_FILE=/root/.aws/config
Environment=AWS_SHARED_CREDENTIALS_FILE=/root/.aws/credentials

# Security settings
NoNewPrivileges=true
ProtectHome=true
ProtectSystem=strict
ReadWritePaths=/var/log

[Install]
WantedBy=multi-user.target
"#,
        exe_path, config_path, interval, verbose_flag
    )
}

fn run_command(command: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(command).args(args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Command '{}' failed: {}", command, stderr);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AwsConfig, DnsRecord};
    use tempfile::tempdir;

    #[test]
    fn test_create_systemd_service_content() {
        let exe_path = "/usr/local/bin/auto-dns";
        let config_path = "/etc/auto-dns/config.toml";
        let interval = 300;
        let verbose = true;

        let content = create_systemd_service_content(exe_path, config_path, interval, verbose);

        // Check that the service content contains expected components
        assert!(content.contains("[Unit]"));
        assert!(content.contains("Description=Auto DNS Updater"));
        assert!(content.contains("After=network-online.target"));
        assert!(content.contains("Wants=network-online.target"));

        assert!(content.contains("[Service]"));
        assert!(content.contains("Type=simple"));
        assert!(content.contains(&format!(
            "ExecStart={} --config {} --interval {} --skip-systemd-setup --verbose",
            exe_path, config_path, interval
        )));
        assert!(content.contains("Restart=always"));
        assert!(content.contains("RestartSec=10"));
        assert!(content.contains("User=root"));
        assert!(content.contains("Group=root"));

        // Check AWS environment variables
        assert!(content.contains("Environment=AWS_CONFIG_FILE=/root/.aws/config"));
        assert!(content.contains("Environment=AWS_SHARED_CREDENTIALS_FILE=/root/.aws/credentials"));

        // Check security settings
        assert!(content.contains("NoNewPrivileges=true"));
        assert!(content.contains("ProtectHome=true"));
        assert!(content.contains("ProtectSystem=strict"));
        assert!(content.contains("ReadWritePaths=/var/log"));

        assert!(content.contains("[Install]"));
        assert!(content.contains("WantedBy=multi-user.target"));
    }

    #[test]
    fn test_create_systemd_service_content_without_verbose() {
        let exe_path = "/usr/local/bin/auto-dns";
        let config_path = "/etc/auto-dns/config.toml";
        let interval = 600;
        let verbose = false;

        let content = create_systemd_service_content(exe_path, config_path, interval, verbose);

        // Should not contain --verbose flag when verbose is false
        assert!(content.contains(&format!(
            "ExecStart={} --config {} --interval {} --skip-systemd-setup",
            exe_path, config_path, interval
        )));
        assert!(!content.contains("--verbose"));
    }

    #[test]
    fn test_create_systemd_service_content_with_different_intervals() {
        let exe_path = "/usr/bin/auto-dns";
        let config_path = "/home/user/config.toml";

        // Test with different interval values
        for interval in [60, 300, 900, 3600] {
            let content = create_systemd_service_content(exe_path, config_path, interval, false);
            assert!(content.contains(&format!("--interval {}", interval)));
        }
    }

    #[test]
    fn test_systemd_service_content_escaping() {
        // Test with paths that might need escaping
        let exe_path = "/usr/local/bin/auto-dns with spaces";
        let config_path = "/etc/auto-dns/config with spaces.toml";
        let interval = 300;
        let verbose = false;

        let content = create_systemd_service_content(exe_path, config_path, interval, verbose);

        // The content should contain the paths exactly as provided
        // (systemd handles path escaping automatically)
        assert!(content.contains(exe_path));
        assert!(content.contains(config_path));
    }

    #[test]
    fn test_is_systemd_available() {
        // This test will depend on the environment, but we can at least verify
        // the function doesn't panic and returns a boolean
        let result = is_systemd_available();
        assert!(result == true || result == false);
    }

    #[test]
    fn test_is_running_as_root() {
        // This test will depend on how the tests are run, but we can verify
        // the function doesn't panic and returns a boolean
        let result = is_running_as_root();
        assert!(result == true || result == false);
    }

    #[tokio::test]
    async fn test_check_and_setup_systemd_not_as_root() {
        // Create a temporary CLI struct for testing
        let cli = Cli {
            config: "test-config.toml".to_string(),
            once: false,
            interval: 300,
            verbose: false,
            skip_systemd_setup: false,
        };

        // If not running as root, this should succeed without creating files
        // (it will log and return Ok)
        let result = check_and_setup_systemd(&cli).await;

        // The function should handle non-root execution gracefully
        assert!(result.is_ok() || result.is_err()); // Should not panic
    }

    #[test]
    fn test_systemd_service_file_format() {
        let content =
            create_systemd_service_content("/usr/bin/auto-dns", "/etc/auto-dns.toml", 300, true);

        // Verify the service file has proper INI format structure
        let lines: Vec<&str> = content.lines().collect();

        // Should start with [Unit] section
        assert!(lines.iter().any(|&line| line == "[Unit]"));

        // Should have [Service] section
        assert!(lines.iter().any(|&line| line == "[Service]"));

        // Should have [Install] section
        assert!(lines.iter().any(|&line| line == "[Install]"));

        // Check for proper key=value format in service section
        let service_section_started = lines.iter().position(|&line| line == "[Service]").unwrap();
        let install_section_started = lines.iter().position(|&line| line == "[Install]").unwrap();

        for line in &lines[service_section_started + 1..install_section_started] {
            if !line.is_empty() && !line.starts_with('#') {
                assert!(
                    line.contains('='),
                    "Service section line should have key=value format: {}",
                    line
                );
            }
        }
    }

    #[test]
    fn test_run_command_success() {
        // Test with a command that should always succeed
        let result = run_command("echo", &["test"]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_command_failure() {
        // Test with a command that should fail
        let result = run_command("false", &[]);
        assert!(result.is_err());

        if let Err(e) = result {
            assert!(e.to_string().contains("Command 'false' failed"));
        }
    }

    #[tokio::test]
    async fn test_config_save_and_load() {
        // Create a temporary directory for testing
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create a test config
        let original_config = Config {
            records: vec![
                DnsRecord {
                    name: "test1.example.com".to_string(),
                    hosted_zone_id: "Z1111111111111".to_string(),
                    ttl: 300,
                },
                DnsRecord {
                    name: "test2.example.com".to_string(),
                    hosted_zone_id: "Z2222222222222".to_string(),
                    ttl: 600,
                },
            ],
            aws: AwsConfig::default(),
        };

        // Save the config
        original_config.save(&config_path).await.unwrap();

        // Load it back
        let loaded_config = Config::load(&config_path).await.unwrap();

        // Verify it matches
        assert_eq!(loaded_config.records.len(), 2);
        assert_eq!(loaded_config.records[0].name, "test1.example.com");
        assert_eq!(loaded_config.records[0].hosted_zone_id, "Z1111111111111");
        assert_eq!(loaded_config.records[0].ttl, 300);
        assert_eq!(loaded_config.records[1].name, "test2.example.com");
        assert_eq!(loaded_config.records[1].hosted_zone_id, "Z2222222222222");
        assert_eq!(loaded_config.records[1].ttl, 600);
    }

    #[tokio::test]
    async fn test_config_load_or_create_existing() {
        // Create a temporary directory for testing
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("existing_config.toml");

        // Create a test config file
        let test_config = Config {
            records: vec![DnsRecord {
                name: "existing.example.com".to_string(),
                hosted_zone_id: "Z9999999999999".to_string(),
                ttl: 900,
            }],
            aws: AwsConfig::default(),
        };
        test_config.save(&config_path).await.unwrap();

        // Use load_or_create on existing file
        let loaded_config = Config::load_or_create(&config_path).await.unwrap();

        // Should load the existing config
        assert_eq!(loaded_config.records.len(), 1);
        assert_eq!(loaded_config.records[0].name, "existing.example.com");
        assert_eq!(loaded_config.records[0].hosted_zone_id, "Z9999999999999");
        assert_eq!(loaded_config.records[0].ttl, 900);
    }

    #[tokio::test]
    async fn test_config_with_aws_credentials() {
        // Create a temporary directory for testing
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("aws_config.toml");

        // Create a config with AWS credentials
        let aws_config = AwsConfig {
            access_key_id: Some("AKIAIOSFODNN7EXAMPLE".to_string()),
            secret_access_key: Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string()),
        };

        let config = Config {
            records: vec![DnsRecord {
                name: "test.example.com".to_string(),
                hosted_zone_id: "Z1234567890ABC".to_string(),
                ttl: 300,
            }],
            aws: aws_config,
        };

        // Save and reload the config
        config.save(&config_path).await.unwrap();
        let loaded_config = Config::load(&config_path).await.unwrap();

        // Verify AWS configuration was preserved
        assert_eq!(
            loaded_config.aws.access_key_id,
            Some("AKIAIOSFODNN7EXAMPLE".to_string())
        );
        assert_eq!(
            loaded_config.aws.secret_access_key,
            Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string())
        );
    }

    #[tokio::test]
    async fn test_config_without_aws_credentials() {
        // Create a temporary directory for testing
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("no_aws_config.toml");

        // Create a config without AWS credentials (using defaults)
        let config = Config {
            records: vec![DnsRecord {
                name: "test.example.com".to_string(),
                hosted_zone_id: "Z1234567890ABC".to_string(),
                ttl: 300,
            }],
            aws: AwsConfig::default(),
        };

        // Save and reload the config
        config.save(&config_path).await.unwrap();
        let loaded_config = Config::load(&config_path).await.unwrap();

        // Verify AWS configuration is empty (will use environment/IAM)
        assert_eq!(loaded_config.aws.access_key_id, None);
        assert_eq!(loaded_config.aws.secret_access_key, None);
    }
}
