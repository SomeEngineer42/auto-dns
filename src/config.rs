use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub records: Vec<DnsRecord>,
    pub aws: AwsConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DnsRecord {
    pub name: String,
    pub hosted_zone_id: String,
    #[serde(default = "default_ttl")]
    pub ttl: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AwsConfig {
    pub access_key_id: String,
    pub secret_access_key: String,
}

impl AwsConfig {
    pub fn region(&self) -> String {
        "us-east-1".to_string()
    }
}

fn default_ttl() -> i64 {
    300 // 5 minutes
}

impl Config {
    pub async fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = tokio::fs::read_to_string(path.as_ref())
            .await
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| "Failed to parse config file as TOML")?;

        // Validate configuration
        if config.records.is_empty() {
            anyhow::bail!("No DNS records configured");
        }

        // Validate AWS configuration
        if config.aws.access_key_id.is_empty() {
            anyhow::bail!("AWS access key ID cannot be empty");
        }
        if config.aws.secret_access_key.is_empty() {
            anyhow::bail!("AWS secret access key cannot be empty");
        }

        for record in &config.records {
            if record.name.is_empty() {
                anyhow::bail!("DNS record name cannot be empty");
            }
            if record.hosted_zone_id.is_empty() {
                anyhow::bail!("Hosted zone ID cannot be empty for record: {}", record.name);
            }
            if record.ttl <= 0 {
                anyhow::bail!("TTL must be positive for record: {}", record.name);
            }
        }

        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            records: vec![DnsRecord {
                name: "example.com".to_string(),
                hosted_zone_id: "Z1234567890ABC".to_string(),
                ttl: 300,
            }],
            aws: AwsConfig {
                access_key_id: "AKIA...".to_string(),
                secret_access_key: "...".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_load_valid_config() {
        let config_content = r#"
[[records]]
name = "test.example.com"
hosted_zone_id = "Z1234567890ABC"
ttl = 600

[[records]]
name = "api.example.com"
hosted_zone_id = "Z1234567890ABC"

[aws]
access_key_id = "AKIATEST"
secret_access_key = "test-secret"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let config = Config::load(temp_file.path()).await.unwrap();
        assert_eq!(config.records.len(), 2);
        assert_eq!(config.records[0].name, "test.example.com");
        assert_eq!(config.records[0].ttl, 600);
        assert_eq!(config.records[1].ttl, 300); // default TTL
    }

    #[tokio::test]
    async fn test_load_invalid_config() {
        let config_content = r#"
records = []

[aws]
access_key_id = "test"
secret_access_key = "test"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let result = Config::load(temp_file.path()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No DNS records configured"));
    }

    #[tokio::test]
    async fn test_load_config_without_region() {
        let config_content = r#"
[[records]]
name = "test.example.com"
hosted_zone_id = "Z1234567890ABC"
ttl = 300

[aws]
access_key_id = "AKIATEST"
secret_access_key = "test-secret"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let config = Config::load(temp_file.path()).await.unwrap();
        assert_eq!(config.records.len(), 1);
        assert_eq!(config.records[0].name, "test.example.com");
        assert_eq!(config.aws.access_key_id, "AKIATEST");
    }
}
