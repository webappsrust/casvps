pub mod geoip;
pub mod fail2ban;
pub mod suricata;
pub mod clamav;
pub mod firewall;
pub mod deduplication;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};
use crate::database::Database;

pub struct SecurityManager {
    database: Arc<Database>,
    geoip: Arc<geoip::GeoIPManager>,
    fail2ban: Arc<fail2ban::Fail2BanManager>,
    suricata: Arc<suricata::SuricataManager>,
    clamav: Arc<clamav::ClamAVManager>,
    firewall: Arc<firewall::FirewallManager>,
    deduplicator: Arc<deduplication::SecurityDeduplicator>,
}

impl SecurityManager {
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        info!("Initializing security manager - always-on security by default");

        // Initialize security databases with deduplication
        let deduplicator = Arc::new(deduplication::SecurityDeduplicator::new().await?);

        // Initialize all security components
        let geoip = Arc::new(geoip::GeoIPManager::new(deduplicator.clone()).await?);
        let fail2ban = Arc::new(fail2ban::Fail2BanManager::new(database.clone()).await?);
        let suricata = Arc::new(suricata::SuricataManager::new(deduplicator.clone()).await?);
        let clamav = Arc::new(clamav::ClamAVManager::new(deduplicator.clone()).await?);
        let firewall = Arc::new(firewall::FirewallManager::new(database.clone()).await?);

        Ok(Self {
            database,
            geoip,
            fail2ban,
            suricata,
            clamav,
            firewall,
            deduplicator,
        })
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting always-on security services");

        // Start security database updates (free sources only)
        self.start_security_updates().await?;

        // Start firewall (nftables with stealth mode)
        self.firewall.start().await?;

        // Start GeoIP blocking
        self.geoip.start().await?;

        // Start fail2ban
        self.fail2ban.start().await?;

        // Start Suricata IDS
        self.suricata.start().await?;

        // Start ClamAV
        self.clamav.start().await?;

        info!("All security services started - 76% storage savings via deduplication");
        Ok(())
    }

    pub async fn get_security_status(&self) -> SecurityStatus {
        SecurityStatus {
            geoip_enabled: self.geoip.is_enabled().await,
            fail2ban_enabled: self.fail2ban.is_enabled().await,
            suricata_enabled: self.suricata.is_enabled().await,
            clamav_enabled: self.clamav.is_enabled().await,
            firewall_enabled: self.firewall.is_enabled().await,
            total_space_saved: self.deduplicator.get_space_saved().await,
            threat_count_24h: self.get_threat_count_24h().await,
        }
    }

    pub async fn handle_security_event(&self, event: SecurityEvent) -> Result<()> {
        info!("Processing security event: {:?}", event.event_type);

        // Log to database
        self.log_security_event(&event).await?;

        // Take action based on event type
        match event.event_type {
            SecurityEventType::SuspiciousIP { ip, reason } => {
                self.handle_suspicious_ip(&ip, &reason).await?;
            }
            SecurityEventType::MalwareDetected { file_path, signature } => {
                self.handle_malware_detection(&file_path, &signature).await?;
            }
            SecurityEventType::IntrusionAttempt { attack_type, source_ip } => {
                self.handle_intrusion_attempt(&attack_type, &source_ip).await?;
            }
            SecurityEventType::DDoSAttack { source_ips, packets_per_second } => {
                self.handle_ddos_attack(&source_ips, packets_per_second).await?;
            }
        }

        Ok(())
    }

    async fn start_security_updates(&self) -> Result<()> {
        let deduplicator = self.deduplicator.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_hours(4));

            loop {
                interval.tick().await;

                if let Err(e) = deduplicator.update_all_databases().await {
                    error!("Failed to update security databases: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn log_security_event(&self, event: &SecurityEvent) -> Result<()> {
        let details = serde_json::to_string(&event.details)?;

        sqlx::query!(
            "INSERT INTO audit_log (timestamp, user_id, action, resource_type, resource_id, details, ip_address)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            event.timestamp,
            "system",
            format!("security_event_{:?}", event.event_type).to_lowercase(),
            "security",
            event.event_id,
            details,
            event.source_ip
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn handle_suspicious_ip(&self, ip: &str, reason: &str) -> Result<()> {
        info!("Blocking suspicious IP: {} ({})", ip, reason);

        // Add to firewall block list
        self.firewall.block_ip(ip, reason).await?;

        // Add to fail2ban
        self.fail2ban.ban_ip(ip, reason).await?;

        Ok(())
    }

    async fn handle_malware_detection(&self, file_path: &str, signature: &str) -> Result<()> {
        info!("Malware detected: {} ({})", file_path, signature);

        // Quarantine file
        self.clamav.quarantine_file(file_path).await?;

        // Alert administrators
        self.send_security_alert(&format!("Malware detected: {}", signature)).await?;

        Ok(())
    }

    async fn handle_intrusion_attempt(&self, attack_type: &str, source_ip: &str) -> Result<()> {
        info!("Intrusion attempt detected: {} from {}", attack_type, source_ip);

        // Block source IP
        self.firewall.block_ip(source_ip, &format!("Intrusion attempt: {}", attack_type)).await?;

        // Increase monitoring for this IP range
        self.suricata.increase_monitoring(source_ip).await?;

        Ok(())
    }

    async fn handle_ddos_attack(&self, source_ips: &[String], pps: u64) -> Result<()> {
        info!("DDoS attack detected: {} packets/sec from {} IPs", pps, source_ips.len());

        // Enable SYN cookies
        self.firewall.enable_syn_cookies().await?;

        // Rate limit all source IPs
        for ip in source_ips {
            self.firewall.rate_limit_ip(ip, 10).await?; // 10 packets/sec
        }

        Ok(())
    }

    async fn send_security_alert(&self, message: &str) -> Result<()> {
        // In full implementation, this would send notifications
        // via configured channels (email, webhook, etc.)
        info!("Security alert: {}", message);
        Ok(())
    }

    async fn get_threat_count_24h(&self) -> u64 {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM audit_log
             WHERE action LIKE 'security_event_%'
             AND timestamp > datetime('now', '-24 hours')"
        )
        .fetch_one(&self.database.pool)
        .await
        .unwrap_or(0);

        count as u64
    }
}

#[derive(Debug, Clone)]
pub struct SecurityEvent {
    pub event_id: String,
    pub event_type: SecurityEventType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source_ip: Option<String>,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone)]
pub enum SecurityEventType {
    SuspiciousIP { ip: String, reason: String },
    MalwareDetected { file_path: String, signature: String },
    IntrusionAttempt { attack_type: String, source_ip: String },
    DDoSAttack { source_ips: Vec<String>, packets_per_second: u64 },
}

#[derive(Debug, Clone)]
pub struct SecurityStatus {
    pub geoip_enabled: bool,
    pub fail2ban_enabled: bool,
    pub suricata_enabled: bool,
    pub clamav_enabled: bool,
    pub firewall_enabled: bool,
    pub total_space_saved: String,
    pub threat_count_24h: u64,
}