use anyhow::{Context, Result};
use aws_config::{BehaviorVersion, Region};
use aws_credential_types::{provider::SharedCredentialsProvider, Credentials};
use aws_sdk_route53::types::{Change, ChangeAction, ResourceRecord, ResourceRecordSet, RrType};
use aws_sdk_route53::Client;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use tracing::{debug, info};

use crate::config::AwsConfig;

#[async_trait::async_trait]
pub trait DnsOperations {
    async fn get_current_record_ip(
        &self,
        hosted_zone_id: &str,
        record_name: &str,
    ) -> Result<Ipv4Addr>;

    async fn update_record(
        &self,
        hosted_zone_id: &str,
        record_name: &str,
        ip: &Ipv4Addr,
        ttl: i64,
    ) -> Result<()>;
}

pub struct DnsUpdater {
    client: Client,
}

#[async_trait::async_trait]
impl DnsOperations for DnsUpdater {
    async fn get_current_record_ip(
        &self,
        hosted_zone_id: &str,
        record_name: &str,
    ) -> Result<Ipv4Addr> {
        debug!("Getting current IP for record: {}", record_name);

        let response = self
            .client
            .list_resource_record_sets()
            .hosted_zone_id(hosted_zone_id)
            .send()
            .await
            .with_context(|| format!("Failed to list records in zone {hosted_zone_id}"))?;

        for record_set in response.resource_record_sets() {
            let name = record_set.name();
            let record_type = record_set.r#type();

            if name.trim_end_matches('.') == record_name.trim_end_matches('.')
                && *record_type == RrType::A
            {
                let records = record_set.resource_records();
                if let Some(first_record) = records.first() {
                    let value = first_record.value();
                    return value
                        .parse()
                        .with_context(|| format!("Invalid IP in DNS record: {value}"));
                }
            }
        }

        anyhow::bail!("No A record found for {}", record_name)
    }

    async fn update_record(
        &self,
        hosted_zone_id: &str,
        record_name: &str,
        ip: &Ipv4Addr,
        ttl: i64,
    ) -> Result<()> {
        info!("Updating DNS record {} to {}", record_name, ip);

        let record_name = if record_name.ends_with('.') {
            record_name.to_string()
        } else {
            format!("{record_name}.")
        };

        let resource_record = ResourceRecord::builder()
            .value(ip.to_string())
            .build()
            .context("Failed to build resource record")?;

        let record_set = ResourceRecordSet::builder()
            .name(&record_name)
            .r#type(RrType::A)
            .ttl(ttl)
            .resource_records(resource_record)
            .build()
            .context("Failed to build resource record set")?;

        let change = Change::builder()
            .action(ChangeAction::Upsert)
            .resource_record_set(record_set)
            .build()
            .context("Failed to build change")?;

        let response = self
            .client
            .change_resource_record_sets()
            .hosted_zone_id(hosted_zone_id)
            .change_batch(
                aws_sdk_route53::types::ChangeBatch::builder()
                    .changes(change)
                    .comment(format!("Updated by auto-dns at {}", chrono::Utc::now()))
                    .build()
                    .context("Failed to build change batch")?,
            )
            .send()
            .await
            .with_context(|| {
                format!("Failed to update DNS record {record_name} in zone {hosted_zone_id}")
            })?;

        if let Some(change_info) = response.change_info() {
            debug!("Change submitted with ID: {:?}", change_info.id());
        }

        Ok(())
    }
}

impl DnsUpdater {
    pub async fn new(aws_config: &AwsConfig) -> Result<Self> {
        let credentials = Credentials::new(
            &aws_config.access_key_id,
            &aws_config.secret_access_key,
            None,
            None,
            "auto-dns",
        );

        let region = Region::new(aws_config.region());

        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(region)
            .credentials_provider(SharedCredentialsProvider::new(credentials))
            .load()
            .await;

        let client = Client::new(&config);

        Ok(Self { client })
    }
}

pub struct MockDnsUpdater {
    // Unused field for now but could be used for more sophisticated simulation
    #[allow(dead_code)]
    simulated_records: HashMap<String, Ipv4Addr>,
}

impl MockDnsUpdater {
    pub fn new() -> Self {
        Self {
            simulated_records: HashMap::new(),
        }
    }
}

#[async_trait::async_trait]
impl DnsOperations for MockDnsUpdater {
    async fn get_current_record_ip(
        &self,
        hosted_zone_id: &str,
        record_name: &str,
    ) -> Result<Ipv4Addr> {
        info!("[DRY RUN] Getting current IP for record: {} in zone {}", record_name, hosted_zone_id);

        // Simulate a different IP to trigger updates in dry run mode
        let simulated_ip = "192.168.1.100".parse().unwrap();
        info!("[DRY RUN] Simulated current DNS IP: {}", simulated_ip);

        Ok(simulated_ip)
    }

    async fn update_record(
        &self,
        hosted_zone_id: &str,
        record_name: &str,
        ip: &Ipv4Addr,
        ttl: i64,
    ) -> Result<()> {
        info!("[DRY RUN] Would update DNS record {} in zone {} to {} with TTL {}",
              record_name, hosted_zone_id, ip, ttl);
        info!("[DRY RUN] AWS Route53 API call would be made to change_resource_record_sets");
        info!("[DRY RUN] Change would be: UPSERT A record {} -> {}", record_name, ip);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require AWS credentials and would modify real DNS records
    // In a real project, you'd want to use mocks or a test environment

    #[ignore] // Use `cargo test -- --ignored` to run integration tests
    #[tokio::test]
    async fn test_dns_operations() {
        use crate::config::AwsConfig;

        let aws_config = AwsConfig {
            access_key_id: "test-access-key".to_string(),
            secret_access_key: "test-secret-key".to_string(),
        };

        let updater = DnsUpdater::new(&aws_config).await.unwrap();

        // These values should be replaced with actual test zone/record
        let test_zone_id = "Z1234567890ABC";
        let test_record = "test.example.com";
        let test_ip: Ipv4Addr = "1.2.3.4".parse().unwrap();

        // Test updating a record
        updater
            .update_record(test_zone_id, test_record, &test_ip, 300)
            .await
            .unwrap();

        // Test getting the record back
        let retrieved_ip = updater
            .get_current_record_ip(test_zone_id, test_record)
            .await
            .unwrap();

        assert_eq!(retrieved_ip, test_ip);
    }
}
