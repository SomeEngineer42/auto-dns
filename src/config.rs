use anyhow::{Context, Result};
use dialoguer::{theme::ColorfulTheme, Confirm, Input};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub records: Vec<DnsRecord>,
    #[serde(default)]
    pub aws: AwsConfig,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct AwsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_key_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DnsRecord {
    pub name: String,
    pub hosted_zone_id: String,
    #[serde(default = "default_ttl")]
    pub ttl: i64,
}

fn default_ttl() -> i64 {
    300 // 5 minutes
}

impl Config {
    pub async fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = tokio::fs::read_to_string(path.as_ref())
            .await
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;

        let config: Config =
            toml::from_str(&content).with_context(|| "Failed to parse config file as TOML")?;

        // Validate configuration
        if config.records.is_empty() {
            anyhow::bail!("No DNS records configured");
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

    pub async fn load_or_create<P: AsRef<Path>>(path: P) -> Result<Self> {
        let provided_path = path.as_ref();

        // Try to load existing config from provided path first
        if provided_path.exists() {
            return Self::load(provided_path).await;
        }

        // If provided path doesn't exist, try the fallback location in user's home directory
        let fallback_path = Self::get_fallback_config_path()?;
        if fallback_path.exists() {
            println!("Using config file from: {}", fallback_path.display());
            return Self::load(&fallback_path).await;
        }

        // If neither config exists, decide where to create the new one
        let create_path = if provided_path.file_name().unwrap_or_default() == "config.toml"
            && !provided_path.parent().map(|p| p.exists()).unwrap_or(false)
        {
            // If using default config.toml and current directory doesn't exist or isn't writable,
            // use the fallback location
            &fallback_path
        } else {
            // Use the provided path
            provided_path
        };

        println!(
            "Configuration file '{}' not found.",
            provided_path.display()
        );

        if create_path != provided_path {
            println!("Will create new config at: {}", create_path.display());
        }

        let create_config = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Would you like to create a new configuration file?")
            .default(true)
            .interact()?;

        if !create_config {
            anyhow::bail!("Configuration file is required to run auto-dns");
        }

        let config = Self::create_interactive_config().await?;
        config.save(create_path).await?;

        println!("Configuration saved to '{}'", create_path.display());
        Ok(config)
    }

    /// Get the fallback config path in user's home directory
    fn get_fallback_config_path() -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;

        Ok(home_dir
            .join(".config")
            .join("auto-dns")
            .join("config.toml"))
    }

    pub async fn create_interactive_config() -> Result<Self> {
        println!("\n=== Auto-DNS Configuration Setup ===");
        println!("Let's set up your DNS records for automatic IP updates.\n");

        let mut records = Vec::new();

        loop {
            println!("--- DNS Record Configuration ---");

            let name: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter the DNS record name (e.g., home.example.com)")
                .validate_with(|input: &String| -> Result<(), &str> {
                    if input.trim().is_empty() {
                        Err("DNS record name cannot be empty")
                    } else if !input.contains('.') {
                        Err("Please enter a valid domain name (e.g., home.example.com)")
                    } else {
                        Ok(())
                    }
                })
                .interact_text()?;

            let hosted_zone_id: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter the AWS Route53 Hosted Zone ID (e.g., Z1234567890ABC)")
                .validate_with(|input: &String| -> Result<(), &str> {
                    if input.trim().is_empty() {
                        Err("Hosted Zone ID cannot be empty")
                    } else if input.len() < 10 {
                        Err("Hosted Zone ID seems too short")
                    } else {
                        Ok(())
                    }
                })
                .interact_text()?;

            let ttl: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter TTL in seconds")
                .default("300".to_string())
                .validate_with(|input: &String| -> Result<(), &str> {
                    match input.parse::<i64>() {
                        Ok(val) if val > 0 => Ok(()),
                        Ok(_) => Err("TTL must be a positive number"),
                        Err(_) => Err("Please enter a valid number"),
                    }
                })
                .interact_text()?;

            let ttl_parsed = ttl.parse::<i64>().unwrap(); // Safe due to validation above

            records.push(DnsRecord {
                name: name.trim().to_string(),
                hosted_zone_id: hosted_zone_id.trim().to_string(),
                ttl: ttl_parsed,
            });

            println!(
                "\nRecord added: {} -> {} (TTL: {}s)",
                records.last().unwrap().name,
                records.last().unwrap().hosted_zone_id,
                records.last().unwrap().ttl
            );

            let add_another = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Would you like to add another DNS record?")
                .default(false)
                .interact()?;

            if !add_another {
                break;
            }
            println!();
        }

        println!("\n--- AWS Configuration ---");
        println!("Configure AWS credentials for Route53 access.");
        println!("Note: You can leave these empty to use environment variables, IAM roles, or AWS profiles.");

        let use_credentials = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Would you like to configure AWS credentials in the config file?")
            .default(false)
            .interact()?;

        let aws_config = if use_credentials {
            let access_key_id: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter AWS Access Key ID")
                .allow_empty(true)
                .interact_text()?;

            let secret_access_key: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter AWS Secret Access Key")
                .allow_empty(true)
                .interact_text()?;

            AwsConfig {
                access_key_id: if access_key_id.is_empty() {
                    None
                } else {
                    Some(access_key_id)
                },
                secret_access_key: if secret_access_key.is_empty() {
                    None
                } else {
                    Some(secret_access_key)
                },
            }
        } else {
            AwsConfig::default()
        };

        Ok(Config {
            records,
            aws: aws_config,
        })
    }

    pub async fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let toml_content =
            toml::to_string(self).with_context(|| "Failed to serialize config to TOML")?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.as_ref().parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        tokio::fs::write(&path, toml_content)
            .await
            .with_context(|| format!("Failed to write config file: {}", path.as_ref().display()))?;

        Ok(())
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
            aws: AwsConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

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
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();

        let result = Config::load(temp_file.path()).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No DNS records configured"));
    }
}
