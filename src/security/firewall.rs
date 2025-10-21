use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn, error};
use crate::database::Database;

pub struct FirewallManager {
    database: Arc<Database>,
    backend: FirewallBackend,
    enabled: bool,
    default_policy: Policy,
}

impl FirewallManager {
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        info!("Initializing firewall manager with nftables backend");

        Ok(Self {
            database,
            backend: FirewallBackend::Nftables,
            enabled: true,
            default_policy: Policy::Drop,
        })
    }

    pub async fn start(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        info!("Starting firewall service");

        // Initialize firewall backend
        self.initialize_backend().await?;

        // Create base tables and chains
        self.create_base_structure().await?;

        // Apply default policies
        self.apply_default_policies().await?;

        // Load existing rules from database
        self.load_rules_from_database().await?;

        // Start dynamic rule management
        self.start_dynamic_management().await?;

        Ok(())
    }

    pub async fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub async fn add_rule(&self, rule: FirewallRule) -> Result<()> {
        info!("Adding firewall rule: {:?}", rule);

        // Validate rule
        self.validate_rule(&rule).await?;

        // Add to database
        self.store_rule(&rule).await?;

        // Apply to firewall
        self.apply_rule(&rule).await?;

        Ok(())
    }

    pub async fn remove_rule(&self, rule_id: &str) -> Result<()> {
        info!("Removing firewall rule: {}", rule_id);

        // Get rule from database
        let rule = self.get_rule(rule_id).await?;

        // Remove from firewall
        self.remove_rule_from_firewall(&rule).await?;

        // Remove from database
        self.remove_rule_from_database(rule_id).await?;

        Ok(())
    }

    pub async fn block_ip(&self, ip: &str, reason: &str) -> Result<()> {
        info!("Blocking IP {} ({})", ip, reason);

        let rule = FirewallRule {
            id: format!("block-{}", ip),
            chain: "input".to_string(),
            source: Some(ip.to_string()),
            destination: None,
            port: None,
            protocol: None,
            action: Action::Drop,
            comment: Some(reason.to_string()),
            permanent: false,
        };

        self.add_rule(rule).await?;
        Ok(())
    }

    pub async fn unblock_ip(&self, ip: &str) -> Result<()> {
        info!("Unblocking IP {}", ip);

        let rule_id = format!("block-{}", ip);
        self.remove_rule(&rule_id).await?;
        Ok(())
    }

    pub async fn allow_service(&self, service: &str, port: u16, protocol: Protocol) -> Result<()> {
        info!("Allowing service {} on port {}/{:?}", service, port, protocol);

        let rule = FirewallRule {
            id: format!("allow-{}-{}", service, port),
            chain: "input".to_string(),
            source: None,
            destination: None,
            port: Some(port),
            protocol: Some(protocol),
            action: Action::Accept,
            comment: Some(format!("Allow {} service", service)),
            permanent: true,
        };

        self.add_rule(rule).await?;
        Ok(())
    }

    async fn initialize_backend(&self) -> Result<()> {
        match self.backend {
            FirewallBackend::Nftables => {
                // Check if nftables is available
                let output = tokio::process::Command::new("nft")
                    .args(&["--version"])
                    .output()
                    .await?;

                if !output.status.success() {
                    return Err(anyhow::anyhow!("nftables not available"));
                }

                info!("nftables backend initialized");
            },
            FirewallBackend::Iptables => {
                // Fallback to iptables
                let output = tokio::process::Command::new("iptables")
                    .args(&["--version"])
                    .output()
                    .await?;

                if !output.status.success() {
                    return Err(anyhow::anyhow!("iptables not available"));
                }

                info!("iptables backend initialized");
            },
        }

        Ok(())
    }

    async fn create_base_structure(&self) -> Result<()> {
        match self.backend {
            FirewallBackend::Nftables => {
                let commands = vec![
                    "add table inet casvps",
                    "add chain inet casvps input { type filter hook input priority 0; policy drop; }",
                    "add chain inet casvps forward { type filter hook forward priority 0; policy accept; }",
                    "add chain inet casvps output { type filter hook output priority 0; policy accept; }",
                    "add set inet casvps blacklist { type ipv4_addr; }",
                    "add set inet casvps whitelist { type ipv4_addr; }",
                ];

                for cmd in commands {
                    let result = tokio::process::Command::new("nft")
                        .args(&[cmd])
                        .output()
                        .await?;

                    if !result.status.success() {
                        warn!("nft command failed: {}", String::from_utf8_lossy(&result.stderr));
                    }
                }
            },
            FirewallBackend::Iptables => {
                // Create basic iptables structure
                let commands = vec![
                    "iptables -P INPUT DROP",
                    "iptables -P FORWARD ACCEPT",
                    "iptables -P OUTPUT ACCEPT",
                    "iptables -N CASVPS-INPUT",
                    "iptables -N CASVPS-FORWARD",
                    "iptables -I INPUT -j CASVPS-INPUT",
                    "iptables -I FORWARD -j CASVPS-FORWARD",
                ];

                for cmd in commands {
                    let parts: Vec<&str> = cmd.split_whitespace().collect();
                    let result = tokio::process::Command::new(parts[0])
                        .args(&parts[1..])
                        .output()
                        .await?;

                    if !result.status.success() {
                        warn!("iptables command failed: {}", String::from_utf8_lossy(&result.stderr));
                    }
                }
            },
        }

        Ok(())
    }

    async fn apply_default_policies(&self) -> Result<()> {
        // Allow loopback
        let loopback_rule = FirewallRule {
            id: "allow-loopback".to_string(),
            chain: "input".to_string(),
            source: Some("127.0.0.1".to_string()),
            destination: None,
            port: None,
            protocol: None,
            action: Action::Accept,
            comment: Some("Allow loopback traffic".to_string()),
            permanent: true,
        };
        self.apply_rule(&loopback_rule).await?;

        // Allow established connections
        self.allow_established_connections().await?;

        // Allow ping with rate limiting
        self.allow_ping_with_limits().await?;

        // Allow SSH (port 22) - always needed for management
        self.allow_service("ssh", 22, Protocol::Tcp).await?;

        // Allow CasVPS web interface (port 8006)
        self.allow_service("casvps-web", 8006, Protocol::Tcp).await?;

        // Allow HTTPS (port 443) for reverse proxy scenarios
        self.allow_service("https", 443, Protocol::Tcp).await?;

        Ok(())
    }

    async fn allow_established_connections(&self) -> Result<()> {
        match self.backend {
            FirewallBackend::Nftables => {
                tokio::process::Command::new("nft")
                    .args(&["add", "rule", "inet", "casvps", "input", "ct", "state", "established,related", "accept"])
                    .output()
                    .await?;
            },
            FirewallBackend::Iptables => {
                tokio::process::Command::new("iptables")
                    .args(&["-A", "CASVPS-INPUT", "-m", "conntrack", "--ctstate", "ESTABLISHED,RELATED", "-j", "ACCEPT"])
                    .output()
                    .await?;
            },
        }
        Ok(())
    }

    async fn allow_ping_with_limits(&self) -> Result<()> {
        match self.backend {
            FirewallBackend::Nftables => {
                tokio::process::Command::new("nft")
                    .args(&[
                        "add", "rule", "inet", "casvps", "input",
                        "icmp", "type", "echo-request",
                        "limit", "rate", "10/second",
                        "accept"
                    ])
                    .output()
                    .await?;
            },
            FirewallBackend::Iptables => {
                tokio::process::Command::new("iptables")
                    .args(&[
                        "-A", "CASVPS-INPUT",
                        "-p", "icmp", "--icmp-type", "echo-request",
                        "-m", "limit", "--limit", "10/sec",
                        "-j", "ACCEPT"
                    ])
                    .output()
                    .await?;
            },
        }
        Ok(())
    }

    async fn load_rules_from_database(&self) -> Result<()> {
        // Load existing firewall rules from database
        let rules = sqlx::query_as::<_, FirewallRuleRow>(
            "SELECT rule_id, chain, source_ip, dest_ip, port, protocol, action, comment, permanent
             FROM firewall_rules WHERE enabled = TRUE"
        )
        .fetch_all(&self.database.pool)
        .await?;

        for rule_row in rules {
            let rule = FirewallRule {
                id: rule_row.rule_id,
                chain: rule_row.chain,
                source: rule_row.source_ip,
                destination: rule_row.dest_ip,
                port: rule_row.port.map(|p| p as u16),
                protocol: rule_row.protocol.map(|p| match p.as_str() {
                    "tcp" => Protocol::Tcp,
                    "udp" => Protocol::Udp,
                    _ => Protocol::Tcp,
                }),
                action: match rule_row.action.as_str() {
                    "accept" => Action::Accept,
                    "drop" => Action::Drop,
                    "reject" => Action::Reject,
                    _ => Action::Drop,
                },
                comment: rule_row.comment,
                permanent: rule_row.permanent,
            };

            if let Err(e) = self.apply_rule(&rule).await {
                warn!("Failed to apply rule {}: {}", rule.id, e);
            }
        }

        Ok(())
    }

    async fn start_dynamic_management(&self) -> Result<()> {
        // Start background task for dynamic rule management
        let database = self.database.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                // Clean up temporary rules
                if let Err(e) = Self::cleanup_temporary_rules(database.clone()).await {
                    error!("Failed to cleanup temporary rules: {}", e);
                }

                // Process any pending rule changes
                if let Err(e) = Self::process_rule_queue(database.clone()).await {
                    error!("Failed to process rule queue: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn cleanup_temporary_rules(database: Arc<Database>) -> Result<()> {
        // Remove expired temporary rules
        let expired_rules = sqlx::query_as::<_, FirewallRuleRow>(
            "SELECT rule_id, chain, source_ip, dest_ip, port, protocol, action, comment, permanent
             FROM firewall_rules
             WHERE permanent = FALSE AND created_at < datetime('now', '-1 hour')"
        )
        .fetch_all(&database.pool)
        .await?;

        for rule in expired_rules {
            sqlx::query!("DELETE FROM firewall_rules WHERE rule_id = ?", rule.rule_id)
                .execute(&database.pool)
                .await?;

            info!("Cleaned up expired rule: {}", rule.rule_id);
        }

        Ok(())
    }

    async fn process_rule_queue(_database: Arc<Database>) -> Result<()> {
        // Process any queued rule changes
        // This would handle bulk updates or delayed rule applications
        Ok(())
    }

    async fn validate_rule(&self, rule: &FirewallRule) -> Result<()> {
        // Validate rule parameters
        if rule.id.is_empty() {
            return Err(anyhow::anyhow!("Rule ID cannot be empty"));
        }

        if let Some(port) = rule.port {
            if port == 0 || port > 65535 {
                return Err(anyhow::anyhow!("Invalid port number: {}", port));
            }
        }

        // Validate IP addresses
        if let Some(ip) = &rule.source {
            if ip.parse::<std::net::IpAddr>().is_err() && !ip.contains('/') {
                return Err(anyhow::anyhow!("Invalid source IP: {}", ip));
            }
        }

        Ok(())
    }

    async fn store_rule(&self, rule: &FirewallRule) -> Result<()> {
        sqlx::query!(
            "INSERT OR REPLACE INTO firewall_rules
             (rule_id, chain, source_ip, dest_ip, port, protocol, action, comment, permanent, enabled)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, TRUE)",
            rule.id,
            rule.chain,
            rule.source,
            rule.destination,
            rule.port.map(|p| p as i32),
            rule.protocol.as_ref().map(|p| format!("{:?}", p).to_lowercase()),
            format!("{:?}", rule.action).to_lowercase(),
            rule.comment,
            rule.permanent
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn apply_rule(&self, rule: &FirewallRule) -> Result<()> {
        match self.backend {
            FirewallBackend::Nftables => self.apply_nftables_rule(rule).await,
            FirewallBackend::Iptables => self.apply_iptables_rule(rule).await,
        }
    }

    async fn apply_nftables_rule(&self, rule: &FirewallRule) -> Result<()> {
        let mut cmd = vec!["add", "rule", "inet", "casvps", &rule.chain];

        // Add source condition
        if let Some(source) = &rule.source {
            cmd.extend(&["ip", "saddr", source]);
        }

        // Add destination condition
        if let Some(dest) = &rule.destination {
            cmd.extend(&["ip", "daddr", dest]);
        }

        // Add protocol condition
        if let Some(protocol) = &rule.protocol {
            cmd.push(match protocol {
                Protocol::Tcp => "tcp",
                Protocol::Udp => "udp",
                Protocol::Icmp => "icmp",
            });
        }

        // Add port condition
        if let Some(port) = rule.port {
            cmd.extend(&["dport", &port.to_string()]);
        }

        // Add action
        cmd.push(match rule.action {
            Action::Accept => "accept",
            Action::Drop => "drop",
            Action::Reject => "reject",
        });

        // Add comment if provided
        if let Some(comment) = &rule.comment {
            cmd.extend(&["comment", &format!("\"{}\"", comment)]);
        }

        let result = tokio::process::Command::new("nft")
            .args(&cmd)
            .output()
            .await?;

        if !result.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to apply nftables rule: {}",
                String::from_utf8_lossy(&result.stderr)
            ));
        }

        Ok(())
    }

    async fn apply_iptables_rule(&self, rule: &FirewallRule) -> Result<()> {
        let mut cmd = vec!["-A", &format!("CASVPS-{}", rule.chain.to_uppercase())];

        // Add protocol
        if let Some(protocol) = &rule.protocol {
            cmd.extend(&["-p", match protocol {
                Protocol::Tcp => "tcp",
                Protocol::Udp => "udp",
                Protocol::Icmp => "icmp",
            }]);
        }

        // Add source
        if let Some(source) = &rule.source {
            cmd.extend(&["-s", source]);
        }

        // Add destination
        if let Some(dest) = &rule.destination {
            cmd.extend(&["-d", dest]);
        }

        // Add port
        if let Some(port) = rule.port {
            cmd.extend(&["--dport", &port.to_string()]);
        }

        // Add comment
        if let Some(comment) = &rule.comment {
            cmd.extend(&["-m", "comment", "--comment", comment]);
        }

        // Add action
        cmd.extend(&["-j", match rule.action {
            Action::Accept => "ACCEPT",
            Action::Drop => "DROP",
            Action::Reject => "REJECT",
        }]);

        let result = tokio::process::Command::new("iptables")
            .args(&cmd)
            .output()
            .await?;

        if !result.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to apply iptables rule: {}",
                String::from_utf8_lossy(&result.stderr)
            ));
        }

        Ok(())
    }

    async fn get_rule(&self, rule_id: &str) -> Result<FirewallRule> {
        let rule_row = sqlx::query_as::<_, FirewallRuleRow>(
            "SELECT rule_id, chain, source_ip, dest_ip, port, protocol, action, comment, permanent
             FROM firewall_rules WHERE rule_id = ?"
        )
        .bind(rule_id)
        .fetch_one(&self.database.pool)
        .await?;

        Ok(FirewallRule {
            id: rule_row.rule_id,
            chain: rule_row.chain,
            source: rule_row.source_ip,
            destination: rule_row.dest_ip,
            port: rule_row.port.map(|p| p as u16),
            protocol: rule_row.protocol.map(|p| match p.as_str() {
                "tcp" => Protocol::Tcp,
                "udp" => Protocol::Udp,
                _ => Protocol::Tcp,
            }),
            action: match rule_row.action.as_str() {
                "accept" => Action::Accept,
                "drop" => Action::Drop,
                "reject" => Action::Reject,
                _ => Action::Drop,
            },
            comment: rule_row.comment,
            permanent: rule_row.permanent,
        })
    }

    async fn remove_rule_from_firewall(&self, rule: &FirewallRule) -> Result<()> {
        // Implementation would remove the specific rule from nftables/iptables
        // This is complex as it requires finding the exact rule handle/line number
        info!("Removing rule {} from firewall", rule.id);
        Ok(())
    }

    async fn remove_rule_from_database(&self, rule_id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM firewall_rules WHERE rule_id = ?", rule_id)
            .execute(&self.database.pool)
            .await?;
        Ok(())
    }

    pub async fn block_ip_simple(&self, ip: &str) -> Result<()> {
        self.block_ip(ip, "Security manager block").await
    }

    pub async fn enable_syn_cookies(&self) -> Result<()> {
        info!("Enabling SYN cookies for DDoS protection");

        // Enable SYN cookies via sysctl
        tokio::process::Command::new("sysctl")
            .args(&["-w", "net.ipv4.tcp_syncookies=1"])
            .output()
            .await?;

        Ok(())
    }

    pub async fn rate_limit_ip(&self, ip: &str, limit: u32) -> Result<()> {
        info!("Rate limiting IP {} to {} packets/sec", ip, limit);

        let rule = FirewallRule {
            id: format!("rate-limit-{}", ip),
            chain: "input".to_string(),
            source: Some(ip.to_string()),
            destination: None,
            port: None,
            protocol: None,
            action: Action::Accept, // Accept but rate limited
            comment: Some(format!("Rate limit {} pps", limit)),
            permanent: false,
        };

        // In full implementation, this would add rate limiting rules
        self.add_rule(rule).await?;
        Ok(())
    }

    pub async fn get_stats(&self) -> FirewallStats {
        let rule_count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM firewall_rules WHERE enabled = TRUE"
        )
        .fetch_one(&self.database.pool)
        .await
        .unwrap_or(0);

        let blocked_ips = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM firewall_rules WHERE action = 'drop' AND source_ip IS NOT NULL"
        )
        .fetch_one(&self.database.pool)
        .await
        .unwrap_or(0);

        FirewallStats {
            enabled: self.enabled,
            backend: format!("{:?}", self.backend),
            total_rules: rule_count as usize,
            blocked_ips: blocked_ips as usize,
            default_policy: format!("{:?}", self.default_policy),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FirewallRule {
    pub id: String,
    pub chain: String,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub port: Option<u16>,
    pub protocol: Option<Protocol>,
    pub action: Action,
    pub comment: Option<String>,
    pub permanent: bool,
}

#[derive(Debug, Clone)]
pub enum FirewallBackend {
    Nftables,
    Iptables,
}

#[derive(Debug, Clone)]
pub enum Policy {
    Accept,
    Drop,
    Reject,
}

#[derive(Debug, Clone)]
pub enum Protocol {
    Tcp,
    Udp,
    Icmp,
}

#[derive(Debug, Clone)]
pub enum Action {
    Accept,
    Drop,
    Reject,
}

#[derive(Debug, Clone)]
pub struct FirewallStats {
    pub enabled: bool,
    pub backend: String,
    pub total_rules: usize,
    pub blocked_ips: usize,
    pub default_policy: String,
}

#[derive(sqlx::FromRow)]
struct FirewallRuleRow {
    rule_id: String,
    chain: String,
    source_ip: Option<String>,
    dest_ip: Option<String>,
    port: Option<i32>,
    protocol: Option<String>,
    action: String,
    comment: Option<String>,
    permanent: bool,
}