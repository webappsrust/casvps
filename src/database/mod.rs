use anyhow::Result;
use sqlx::{SqlitePool, Row};
use std::path::Path;
use tracing::{info, debug};
use uuid::Uuid;
use serde_json::Value as JsonValue;

pub mod schema;

use schema::*;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(path: &str) -> Result<Self> {
        // Create database file if it doesn't exist
        if !Path::new(path).exists() {
            std::fs::File::create(path)?;
        }

        let connection_str = format!("sqlite:{}", path);
        let pool = SqlitePool::connect(&connection_str).await?;

        Ok(Self { pool })
    }

    pub async fn initialize_schema(&self) -> Result<()> {
        info!("Initializing database schema");

        // Run all schema creation statements
        sqlx::query(&SCHEMA_SYSTEM_CONFIG).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_USERS).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_VMS).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_CONTAINERS).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_API_TOKENS).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_BOOT_ORDER).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_ISO_LIBRARY).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_SNAPSHOTS).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_BACKUP_JOBS).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_CLUSTER_NODES).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_USER_NETWORKS).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_IP_ALLOCATIONS).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_CERTIFICATES).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_RECOVERY_RULES).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_SCHEDULED_TASKS).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_COMPLIANCE_CONFIG).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_AUDIT_LOG).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_USERNAME_BLACKLIST).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_AUTH_REALMS).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_NOTIFICATION_CHANNELS).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_NOTIFICATION_RULES).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_REPORT_TEMPLATES).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_IP_NETWORKS).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_MONITORED_SERVICES).execute(&self.pool).await?;
        sqlx::query(&SCHEMA_SYSCTL_CONFIG).execute(&self.pool).await?;

        // Insert default data
        self.insert_default_data().await?;

        Ok(())
    }

    async fn insert_default_data(&self) -> Result<()> {
        // Insert username blacklist
        let blacklist = vec![
            "root", "admin", "administrator", "system",
            "daemon", "bin", "sys", "sync", "games",
            "man", "lp", "mail", "news", "uucp",
            "proxy", "www-data", "backup", "list",
            "irc", "gnats", "nobody", "systemd-network",
        ];

        for username in blacklist {
            sqlx::query("INSERT OR IGNORE INTO username_blacklist (username) VALUES (?)")
                .bind(username)
                .execute(&self.pool)
                .await?;
        }

        // Insert default configuration (Pi4 baseline)
        self.set_config("overprovisioning.cpu.ratio", "4.0").await?;
        self.set_config("overprovisioning.memory.ratio", "1.5").await?;
        self.set_config("overprovisioning.storage.ratio", "2.0").await?;
        self.set_config("overprovisioning.auto_adjust", "true").await?;

        // Memory settings
        self.set_config("vm.overcommit_memory", "1").await?;
        self.set_config("vm.overcommit_ratio", "150").await?;
        self.set_config("kernel.mm.ksm.run", "1").await?;
        self.set_config("vm.swappiness", "10").await?;

        // Network settings
        self.set_config("net.ipv4.ip_forward", "1").await?;
        self.set_config("net.ipv6.conf.all.forwarding", "1").await?;

        // Security (always on by default)
        self.set_config("security.geoip.enabled", "true").await?;
        self.set_config("security.fail2ban.enabled", "true").await?;
        self.set_config("security.suricata.enabled", "true").await?;
        self.set_config("security.clamav.enabled", "true").await?;
        self.set_config("security.firewall.enabled", "true").await?;

        // Compliance (off by default)
        self.set_config("compliance.hipaa.enabled", "false").await?;
        self.set_config("compliance.pci.enabled", "false").await?;
        self.set_config("compliance.sox.enabled", "false").await?;
        self.set_config("compliance.gdpr.enabled", "false").await?;

        // Default scheduled tasks
        self.insert_scheduled_task("backup_vms", "0 2 * * *", "backup.run_all()").await?;
        self.insert_scheduled_task("snapshot_vms", "0 1 * * *", "snapshot.run_all()").await?;
        self.insert_scheduled_task("check_updates", "0 3 * * *", "update.check()").await?;
        self.insert_scheduled_task("cleanup", "0 5 * * *", "cleanup.run()").await?;
        self.insert_scheduled_task("cert_renewal", "0 0 * * *", "certs.check_renew()").await?;

        // Default recovery rules
        self.insert_recovery_rule("vm_crash", "vm_state=crashed", "restart_vm").await?;
        self.insert_recovery_rule("storage_full", "storage_usage>95", "cleanup_snapshots").await?;
        self.insert_recovery_rule("memory_high", "memory_usage>90", "increase_ksm").await?;
        self.insert_recovery_rule("cert_expiry", "cert_days<7", "renew_certificate").await?;
        self.insert_recovery_rule("network_down", "bridge_state=down", "recreate_bridge").await?;

        Ok(())
    }

    pub async fn generate_node_uuid(&self) -> Result<String> {
        let uuid = Uuid::new_v4().to_string();
        self.set_config("node.uuid", &uuid).await?;
        Ok(uuid)
    }

    pub async fn set_default_config(&self) -> Result<()> {
        // Set platform-specific defaults
        let sys = sysinfo::System::new_all();
        let total_memory = sys.total_memory();

        if total_memory < 8 * 1024 * 1024 * 1024 {
            // Pi4 mode
            self.set_config("platform.mode", "pi4").await?;
            self.set_config("resources.memory.reserved", "512MB").await?;
            self.set_config("vms.max_count", "5").await?;
        } else {
            // Homelab or enterprise
            self.set_config("platform.mode", "homelab").await?;
            self.set_config("resources.memory.reserved", "2GB").await?;
            self.set_config("vms.max_count", "100").await?;
        }

        Ok(())
    }

    pub async fn get_config(&self, key: &str) -> Result<String> {
        let row = sqlx::query("SELECT value FROM system_config WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let value: JsonValue = row.get("value");
                Ok(value.as_str().unwrap_or("").to_string())
            }
            None => Ok(String::new()),
        }
    }

    pub async fn set_config(&self, key: &str, value: &str) -> Result<()> {
        let json_value = serde_json::json!(value);

        sqlx::query(
            "INSERT INTO system_config (key, value, category) VALUES (?, ?, ?)
             ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = CURRENT_TIMESTAMP"
        )
        .bind(key)
        .bind(&json_value)
        .bind(Self::get_category_from_key(key))
        .bind(&json_value)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn verify_integrity(&self) -> Result<()> {
        // Run integrity check
        let result = sqlx::query("PRAGMA integrity_check")
            .fetch_one(&self.pool)
            .await?;

        let check: String = result.get(0);
        if check != "ok" {
            return Err(anyhow::anyhow!("Database integrity check failed: {}", check));
        }

        debug!("Database integrity check passed");
        Ok(())
    }

    pub async fn get_cluster_id(&self) -> Result<Option<String>> {
        let row = sqlx::query("SELECT value FROM system_config WHERE key = 'cluster.id'")
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let value: JsonValue = row.get("value");
                Ok(value.as_str().map(String::from))
            }
            None => Ok(None),
        }
    }

    async fn insert_scheduled_task(&self, name: &str, schedule: &str, command: &str) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO scheduled_tasks (task_id, name, schedule, command, enabled)
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(Uuid::new_v4().to_string())
        .bind(name)
        .bind(schedule)
        .bind(command)
        .bind(true)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn insert_recovery_rule(&self, rule_id: &str, condition: &str, action: &str) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO recovery_rules (rule_id, condition, action, priority, enabled)
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(rule_id)
        .bind(condition)
        .bind(action)
        .bind(100)
        .bind(true)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    fn get_category_from_key(key: &str) -> &'static str {
        if key.starts_with("memory.") || key.starts_with("vm.") {
            "memory"
        } else if key.starts_with("network.") || key.starts_with("net.") {
            "network"
        } else if key.starts_with("storage.") {
            "storage"
        } else if key.starts_with("security.") {
            "security"
        } else if key.starts_with("compliance.") {
            "compliance"
        } else if key.starts_with("backup.") {
            "backup"
        } else if key.starts_with("cluster.") {
            "cluster"
        } else if key.starts_with("overprovisioning.") {
            "limits"
        } else {
            "system"
        }
    }
}