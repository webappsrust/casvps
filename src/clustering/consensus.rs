use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

use super::raft::{RaftState, LogEntry, LogEntryType};
use crate::database::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusOperation {
    pub operation_id: String,
    pub operation_type: OperationType,
    pub data: serde_json::Value,
    pub initiated_by: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    // System configuration changes
    SystemConfigUpdate { key: String, value: serde_json::Value },

    // Node management
    NodeJoin { node_id: String, node_info: serde_json::Value },
    NodeLeave { node_id: String, reason: String },
    NodeUpdate { node_id: String, updates: serde_json::Value },

    // VM operations that need consensus
    VMCreate { vm_id: String, config: serde_json::Value },
    VMDestroy { vm_id: String },
    VMMigrate { vm_id: String, from_node: String, to_node: String },
    VMResize { vm_id: String, new_config: serde_json::Value },

    // Network changes
    NetworkCreate { network_id: String, config: serde_json::Value },
    NetworkDestroy { network_id: String },
    NetworkUpdate { network_id: String, updates: serde_json::Value },

    // Storage operations
    StoragePoolCreate { pool_id: String, config: serde_json::Value },
    StoragePoolDestroy { pool_id: String },
    VolumeCreate { volume_id: String, config: serde_json::Value },
    VolumeDestroy { volume_id: String },

    // User management
    UserCreate { user_id: String, user_data: serde_json::Value },
    UserUpdate { user_id: String, updates: serde_json::Value },
    UserDelete { user_id: String },

    // Security and compliance
    SecurityPolicyUpdate { policy_type: String, policy_data: serde_json::Value },
    ComplianceConfigUpdate { compliance_type: String, config: serde_json::Value },
}

pub struct ConsensusManager {
    database: Arc<Database>,
    raft_state: Arc<RwLock<RaftState>>,
    pending_operations: Arc<RwLock<HashMap<String, ConsensusOperation>>>,
    applied_operations: Arc<RwLock<Vec<String>>>,
}

impl ConsensusManager {
    pub fn new(database: Arc<Database>, raft_state: Arc<RwLock<RaftState>>) -> Self {
        Self {
            database,
            raft_state,
            pending_operations: Arc::new(RwLock::new(HashMap::new())),
            applied_operations: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting consensus manager");

        // Start log application loop
        self.start_log_application_loop().await?;

        Ok(())
    }

    /// Propose a new operation to the cluster
    /// Only leaders can propose operations
    pub async fn propose_operation(&self, operation: ConsensusOperation) -> Result<String> {
        let raft_state = self.raft_state.read().await;
        if !matches!(raft_state.role, super::raft::NodeRole::Leader) {
            return Err(anyhow::anyhow!("Only leaders can propose operations"));
        }
        drop(raft_state);

        let operation_id = operation.operation_id.clone();
        info!("Proposing operation: {} ({:?})", operation_id, operation.operation_type);

        // Add to pending operations
        {
            let mut pending = self.pending_operations.write().await;
            pending.insert(operation_id.clone(), operation.clone());
        }

        // Create log entry
        let log_entry_type = match operation.operation_type {
            OperationType::NodeJoin { .. } => LogEntryType::NodeJoin,
            OperationType::NodeLeave { .. } => LogEntryType::NodeLeave,
            OperationType::SystemConfigUpdate { .. } => LogEntryType::SystemConfig,
            OperationType::VMCreate { .. } |
            OperationType::VMDestroy { .. } |
            OperationType::VMMigrate { .. } |
            OperationType::VMResize { .. } => LogEntryType::VMOperation,
            OperationType::NetworkCreate { .. } |
            OperationType::NetworkDestroy { .. } |
            OperationType::NetworkUpdate { .. } => LogEntryType::NetworkChange,
            _ => LogEntryType::ConfigChange,
        };

        // Append to Raft log
        let mut raft_state = self.raft_state.write().await;
        let log_index = raft_state.append_log_entry(
            log_entry_type,
            serde_json::to_value(operation)?
        );

        info!("Operation {} appended to log at index {}", operation_id, log_index);
        Ok(operation_id)
    }

    /// Wait for an operation to be committed and applied
    pub async fn wait_for_operation(&self, operation_id: &str) -> Result<()> {
        let timeout = tokio::time::Duration::from_secs(30);
        let start = tokio::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("Operation timeout"));
            }

            // Check if operation has been applied
            {
                let applied = self.applied_operations.read().await;
                if applied.contains(&operation_id.to_string()) {
                    return Ok(());
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    /// Apply a committed log entry
    pub async fn apply_log_entry(&self, entry: LogEntry) -> Result<()> {
        debug!("Applying log entry {} at index {}", entry.entry_type as u8, entry.index);

        let operation: ConsensusOperation = serde_json::from_value(entry.data)?;

        match &operation.operation_type {
            OperationType::SystemConfigUpdate { key, value } => {
                self.apply_system_config_update(key, value).await?;
            }
            OperationType::NodeJoin { node_id, node_info } => {
                self.apply_node_join(node_id, node_info).await?;
            }
            OperationType::NodeLeave { node_id, reason } => {
                self.apply_node_leave(node_id, reason).await?;
            }
            OperationType::VMCreate { vm_id, config } => {
                self.apply_vm_create(vm_id, config).await?;
            }
            OperationType::VMDestroy { vm_id } => {
                self.apply_vm_destroy(vm_id).await?;
            }
            OperationType::VMMigrate { vm_id, from_node, to_node } => {
                self.apply_vm_migrate(vm_id, from_node, to_node).await?;
            }
            OperationType::NetworkCreate { network_id, config } => {
                self.apply_network_create(network_id, config).await?;
            }
            OperationType::UserCreate { user_id, user_data } => {
                self.apply_user_create(user_id, user_data).await?;
            }
            _ => {
                warn!("Unhandled operation type: {:?}", operation.operation_type);
            }
        }

        // Remove from pending and add to applied
        {
            let mut pending = self.pending_operations.write().await;
            pending.remove(&operation.operation_id);
        }
        {
            let mut applied = self.applied_operations.write().await;
            applied.push(operation.operation_id);
        }

        info!("Applied operation: {}", operation.operation_id);
        Ok(())
    }

    async fn start_log_application_loop(&self) -> Result<()> {
        let raft_state = self.raft_state.clone();
        let consensus_manager = self.clone_for_task();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));

            loop {
                interval.tick().await;

                let applied_entries = {
                    let mut state = raft_state.write().await;
                    state.apply_committed_entries()
                };

                for entry in applied_entries {
                    if let Err(e) = consensus_manager.apply_log_entry(entry).await {
                        error!("Failed to apply log entry: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    fn clone_for_task(&self) -> ConsensusManager {
        ConsensusManager {
            database: self.database.clone(),
            raft_state: self.raft_state.clone(),
            pending_operations: self.pending_operations.clone(),
            applied_operations: self.applied_operations.clone(),
        }
    }

    // Individual operation handlers
    async fn apply_system_config_update(&self, key: &str, value: &serde_json::Value) -> Result<()> {
        info!("Applying system config update: {} = {}", key, value);

        sqlx::query!(
            "INSERT OR REPLACE INTO system_config (key, value, category, updated_at)
             VALUES (?, ?, ?, ?)",
            key,
            serde_json::to_string(value)?,
            "cluster",
            chrono::Utc::now()
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn apply_node_join(&self, node_id: &str, node_info: &serde_json::Value) -> Result<()> {
        info!("Applying node join: {}", node_id);

        let now = chrono::Utc::now();
        let node_name = node_info.get("node_name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let address = node_info.get("address")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        sqlx::query!(
            "INSERT INTO cluster_nodes
             (node_id, node_name, address, role, status, joined_at, last_heartbeat)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            node_id,
            node_name,
            address,
            "follower",
            "online",
            now,
            now
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn apply_node_leave(&self, node_id: &str, _reason: &str) -> Result<()> {
        info!("Applying node leave: {}", node_id);

        sqlx::query!(
            "UPDATE cluster_nodes SET status = 'offline' WHERE node_id = ?",
            node_id
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn apply_vm_create(&self, vm_id: &str, config: &serde_json::Value) -> Result<()> {
        info!("Applying VM create: {}", vm_id);

        let now = chrono::Utc::now();
        let name = config.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        let user_id = config.get("user_id").and_then(|v| v.as_str()).unwrap_or("system");

        sqlx::query!(
            "INSERT INTO vms (vm_id, name, user_id, config, state, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
            vm_id,
            name,
            user_id,
            serde_json::to_string(config)?,
            "stopped",
            now
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn apply_vm_destroy(&self, vm_id: &str) -> Result<()> {
        info!("Applying VM destroy: {}", vm_id);

        sqlx::query!(
            "DELETE FROM vms WHERE vm_id = ?",
            vm_id
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn apply_vm_migrate(&self, vm_id: &str, _from_node: &str, to_node: &str) -> Result<()> {
        info!("Applying VM migration: {} to {}", vm_id, to_node);

        sqlx::query!(
            "UPDATE vms SET node_id = ? WHERE vm_id = ?",
            to_node,
            vm_id
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn apply_network_create(&self, network_id: &str, config: &serde_json::Value) -> Result<()> {
        info!("Applying network create: {}", network_id);

        let name = config.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        let user_id = config.get("user_id").and_then(|v| v.as_str()).unwrap_or("system");
        let subnet = config.get("subnet").and_then(|v| v.as_str()).unwrap_or("192.168.0.0/24");
        let now = chrono::Utc::now();

        sqlx::query!(
            "INSERT INTO user_networks (network_id, user_id, subnet, created_at)
             VALUES (?, ?, ?, ?)",
            network_id,
            user_id,
            subnet,
            now
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn apply_user_create(&self, user_id: &str, user_data: &serde_json::Value) -> Result<()> {
        info!("Applying user create: {}", user_id);

        let username = user_data.get("username").and_then(|v| v.as_str()).unwrap_or("unknown");
        let email = user_data.get("email").and_then(|v| v.as_str());
        let role = user_data.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let now = chrono::Utc::now();

        sqlx::query!(
            "INSERT INTO users (user_id, username, email, role, created_at)
             VALUES (?, ?, ?, ?, ?)",
            user_id,
            username,
            email,
            role,
            now
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }
}