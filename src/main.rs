use anyhow::Result;
use clap::Parser;
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

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
    let dns_updater = DnsUpdater::new(&config.aws).await?;

    if cli.once {
        run_update(&ip_detector, &dns_updater, &config).await?;
    } else {
        run_continuous(&ip_detector, &dns_updater, &config).await?;
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

        match dns_updater.get_current_record_ip(&record.hosted_zone_id, &record.name).await {
            Ok(dns_ip) => {
                if dns_ip != current_ip {
                    info!(
                        "IP mismatch for {}: DNS={}, Current={}. Updating...",
                        record.name, dns_ip, current_ip
                    );

                    dns_updater
                        .update_record(&record.hosted_zone_id, &record.name, &current_ip, record.ttl)
                        .await?;

                    info!("Successfully updated {} to {}", record.name, current_ip);
                } else {
                    info!("IP for {} is up to date: {}", record.name, current_ip);
                }
            }
            Err(e) => {
                warn!("Could not get current DNS record for {}: {}", record.name, e);
                info!("Creating new record for {} with IP {}", record.name, current_ip);

                dns_updater
                    .update_record(&record.hosted_zone_id, &record.name, &current_ip, record.ttl)
                    .await?;

                info!("Successfully created {} with IP {}", record.name, current_ip);
            }
        }
    }

    Ok(())
}

async fn run_continuous(
    ip_detector: &IpDetector,
    dns_updater: &DnsUpdater,
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
