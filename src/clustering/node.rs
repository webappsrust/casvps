use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

use super::raft::{RaftState, VoteRequest, VoteResponse, AppendEntriesRequest, AppendEntriesResponse};
use crate::database::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: String,
    pub node_name: String,
    pub address: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub resources: NodeResources,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResources {
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub storage_gb: u64,
    pub network_interfaces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRequest {
    pub node_info: NodeInfo,
    pub join_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinResponse {
    pub success: bool,
    pub message: String,
    pub cluster_config: Option<ClusterConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub cluster_id: String,
    pub nodes: Vec<NodeInfo>,
    pub configuration: serde_json::Value,
}

pub struct ClusterNode {
    pub node_info: NodeInfo,
    database: Arc<Database>,
    raft_state: Arc<RwLock<RaftState>>,
    peers: Arc<RwLock<Vec<NodeInfo>>>,
    server_addr: SocketAddr,
}

impl ClusterNode {
    pub async fn new(
        node_info: NodeInfo,
        database: Arc<Database>,
        bind_addr: SocketAddr,
    ) -> Result<Self> {
        let raft_state = Arc::new(RwLock::new(RaftState::new(node_info.node_id.clone())));
        let peers = Arc::new(RwLock::new(Vec::new()));

        Ok(Self {
            node_info,
            database,
            raft_state,
            peers,
            server_addr: bind_addr,
        })
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting cluster node {} at {}", self.node_info.node_id, self.server_addr);

        // Start Raft consensus algorithm
        self.start_raft_loop().await?;

        // Start cluster communication server
        self.start_cluster_server().await?;

        Ok(())
    }

    pub async fn join_cluster(&self, leader_addr: &str, join_token: &str) -> Result<()> {
        info!("Attempting to join cluster via leader at {}", leader_addr);

        let join_request = JoinRequest {
            node_info: self.node_info.clone(),
            join_token: join_token.to_string(),
        };

        // Send join request to leader
        let client = reqwest::Client::new();
        let response = client
            .post(&format!("http://{}/cluster/join", leader_addr))
            .json(&join_request)
            .send()
            .await?;

        let join_response: JoinResponse = response.json().await?;

        if join_response.success {
            info!("Successfully joined cluster");

            if let Some(cluster_config) = join_response.cluster_config {
                // Update peer list
                let mut peers = self.peers.write().await;
                *peers = cluster_config.nodes;

                // Update Raft state to follower
                let mut raft_state = self.raft_state.write().await;
                raft_state.become_follower(0, None);
            }
        } else {
            error!("Failed to join cluster: {}", join_response.message);
            return Err(anyhow::anyhow!("Join failed: {}", join_response.message));
        }

        Ok(())
    }

    pub async fn handle_join_request(&self, request: JoinRequest) -> Result<JoinResponse> {
        info!("Received join request from node {}", request.node_info.node_id);

        // Verify join token
        let token_valid = self.verify_join_token(&request.join_token).await?;
        if !token_valid {
            return Ok(JoinResponse {
                success: false,
                message: "Invalid or expired join token".to_string(),
                cluster_config: None,
            });
        }

        // Check if we're the leader
        let raft_state = self.raft_state.read().await;
        if !matches!(raft_state.role, super::raft::NodeRole::Leader) {
            return Ok(JoinResponse {
                success: false,
                message: "Not the cluster leader".to_string(),
                cluster_config: None,
            });
        }

        // Add node to cluster
        self.add_node_to_cluster(request.node_info.clone()).await?;

        // Return cluster configuration
        let cluster_config = self.get_cluster_config().await?;

        Ok(JoinResponse {
            success: true,
            message: "Successfully joined cluster".to_string(),
            cluster_config: Some(cluster_config),
        })
    }

    pub async fn handle_vote_request(&self, request: VoteRequest) -> VoteResponse {
        debug!("Received vote request from {} for term {}", request.candidate_id, request.term);

        let mut raft_state = self.raft_state.write().await;
        raft_state.handle_vote_request(request)
    }

    pub async fn handle_append_entries(&self, request: AppendEntriesRequest) -> AppendEntriesResponse {
        debug!("Received append entries from {} for term {}", request.leader_id, request.term);

        let mut raft_state = self.raft_state.write().await;
        raft_state.handle_append_entries(request)
    }

    pub async fn send_vote_requests(&self) -> Result<bool> {
        let raft_state = self.raft_state.read().await;
        let peers = self.peers.read().await;

        if peers.is_empty() {
            // Single node cluster, become leader immediately
            return Ok(true);
        }

        let vote_request = VoteRequest {
            term: raft_state.current_term,
            candidate_id: raft_state.node_id.clone(),
            last_log_index: raft_state.get_last_log_index(),
            last_log_term: raft_state.get_last_log_term(),
        };

        drop(raft_state);

        let mut votes_received = 1; // Vote for ourselves
        let required_votes = (peers.len() + 1) / 2 + 1; // Majority

        // Send vote requests to all peers
        let client = reqwest::Client::new();
        let mut tasks = Vec::new();

        for peer in peers.iter() {
            let client = client.clone();
            let vote_request = vote_request.clone();
            let peer_addr = peer.address.clone();

            let task = tokio::spawn(async move {
                let response = client
                    .post(&format!("http://{}/cluster/vote", peer_addr))
                    .json(&vote_request)
                    .timeout(std::time::Duration::from_millis(100))
                    .send()
                    .await;

                match response {
                    Ok(resp) => {
                        if let Ok(vote_response) = resp.json::<VoteResponse>().await {
                            if vote_response.vote_granted {
                                return Some(vote_response);
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Failed to get vote from {}: {}", peer_addr, e);
                    }
                }
                None
            });

            tasks.push(task);
        }

        // Wait for responses
        for task in tasks {
            if let Ok(Some(_)) = task.await {
                votes_received += 1;
                if votes_received >= required_votes {
                    break;
                }
            }
        }

        Ok(votes_received >= required_votes)
    }

    pub async fn send_heartbeats(&self) -> Result<()> {
        let raft_state = self.raft_state.read().await;
        let peers = self.peers.read().await;

        if !matches!(raft_state.role, super::raft::NodeRole::Leader) {
            return Ok(());
        }

        let append_entries = AppendEntriesRequest {
            term: raft_state.current_term,
            leader_id: raft_state.node_id.clone(),
            prev_log_index: raft_state.get_last_log_index(),
            prev_log_term: raft_state.get_last_log_term(),
            entries: Vec::new(), // Empty for heartbeat
            leader_commit: raft_state.commit_index,
        };

        drop(raft_state);

        let client = reqwest::Client::new();
        let mut tasks = Vec::new();

        for peer in peers.iter() {
            let client = client.clone();
            let append_entries = append_entries.clone();
            let peer_addr = peer.address.clone();

            let task = tokio::spawn(async move {
                let _response = client
                    .post(&format!("http://{}/cluster/append", peer_addr))
                    .json(&append_entries)
                    .timeout(std::time::Duration::from_millis(50))
                    .send()
                    .await;
            });

            tasks.push(task);
        }

        // Wait for all heartbeats to complete
        for task in tasks {
            let _ = task.await;
        }

        Ok(())
    }

    async fn start_raft_loop(&self) -> Result<()> {
        let raft_state = self.raft_state.clone();
        let node_id = self.node_info.node_id.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(10));

            loop {
                interval.tick().await;

                let mut state = raft_state.write().await;

                match state.role {
                    super::raft::NodeRole::Follower => {
                        if state.is_election_timeout_expired() {
                            info!("Election timeout expired for follower {}", node_id);
                            state.become_candidate();
                        }
                    }
                    super::raft::NodeRole::Candidate => {
                        if state.is_election_timeout_expired() {
                            info!("Election timeout expired for candidate {}", node_id);
                            state.become_candidate(); // Start new election
                        }
                    }
                    super::raft::NodeRole::Leader => {
                        if state.should_send_heartbeat() {
                            state.reset_heartbeat();
                            // Heartbeats will be sent by the main loop
                        }
                    }
                }
            }
        });

        Ok(())
    }

    async fn start_cluster_server(&self) -> Result<()> {
        // This would start an HTTP server for cluster communication
        // For now, just log that it would start
        info!("Cluster communication server would start on {}", self.server_addr);
        Ok(())
    }

    async fn verify_join_token(&self, token: &str) -> Result<bool> {
        let now = chrono::Utc::now();

        let record = sqlx::query!(
            "SELECT expires_at FROM join_tokens WHERE token = ? AND expires_at > ?",
            token,
            now
        )
        .fetch_optional(&self.database.pool)
        .await?;

        Ok(record.is_some())
    }

    async fn add_node_to_cluster(&self, node_info: NodeInfo) -> Result<()> {
        let now = chrono::Utc::now();

        sqlx::query!(
            "INSERT INTO cluster_nodes
             (node_id, node_name, address, role, status, joined_at, last_heartbeat)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            node_info.node_id,
            node_info.node_name,
            node_info.address,
            "follower",
            "online",
            now,
            now
        )
        .execute(&self.database.pool)
        .await?;

        // Add to peer list
        let mut peers = self.peers.write().await;
        peers.push(node_info);

        Ok(())
    }

    async fn get_cluster_config(&self) -> Result<ClusterConfig> {
        let peers = self.peers.read().await;
        let mut all_nodes = peers.clone();
        all_nodes.push(self.node_info.clone());

        Ok(ClusterConfig {
            cluster_id: "casvps-cluster".to_string(),
            nodes: all_nodes,
            configuration: serde_json::json!({
                "version": "1.0.0",
                "features": ["ha", "migration", "shared-storage"]
            }),
        })
    }

    pub async fn get_node_resources() -> NodeResources {
        let sys = sysinfo::System::new_all();

        NodeResources {
            cpu_cores: num_cpus::get() as u32,
            memory_mb: sys.total_memory() / 1024 / 1024,
            storage_gb: 100, // TODO: Calculate actual storage
            network_interfaces: vec!["eth0".to_string()], // TODO: Get actual interfaces
        }
    }
}