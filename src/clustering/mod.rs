pub mod raft;
pub mod node;
pub mod consensus;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};
use uuid::Uuid;
use crate::database::Database;

#[derive(Debug, Clone)]
pub enum NodeRole {
    Leader,
    Follower,
    Candidate,
}

#[derive(Debug, Clone)]
pub struct ClusterNode {
    pub node_id: String,
    pub node_name: String,
    pub address: String,
    pub role: NodeRole,
    pub status: NodeStatus,
    pub joined_at: chrono::DateTime<chrono::Utc>,
    pub last_heartbeat: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub enum NodeStatus {
    Online,
    Offline,
    Maintenance,
}

pub struct ClusterManager {
    database: Arc<Database>,
    node_id: String,
    node_name: String,
    address: String,
    nodes: Arc<RwLock<Vec<ClusterNode>>>,
    raft_state: Arc<RwLock<raft::RaftState>>,
}

impl ClusterManager {
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        let node_id = Self::get_or_create_node_id(&database).await?;
        let node_name = format!("casvps-{}", &node_id[..8]);
        let address = Self::detect_node_address().await?;

        let nodes = Arc::new(RwLock::new(Vec::new()));
        let raft_state = Arc::new(RwLock::new(raft::RaftState::new(node_id.clone())));

        Ok(Self {
            database,
            node_id,
            node_name,
            address,
            nodes,
            raft_state,
        })
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting cluster manager for node {}", self.node_id);

        // Register this node in the database
        self.register_node().await?;

        // Initialize Raft state
        self.initialize_raft().await?;

        // Start heartbeat process
        self.start_heartbeat().await?;

        // Load existing cluster nodes
        self.load_cluster_nodes().await?;

        info!("Cluster manager started successfully");
        Ok(())
    }

    pub async fn join_cluster(&self, leader_address: &str, join_token: &str) -> Result<()> {
        info!("Attempting to join cluster at {} with token", leader_address);

        // Validate join token format (node_{59_random_characters})
        if !join_token.starts_with("node_") || join_token.len() != 64 {
            return Err(anyhow::anyhow!("Invalid join token format"));
        }

        // Send join request to leader
        let join_request = serde_json::json!({
            "node_id": self.node_id,
            "node_name": self.node_name,
            "address": self.address,
            "token": join_token
        });

        // For now, just log the join attempt
        // In full implementation, this would make an HTTP request
        info!("Would send join request: {}", join_request);

        // Update node role to follower
        let mut raft_state = self.raft_state.write().await;
        raft_state.role = raft::NodeRole::Follower;

        Ok(())
    }

    pub async fn generate_join_token(&self) -> Result<String> {
        // Only leaders can generate join tokens
        let raft_state = self.raft_state.read().await;
        if !matches!(raft_state.role, raft::NodeRole::Leader) {
            return Err(anyhow::anyhow!("Only cluster leaders can generate join tokens"));
        }

        // Generate token: node_{59_random_characters}
        let random_part: String = (0..59)
            .map(|_| {
                let chars = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
                chars[rand::random::<usize>() % chars.len()] as char
            })
            .collect();

        let token = format!("node_{}", random_part);

        // Store token in database with expiry (24 hours)
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);
        sqlx::query!(
            "INSERT INTO join_tokens (token, generated_by, expires_at) VALUES (?, ?, ?)",
            token,
            self.node_id,
            expires_at
        )
        .execute(&self.database.pool)
        .await?;

        info!("Generated join token: {}", &token[..16]);
        Ok(token)
    }

    pub async fn remove_node(&self, node_id: &str) -> Result<()> {
        info!("Removing node {} from cluster", node_id);

        // Remove from database
        sqlx::query!(
            "DELETE FROM cluster_nodes WHERE node_id = ?",
            node_id
        )
        .execute(&self.database.pool)
        .await?;

        // Remove from in-memory list
        let mut nodes = self.nodes.write().await;
        nodes.retain(|node| node.node_id != node_id);

        Ok(())
    }

    pub async fn get_cluster_nodes(&self) -> Result<Vec<ClusterNode>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.clone())
    }

    pub async fn is_leader(&self) -> bool {
        let raft_state = self.raft_state.read().await;
        matches!(raft_state.role, raft::NodeRole::Leader)
    }

    pub async fn get_leader(&self) -> Option<String> {
        let raft_state = self.raft_state.read().await;
        raft_state.current_leader.clone()
    }

    async fn get_or_create_node_id(database: &Database) -> Result<String> {
        // Try to get existing node ID
        if let Ok(record) = sqlx::query!(
            "SELECT value FROM system_config WHERE key = 'cluster.node_id'"
        )
        .fetch_one(&database.pool)
        .await
        {
            if let Ok(node_id) = serde_json::from_str::<String>(&record.value) {
                return Ok(node_id);
            }
        }

        // Generate new node ID
        let node_id = Uuid::new_v4().to_string();

        // Store in database
        sqlx::query!(
            "INSERT OR REPLACE INTO system_config (key, value, category) VALUES (?, ?, ?)",
            "cluster.node_id",
            serde_json::to_string(&node_id)?,
            "cluster"
        )
        .execute(&database.pool)
        .await?;

        Ok(node_id)
    }

    async fn detect_node_address(&self) -> Result<String> {
        // Try to detect the best IP address for cluster communication
        if let Some(lan_ip) = self.detect_lan_ip() {
            return Ok(format!("{}:7946", lan_ip)); // Use port 7946 for cluster communication
        }

        Ok("127.0.0.1:7946".to_string())
    }

    fn detect_lan_ip(&self) -> Option<String> {
        use pnet::datalink;

        let interfaces = datalink::interfaces();
        for interface in interfaces {
            if !interface.is_loopback() && interface.is_up() {
                for ip in &interface.ips {
                    if let pnet::ipnetwork::IpNetwork::V4(ipv4) = ip {
                        let ip_addr = ipv4.ip();
                        if !ip_addr.is_loopback() &&
                           !ip_addr.is_multicast() &&
                           !ip_addr.is_broadcast() {
                            return Some(ip_addr.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    async fn register_node(&self) -> Result<()> {
        let now = chrono::Utc::now();

        sqlx::query!(
            "INSERT OR REPLACE INTO cluster_nodes
             (node_id, node_name, address, role, status, joined_at, last_heartbeat)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            self.node_id,
            self.node_name,
            self.address,
            "follower", // Start as follower, election will determine leader
            "online",
            now,
            now
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn initialize_raft(&self) -> Result<()> {
        let mut raft_state = self.raft_state.write().await;

        // Check if this is the first node in the cluster
        let node_count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM cluster_nodes WHERE status = 'online'"
        )
        .fetch_one(&self.database.pool)
        .await?;

        if node_count == 1 {
            // This is the first node, become leader immediately
            raft_state.role = raft::NodeRole::Leader;
            raft_state.current_leader = Some(self.node_id.clone());
            raft_state.current_term += 1;

            info!("First node in cluster, becoming leader");

            // Update database
            sqlx::query!(
                "UPDATE cluster_nodes SET role = 'leader' WHERE node_id = ?",
                self.node_id
            )
            .execute(&self.database.pool)
            .await?;
        }

        Ok(())
    }

    async fn start_heartbeat(&self) -> Result<()> {
        let database = self.database.clone();
        let node_id = self.node_id.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

            loop {
                interval.tick().await;

                let now = chrono::Utc::now();
                if let Err(e) = sqlx::query!(
                    "UPDATE cluster_nodes SET last_heartbeat = ? WHERE node_id = ?",
                    now,
                    node_id
                )
                .execute(&database.pool)
                .await
                {
                    error!("Failed to update heartbeat: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn load_cluster_nodes(&self) -> Result<()> {
        let records = sqlx::query!(
            "SELECT node_id, node_name, address, role, status, joined_at, last_heartbeat
             FROM cluster_nodes WHERE status != 'offline'"
        )
        .fetch_all(&self.database.pool)
        .await?;

        let mut nodes = self.nodes.write().await;
        nodes.clear();

        for record in records {
            let role = match record.role.as_str() {
                "leader" => NodeRole::Leader,
                "candidate" => NodeRole::Candidate,
                _ => NodeRole::Follower,
            };

            let status = match record.status.as_str() {
                "maintenance" => NodeStatus::Maintenance,
                "offline" => NodeStatus::Offline,
                _ => NodeStatus::Online,
            };

            nodes.push(ClusterNode {
                node_id: record.node_id,
                node_name: record.node_name,
                address: record.address,
                role,
                status,
                joined_at: record.joined_at,
                last_heartbeat: record.last_heartbeat,
            });
        }

        info!("Loaded {} cluster nodes", nodes.len());
        Ok(())
    }
}