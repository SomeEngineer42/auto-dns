use anyhow::{bail, Result};
use clap::Parser;
use std::io::{self, Write};
use std::time::Duration;
use tracing::{error, info, warn};

mod config;
mod dns;
mod ip;

use config::Config;
use dns::{DnsOperations, DnsUpdater, MockDnsUpdater};
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

    /// Create a new configuration file at the specified path
    #[arg(long)]
    write_config: Option<String>,

    /// Simulate AWS operations without making actual API calls (dry run mode)
    #[arg(long)]
    no_aws: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Validate that write-config is used alone
    if let Some(config_path) = &cli.write_config {
        if cli.once || cli.config != "config.toml" || cli.no_aws {
            bail!("--write-config cannot be used with other flags");
        }
        return create_config_interactively(config_path).await;
    }

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("auto_dns=info")
        .init();

    info!("Starting auto-dns updater");

    // Load configuration
    let config = Config::load(&cli.config).await?;
    info!("Loaded configuration for {} records", config.records.len());

    // Initialize components
    let ip_detector = IpDetector::new();

    if cli.no_aws {
        info!("Running in dry-run mode (--no-aws). No actual AWS API calls will be made.");
        let mock_dns_updater = MockDnsUpdater::new();

        if cli.once {
            run_update(&ip_detector, &mock_dns_updater, &config).await?;
        } else {
            run_continuous(&ip_detector, &mock_dns_updater, &config).await?;
        }
    } else {
        let dns_updater = DnsUpdater::new(&config.aws).await?;

        if cli.once {
            run_update(&ip_detector, &dns_updater, &config).await?;
        } else {
            run_continuous(&ip_detector, &dns_updater, &config).await?;
        }
    }

    Ok(())
}

async fn create_config_interactively(config_path: &str) -> Result<()> {
    println!("Creating new configuration file at: {config_path}");
    println!();

    // Get AWS configuration
    println!("AWS Configuration:");
    print!("AWS Region (e.g., us-east-1): ");
    io::stdout().flush()?;
    let mut region = String::new();
    io::stdin().read_line(&mut region)?;
    let region = region.trim().to_string();

    print!("AWS Access Key ID (optional, leave empty to use default credentials): ");
    io::stdout().flush()?;
    let mut access_key_id = String::new();
    io::stdin().read_line(&mut access_key_id)?;
    let access_key_id = access_key_id.trim();

    print!("AWS Secret Access Key (optional, leave empty to use default credentials): ");
    io::stdout().flush()?;
    let mut secret_access_key = String::new();
    io::stdin().read_line(&mut secret_access_key)?;
    let secret_access_key = secret_access_key.trim();

    println!();
    println!("DNS Records Configuration:");
    print!("How many DNS records do you want to configure? ");
    io::stdout().flush()?;
    let mut num_records = String::new();
    io::stdin().read_line(&mut num_records)?;
    let num_records: usize = num_records.trim().parse()?;

    let mut records = Vec::new();
    for i in 1..=num_records {
        println!("\nRecord {i}:");

        print!("Hosted Zone ID: ");
        io::stdout().flush()?;
        let mut hosted_zone_id = String::new();
        io::stdin().read_line(&mut hosted_zone_id)?;
        let hosted_zone_id = hosted_zone_id.trim().to_string();

        print!("Record name (e.g., home.example.com): ");
        io::stdout().flush()?;
        let mut name = String::new();
        io::stdin().read_line(&mut name)?;
        let name = name.trim().to_string();

        print!("TTL in seconds (default 300): ");
        io::stdout().flush()?;
        let mut ttl = String::new();
        io::stdin().read_line(&mut ttl)?;
        let ttl = if ttl.trim().is_empty() {
            300
        } else {
            ttl.trim().parse()?
        };

        records.push(format!(
            r#"
[[records]]
hosted_zone_id = "{hosted_zone_id}"
name = "{name}"
ttl = {ttl}"#,
        ));
    }

    // Generate config content
    let mut config_content = String::new();
    config_content.push_str("[aws]\n");
    config_content.push_str(&format!("region = \"{region}\"\n"));

    if !access_key_id.is_empty() && !secret_access_key.is_empty() {
        config_content.push_str(&format!("access_key_id = \"{access_key_id}\"\n"));
        config_content.push_str(&format!("secret_access_key = \"{secret_access_key}\"\n"));
    }

    for record in records {
        config_content.push_str(&record);
    }

    // Write config file
    tokio::fs::write(config_path, config_content).await?;
    println!(
        "\nConfiguration file created successfully at: {config_path}",
    );

    Ok(())
}

async fn run_update(
    ip_detector: &IpDetector,
    dns_updater: &dyn DnsOperations,
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
                if dns_ip != current_ip {
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
                } else {
                    info!("IP for {} is up to date: {}", record.name, current_ip);
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
    dns_updater: &dyn DnsOperations,
    config: &Config,
) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(300)); // Fixed 5-minute interval

    loop {
        interval.tick().await;

        if let Err(e) = run_update(ip_detector, dns_updater, config).await {
            error!("Error during update cycle: {}", e);
        }
    }
}
