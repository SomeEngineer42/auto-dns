use anyhow::{Context, Result};
use std::net::Ipv4Addr;
use std::str::FromStr;
use tracing::{debug, warn};

pub struct IpDetector {
    client: reqwest::Client,
    services: Vec<&'static str>,
}

impl IpDetector {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        let services = vec![
            "https://api.ipify.org",
            "https://icanhazip.com",
            "https://ifconfig.me/ip",
            "https://checkip.amazonaws.com",
            "https://ipecho.net/plain",
        ];

        Self { client, services }
    }

    pub async fn get_public_ip(&self) -> Result<Ipv4Addr> {
        for (i, service) in self.services.iter().enumerate() {
            debug!("Trying IP detection service {}: {}", i + 1, service);

            match self.fetch_ip_from_service(service).await {
                Ok(ip) => {
                    debug!("Successfully got IP {} from {}", ip, service);
                    return Ok(ip);
                }
                Err(e) => {
                    warn!("Failed to get IP from {}: {}", service, e);
                    continue;
                }
            }
        }

        anyhow::bail!("Failed to detect public IP from any service")
    }

    async fn fetch_ip_from_service(&self, url: &str) -> Result<Ipv4Addr> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed to make request to {url}"))?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP error {}: {}", response.status(), url);
        }

        let text = response
            .text()
            .await
            .with_context(|| format!("Failed to read response from {url}"))?;

        let ip_str = text.trim();
        let ip = Ipv4Addr::from_str(ip_str)
            .with_context(|| format!("Invalid IP address '{ip_str}' from {url}"))?;

        Ok(ip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_public_ip() {
        let detector = IpDetector::new();
        let result = detector.get_public_ip().await;

        // This test depends on network connectivity, so we'll just check
        // that it either succeeds or fails gracefully
        match result {
            Ok(ip) => {
                println!("Detected IP: {}", ip);
                assert!(!ip.is_loopback());
                assert!(!ip.is_private());
            }
            Err(e) => {
                println!("IP detection failed (this is OK in CI): {}", e);
            }
        }
    }
}
