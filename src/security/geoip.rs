use anyhow::Result;
use std::sync::Arc;
use tracing::{info, warn, error};
use super::deduplication::{SecurityDeduplicator, SecuritySource, ThreatLevel};

pub struct GeoIPManager {
    deduplicator: Arc<SecurityDeduplicator>,
    blocked_countries: Vec<String>,
    allow_list: Vec<String>,
    enabled: bool,
}

impl GeoIPManager {
    pub async fn new(deduplicator: Arc<SecurityDeduplicator>) -> Result<Self> {
        info!("Initializing GeoIP manager with GitHub-based database");

        Ok(Self {
            deduplicator,
            blocked_countries: vec![
                // High-risk countries by default (can be configured)
                "CN".to_string(), // China
                "RU".to_string(), // Russia
                "KP".to_string(), // North Korea
                "IR".to_string(), // Iran
            ],
            allow_list: vec![
                // Always allow these regardless of country
                "127.0.0.1".to_string(),
                "::1".to_string(),
                // Add local network ranges
                "192.168.0.0/16".to_string(),
                "10.0.0.0/8".to_string(),
                "172.16.0.0/12".to_string(),
            ],
            enabled: true, // Always enabled by default per spec
        })
    }

    pub async fn start(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        info!("Starting GeoIP blocking service");

        // Update GeoIP database
        self.update_database().await?;

        // Start periodic updates (every 24 hours)
        self.start_update_scheduler().await?;

        Ok(())
    }

    pub async fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub async fn check_ip(&self, ip: &str) -> Result<GeoIPResult> {
        // Check allow list first
        if self.is_in_allow_list(ip).await {
            return Ok(GeoIPResult {
                allowed: true,
                country_code: None,
                reason: "Allow list".to_string(),
            });
        }

        // Check against deduplicated threat database
        if let Some(threat_info) = self.deduplicator.check_ip_threat(ip).await {
            return Ok(GeoIPResult {
                allowed: false,
                country_code: None,
                reason: format!("Threat database (level: {:?})", threat_info.threat_level),
            });
        }

        // Get country code from GeoIP database
        let country_code = self.get_country_code(ip).await?;

        let allowed = if let Some(country) = &country_code {
            !self.blocked_countries.contains(country)
        } else {
            true // Allow if country unknown
        };

        Ok(GeoIPResult {
            allowed,
            country_code,
            reason: if allowed {
                "Allowed country".to_string()
            } else {
                "Blocked country".to_string()
            },
        })
    }

    pub async fn block_country(&mut self, country_code: &str) -> Result<()> {
        info!("Blocking country: {}", country_code);

        if !self.blocked_countries.contains(&country_code.to_string()) {
            self.blocked_countries.push(country_code.to_string());
        }

        Ok(())
    }

    pub async fn unblock_country(&mut self, country_code: &str) -> Result<()> {
        info!("Unblocking country: {}", country_code);

        self.blocked_countries.retain(|c| c != country_code);

        Ok(())
    }

    pub async fn add_to_allow_list(&mut self, ip: &str) -> Result<()> {
        info!("Adding to GeoIP allow list: {}", ip);

        if !self.allow_list.contains(&ip.to_string()) {
            self.allow_list.push(ip.to_string());
        }

        Ok(())
    }

    async fn update_database(&self) -> Result<()> {
        info!("Updating GeoIP database from GitHub source");

        // The deduplicator handles the actual database updates
        // from https://github.com/P3TERX/GeoLite.mmdb
        // This is a free source that doesn't require MaxMind authentication

        Ok(())
    }

    async fn start_update_scheduler(&self) -> Result<()> {
        let deduplicator = self.deduplicator.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_hours(24));

            loop {
                interval.tick().await;

                if let Err(e) = deduplicator.update_geoip_database().await {
                    error!("Failed to update GeoIP database: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn is_in_allow_list(&self, ip: &str) -> bool {
        // Check exact matches first
        if self.allow_list.contains(&ip.to_string()) {
            return true;
        }

        // Check CIDR ranges
        for allowed in &self.allow_list {
            if allowed.contains('/') {
                if let Ok(network) = allowed.parse::<ipnetwork::IpNetwork>() {
                    if let Ok(test_ip) = ip.parse::<std::net::IpAddr>() {
                        if network.contains(test_ip) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    async fn get_country_code(&self, ip: &str) -> Result<Option<String>> {
        // In full implementation, this would use the maxminddb crate
        // to look up the IP in the GeoLite2 database

        // For now, return a mock result for demonstration
        if ip.starts_with("192.168.") || ip.starts_with("10.") || ip.starts_with("172.") {
            return Ok(None); // Private IP
        }

        // Mock some country codes for testing
        let mock_countries = vec![
            ("8.8.8.8", "US"),
            ("1.1.1.1", "US"),
            ("208.67.222.222", "US"),
            ("114.114.114.114", "CN"),
            ("77.88.8.8", "RU"),
        ];

        for (test_ip, country) in mock_countries {
            if ip == test_ip {
                return Ok(Some(country.to_string()));
            }
        }

        // Default to unknown
        Ok(None)
    }

    pub async fn get_blocked_countries(&self) -> Vec<String> {
        self.blocked_countries.clone()
    }

    pub async fn get_stats(&self) -> GeoIPStats {
        GeoIPStats {
            enabled: self.enabled,
            blocked_countries: self.blocked_countries.len(),
            allow_list_entries: self.allow_list.len(),
            database_last_updated: chrono::Utc::now(), // Mock timestamp
        }
    }
}

#[derive(Debug, Clone)]
pub struct GeoIPResult {
    pub allowed: bool,
    pub country_code: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct GeoIPStats {
    pub enabled: bool,
    pub blocked_countries: usize,
    pub allow_list_entries: usize,
    pub database_last_updated: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::deduplication::SecurityDeduplicator;

    #[tokio::test]
    async fn test_geoip_allow_list() {
        let deduplicator = Arc::new(SecurityDeduplicator::new().await.unwrap());
        let mut geoip = GeoIPManager::new(deduplicator).await.unwrap();

        // Test localhost
        let result = geoip.check_ip("127.0.0.1").await.unwrap();
        assert!(result.allowed);
        assert_eq!(result.reason, "Allow list");

        // Test private network
        let result = geoip.check_ip("192.168.1.100").await.unwrap();
        assert!(result.allowed);
        assert_eq!(result.reason, "Allow list");
    }

    #[tokio::test]
    async fn test_country_blocking() {
        let deduplicator = Arc::new(SecurityDeduplicator::new().await.unwrap());
        let mut geoip = GeoIPManager::new(deduplicator).await.unwrap();

        // Add a test IP to block
        geoip.block_country("TEST").await.unwrap();

        assert!(geoip.get_blocked_countries().await.contains(&"TEST".to_string()));

        // Unblock it
        geoip.unblock_country("TEST").await.unwrap();
        assert!(!geoip.get_blocked_countries().await.contains(&"TEST".to_string()));
    }
}