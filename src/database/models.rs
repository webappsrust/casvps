use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SystemConfig {
    pub key: String,
    pub value: JsonValue,
    pub category: Option<String>,
    pub node_specific: bool,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub user_id: String,
    pub username: String,
    pub realm: String,
    pub email: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VM {
    pub vm_id: String,
    pub name: String,
    pub user_id: Option<String>,
    pub config: JsonValue,
    pub state: String,
    pub node_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Container {
    pub container_id: String,
    pub name: String,
    pub user_id: Option<String>,
    pub image: Option<String>,
    pub config: JsonValue,
    pub state: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ApiToken {
    pub token_hash: String,
    pub token_prefix: Option<String>,
    pub name: String,
    pub user_id: String,
    pub scopes: JsonValue,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ClusterNode {
    pub node_id: String,
    pub node_name: String,
    pub address: String,
    pub role: Option<String>,
    pub status: Option<String>,
    pub joined_at: DateTime<Utc>,
    pub last_heartbeat: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserNetwork {
    pub network_id: String,
    pub user_id: String,
    pub subnet: String,
    pub vlan_id: Option<i32>,
    pub domain: Option<String>,
    pub gateway: Option<String>,
    pub dns_servers: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IpAllocation {
    pub ip_address: String,
    pub subnet_id: Option<String>,
    pub allocation_type: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub hostname: Option<String>,
    pub mac_address: Option<String>,
    pub allocated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Certificate {
    pub id: String,
    pub domain: String,
    pub r#type: Option<String>,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub auto_renew: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecoveryRule {
    pub rule_id: String,
    pub condition: Option<String>,
    pub action: Option<String>,
    pub priority: Option<i32>,
    pub cooldown_seconds: i32,
    pub max_triggers_per_hour: i32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScheduledTask {
    pub task_id: String,
    pub name: Option<String>,
    pub schedule: Option<String>,
    pub command: Option<String>,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IsoLibraryEntry {
    pub id: String,
    pub distro_name: String,
    pub major_version: String,
    pub minor_version: Option<String>,
    pub architecture: Option<String>,
    pub filename: String,
    pub source_url: Option<String>,
    pub local_path: Option<String>,
    pub auto_update: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BackupJob {
    pub job_id: String,
    pub name: String,
    pub schedule: Option<String>,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub destination: Option<String>,
    pub retention_policy: Option<String>,
    pub compression: String,
    pub deduplication: bool,
    pub encryption_key: Option<String>,
    pub enabled: bool,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            key: String::new(),
            value: JsonValue::Null,
            category: None,
            node_specific: false,
            updated_at: Utc::now(),
            updated_by: None,
        }
    }
}

impl User {
    pub fn new(username: String, role: String) -> Self {
        Self {
            user_id: Uuid::new_v4().to_string(),
            username,
            realm: "local".to_string(),
            email: None,
            role,
            created_at: Utc::now(),
            last_login: None,
            enabled: true,
        }
    }

    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

impl VM {
    pub fn new(name: String, user_id: Option<String>) -> Self {
        Self {
            vm_id: Uuid::new_v4().to_string(),
            name,
            user_id,
            config: JsonValue::Object(serde_json::Map::new()),
            state: "stopped".to_string(),
            node_id: None,
            created_at: Utc::now(),
        }
    }
}

impl Container {
    pub fn new(name: String, user_id: Option<String>) -> Self {
        Self {
            container_id: Uuid::new_v4().to_string(),
            name,
            user_id,
            image: None,
            config: JsonValue::Object(serde_json::Map::new()),
            state: "stopped".to_string(),
            created_at: Utc::now(),
        }
    }
}