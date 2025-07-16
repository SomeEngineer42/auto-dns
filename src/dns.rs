use crate::config::AwsConfig;
use anyhow::{Context, Result};
use aws_sdk_route53::types::{
    Change, ChangeAction, ChangeBatch, ResourceRecord, ResourceRecordSet, RrType,
};
use aws_sdk_route53::Client;
use std::net::Ipv4Addr;
use tracing::{debug, info};

pub struct DnsUpdater {
    client: Client,
}

impl DnsUpdater {
    pub async fn new() -> Result<Self> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::v2025_01_17()).await;
        let client = Client::new(&config);

        Ok(Self { client })
    }

    pub async fn new_with_config(aws_config: &AwsConfig) -> Result<Self> {
        let mut config_builder = aws_config::defaults(aws_config::BehaviorVersion::v2025_01_17());

        // Set credentials if provided
        if let (Some(access_key), Some(secret_key)) =
            (&aws_config.access_key_id, &aws_config.secret_access_key)
        {
            use aws_credential_types::{provider::SharedCredentialsProvider, Credentials};
            let credentials = Credentials::new(access_key, secret_key, None, None, "config-file");
            config_builder =
                config_builder.credentials_provider(SharedCredentialsProvider::new(credentials));
        }

        let config = config_builder.load().await;
        let client = Client::new(&config);

        Ok(Self { client })
    }

    pub async fn get_current_record_ip(
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

    pub async fn update_record(
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
            .set_resource_records(Some(vec![resource_record]))
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
                ChangeBatch::builder()
                    .changes(change)
                    .comment(format!(
                        "Updated by auto-dns at {:?}",
                        std::time::SystemTime::now()
                    ))
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

    pub async fn wait_for_change(&self, change_id: &str) -> Result<()> {
        const MAX_ATTEMPTS: u32 = 60; // 5 minutes with 5-second intervals

        info!("Waiting for DNS change to propagate: {}", change_id);

        let mut attempts = 0;

        loop {
            let response = self
                .client
                .get_change()
                .id(change_id)
                .send()
                .await
                .with_context(|| format!("Failed to get change status for {change_id}"))?;

            if let Some(change_info) = response.change_info() {
                let status = change_info.status();
                match status {
                    aws_sdk_route53::types::ChangeStatus::Insync => {
                        info!("DNS change propagated successfully");
                        return Ok(());
                    }
                    aws_sdk_route53::types::ChangeStatus::Pending => {
                        debug!("DNS change still pending...");
                    }
                    _ => {
                        debug!("DNS change status: {:?}", status);
                    }
                }
            }

            attempts += 1;
            if attempts >= MAX_ATTEMPTS {
                anyhow::bail!("Timeout waiting for DNS change to propagate");
            }

            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require AWS credentials and would modify real DNS records
    // In a real project, you'd want to use mocks or a test environment

    #[tokio::test]
    async fn test_dns_operations() {
        // Load environment variables (for AWS credentials)
        dotenv::dotenv().ok();

        // Load configuration from the config file
        let config = crate::config::Config::load("config.toml").await.unwrap();

        // Use the first record from the config for testing
        let test_record = &config.records[0];
        let test_zone_id = &test_record.hosted_zone_id;
        let test_record_name = &test_record.name;
        let test_ttl = test_record.ttl;

        // Use a test IP address
        let test_ip: Ipv4Addr = "203.0.113.1".parse().unwrap(); // RFC 5737 test IP

        let updater = DnsUpdater::new_with_config(&config.aws).await.unwrap();

        // Test updating a record
        updater
            .update_record(test_zone_id, test_record_name, &test_ip, test_ttl)
            .await
            .unwrap();

        // Test getting the record back
        let retrieved_ip = updater
            .get_current_record_ip(test_zone_id, test_record_name)
            .await
            .unwrap();

        assert_eq!(retrieved_ip, test_ip);
    }
}
