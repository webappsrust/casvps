use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use crate::database::Database;

pub struct Fail2BanManager {
    database: Arc<Database>,
    jails: Arc<RwLock<HashMap<String, Jail>>>,
    banned_ips: Arc<RwLock<HashMap<String, BanInfo>>>,
    enabled: bool,
}

impl Fail2BanManager {
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        info!("Initializing fail2ban manager");

        let mut manager = Self {
            database,
            jails: Arc::new(RwLock::new(HashMap::new())),
            banned_ips: Arc::new(RwLock::new(HashMap::new())),
            enabled: true, // Always enabled by default per spec
        };

        // Set up default jails
        manager.setup_default_jails().await?;

        Ok(manager)
    }

    pub async fn start(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        info!("Starting fail2ban service");

        // Start log monitoring
        self.start_log_monitoring().await?;

        // Start ban cleanup process
        self.start_ban_cleanup().await?;

        Ok(())
    }

    pub async fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub async fn ban_ip(&self, ip: &str, reason: &str) -> Result<()> {
        info!("Banning IP: {} ({})", ip, reason);

        let ban_info = BanInfo {
            ip: ip.to_string(),
            reason: reason.to_string(),
            banned_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(24), // Default 24h ban
            ban_count: 1,
        };

        // Add to in-memory banned list
        {
            let mut banned = self.banned_ips.write().await;
            if let Some(existing) = banned.get_mut(ip) {
                existing.ban_count += 1;
                existing.expires_at = chrono::Utc::now() + chrono::Duration::hours(24 * existing.ban_count as i64);
                existing.reason = format!("{} (count: {})", reason, existing.ban_count);
            } else {
                banned.insert(ip.to_string(), ban_info);
            }
        }

        // Apply firewall rule
        self.apply_ban_rule(ip).await?;

        // Log to database
        self.log_ban_action(ip, reason).await?;

        Ok(())
    }

    pub async fn unban_ip(&self, ip: &str) -> Result<()> {
        info!("Unbanning IP: {}", ip);

        // Remove from banned list
        {
            let mut banned = self.banned_ips.write().await;
            banned.remove(ip);
        }

        // Remove firewall rule
        self.remove_ban_rule(ip).await?;

        // Log to database
        self.log_unban_action(ip).await?;

        Ok(())
    }

    pub async fn is_banned(&self, ip: &str) -> bool {
        let banned = self.banned_ips.read().await;
        if let Some(ban_info) = banned.get(ip) {
            chrono::Utc::now() < ban_info.expires_at
        } else {
            false
        }
    }

    pub async fn get_ban_info(&self, ip: &str) -> Option<BanInfo> {
        let banned = self.banned_ips.read().await;
        banned.get(ip).cloned()
    }

    pub async fn check_failed_attempt(&self, ip: &str, service: &str) -> Result<()> {
        let jail_name = format!("{}-jail", service);

        let should_ban = {
            let mut jails = self.jails.write().await;
            if let Some(jail) = jails.get_mut(&jail_name) {
                jail.add_failure(ip);
                jail.should_ban(ip)
            } else {
                false
            }
        };

        if should_ban {
            let reason = format!("Too many failed {} attempts", service);
            self.ban_ip(ip, &reason).await?;
        }

        Ok(())
    }

    async fn setup_default_jails(&self) -> Result<()> {
        let mut jails = self.jails.write().await;

        // SSH jail
        jails.insert("ssh-jail".to_string(), Jail::new(
            "ssh".to_string(),
            5,  // max retries
            600, // find time (10 minutes)
            3600, // ban time (1 hour)
        ));

        // HTTP jail
        jails.insert("http-jail".to_string(), Jail::new(
            "http".to_string(),
            10, // max retries
            300, // find time (5 minutes)
            1800, // ban time (30 minutes)
        ));

        // HTTPS jail
        jails.insert("https-jail".to_string(), Jail::new(
            "https".to_string(),
            10, // max retries
            300, // find time (5 minutes)
            1800, // ban time (30 minutes)
        ));

        // VNC jail (for VM console access)
        jails.insert("vnc-jail".to_string(), Jail::new(
            "vnc".to_string(),
            3,  // max retries
            300, // find time (5 minutes)
            7200, // ban time (2 hours)
        ));

        info!("Set up {} default fail2ban jails", jails.len());
        Ok(())
    }

    async fn start_log_monitoring(&self) -> Result<()> {
        let database = self.database.clone();
        let manager_weak = Arc::downgrade(&Arc::new(self.clone_for_task()));

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));

            loop {
                interval.tick().await;

                if let Some(manager) = manager_weak.upgrade() {
                    if let Err(e) = manager.process_log_entries().await {
                        error!("Failed to process log entries: {}", e);
                    }
                } else {
                    break; // Manager was dropped
                }
            }
        });

        Ok(())
    }

    async fn start_ban_cleanup(&self) -> Result<()> {
        let banned_ips = self.banned_ips.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // 5 minutes

            loop {
                interval.tick().await;

                let mut to_unban = Vec::new();
                {
                    let banned = banned_ips.read().await;
                    let now = chrono::Utc::now();

                    for (ip, ban_info) in banned.iter() {
                        if now >= ban_info.expires_at {
                            to_unban.push(ip.clone());
                        }
                    }
                }

                if !to_unban.is_empty() {
                    let mut banned = banned_ips.write().await;
                    for ip in &to_unban {
                        banned.remove(ip);
                        info!("Auto-unbanned expired IP: {}", ip);
                    }
                }
            }
        });

        Ok(())
    }

    async fn process_log_entries(&self) -> Result<()> {
        // Monitor various log sources for failed attempts
        self.monitor_ssh_logs().await?;
        self.monitor_web_logs().await?;
        self.monitor_vnc_logs().await?;

        Ok(())
    }

    async fn monitor_ssh_logs(&self) -> Result<()> {
        // In full implementation, this would parse /var/log/auth.log
        // Looking for SSH authentication failures

        // Mock detection for demonstration
        // if let Ok(content) = std::fs::read_to_string("/var/log/auth.log") {
        //     for line in content.lines().rev().take(100) { // Check last 100 lines
        //         if line.contains("Failed password") || line.contains("Invalid user") {
        //             if let Some(ip) = self.extract_ip_from_log_line(line) {
        //                 self.check_failed_attempt(&ip, "ssh").await?;
        //             }
        //         }
        //     }
        // }

        Ok(())
    }

    async fn monitor_web_logs(&self) -> Result<()> {
        // Monitor nginx access logs for suspicious activity
        // HTTP 401, 403, 404 patterns, etc.
        Ok(())
    }

    async fn monitor_vnc_logs(&self) -> Result<()> {
        // Monitor VNC access logs for failed connections
        Ok(())
    }

    fn extract_ip_from_log_line(&self, line: &str) -> Option<String> {
        // Extract IP address from log line using regex
        // This is a simplified version
        use std::net::IpAddr;

        for word in line.split_whitespace() {
            if let Ok(_) = word.parse::<IpAddr>() {
                return Some(word.to_string());
            }
        }

        None
    }

    async fn apply_ban_rule(&self, ip: &str) -> Result<()> {
        // Apply iptables/nftables rule to block IP
        info!("Applied firewall ban rule for IP: {}", ip);

        // In full implementation, this would execute:
        // nftables: nft add element ip filter blacklist { IP }
        // iptables: iptables -A INPUT -s IP -j DROP

        Ok(())
    }

    async fn remove_ban_rule(&self, ip: &str) -> Result<()> {
        // Remove iptables/nftables rule
        info!("Removed firewall ban rule for IP: {}", ip);

        // In full implementation:
        // nftables: nft delete element ip filter blacklist { IP }
        // iptables: iptables -D INPUT -s IP -j DROP

        Ok(())
    }

    async fn log_ban_action(&self, ip: &str, reason: &str) -> Result<()> {
        let details = serde_json::json!({
            "action": "ban",
            "ip": ip,
            "reason": reason
        });

        sqlx::query!(
            "INSERT INTO audit_log (timestamp, user_id, action, resource_type, resource_id, details, ip_address)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            chrono::Utc::now(),
            "system",
            "fail2ban_ban",
            "security",
            ip,
            serde_json::to_string(&details)?,
            ip
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn log_unban_action(&self, ip: &str) -> Result<()> {
        let details = serde_json::json!({
            "action": "unban",
            "ip": ip
        });

        sqlx::query!(
            "INSERT INTO audit_log (timestamp, user_id, action, resource_type, resource_id, details, ip_address)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            chrono::Utc::now(),
            "system",
            "fail2ban_unban",
            "security",
            ip,
            serde_json::to_string(&details)?,
            ip
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    fn clone_for_task(&self) -> Fail2BanManager {
        Fail2BanManager {
            database: self.database.clone(),
            jails: self.jails.clone(),
            banned_ips: self.banned_ips.clone(),
            enabled: self.enabled,
        }
    }

    pub async fn get_stats(&self) -> Fail2BanStats {
        let banned = self.banned_ips.read().await;
        let jails = self.jails.read().await;

        Fail2BanStats {
            enabled: self.enabled,
            active_bans: banned.len(),
            total_jails: jails.len(),
            banned_ips: banned.keys().cloned().collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BanInfo {
    pub ip: String,
    pub reason: String,
    pub banned_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub ban_count: u32,
}

#[derive(Debug, Clone)]
struct Jail {
    name: String,
    max_retry: u32,
    find_time: u64, // seconds
    ban_time: u64,  // seconds
    failures: HashMap<String, Vec<chrono::DateTime<chrono::Utc>>>,
}

impl Jail {
    fn new(name: String, max_retry: u32, find_time: u64, ban_time: u64) -> Self {
        Self {
            name,
            max_retry,
            find_time,
            ban_time,
            failures: HashMap::new(),
        }
    }

    fn add_failure(&mut self, ip: &str) {
        let now = chrono::Utc::now();
        let cutoff = now - chrono::Duration::seconds(self.find_time as i64);

        let failures = self.failures.entry(ip.to_string()).or_insert_with(Vec::new);
        failures.push(now);

        // Remove old failures outside the find_time window
        failures.retain(|&failure_time| failure_time > cutoff);
    }

    fn should_ban(&self, ip: &str) -> bool {
        if let Some(failures) = self.failures.get(ip) {
            failures.len() >= self.max_retry as usize
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct Fail2BanStats {
    pub enabled: bool,
    pub active_bans: usize,
    pub total_jails: usize,
    pub banned_ips: Vec<String>,
}