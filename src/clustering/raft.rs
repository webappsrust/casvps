use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn, error};

#[derive(Debug, Clone, PartialEq)]
pub enum NodeRole {
    Leader,
    Follower,
    Candidate,
}

#[derive(Debug, Clone)]
pub struct RaftState {
    pub node_id: String,
    pub role: NodeRole,
    pub current_term: u64,
    pub voted_for: Option<String>,
    pub current_leader: Option<String>,
    pub log: Vec<LogEntry>,
    pub commit_index: u64,
    pub last_applied: u64,

    // Leader-specific state
    pub next_index: HashMap<String, u64>,
    pub match_index: HashMap<String, u64>,

    // Election timeout
    pub election_timeout_ms: u64,
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub term: u64,
    pub index: u64,
    pub entry_type: LogEntryType,
    pub data: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogEntryType {
    ConfigChange,
    NodeJoin,
    NodeLeave,
    SystemConfig,
    VMOperation,
    NetworkChange,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VoteRequest {
    pub term: u64,
    pub candidate_id: String,
    pub last_log_index: u64,
    pub last_log_term: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VoteResponse {
    pub term: u64,
    pub vote_granted: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppendEntriesRequest {
    pub term: u64,
    pub leader_id: String,
    pub prev_log_index: u64,
    pub prev_log_term: u64,
    pub entries: Vec<LogEntry>,
    pub leader_commit: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppendEntriesResponse {
    pub term: u64,
    pub success: bool,
    pub match_index: Option<u64>,
}

impl RaftState {
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
            role: NodeRole::Follower,
            current_term: 0,
            voted_for: None,
            current_leader: None,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            next_index: HashMap::new(),
            match_index: HashMap::new(),
            election_timeout_ms: Self::random_election_timeout(),
            last_heartbeat: chrono::Utc::now(),
        }
    }

    pub fn become_leader(&mut self, peer_ids: Vec<String>) {
        info!("Node {} becoming leader for term {}", self.node_id, self.current_term);

        self.role = NodeRole::Leader;
        self.current_leader = Some(self.node_id.clone());
        self.voted_for = None;

        // Initialize leader state
        let next_log_index = self.log.len() as u64 + 1;
        for peer_id in peer_ids {
            self.next_index.insert(peer_id.clone(), next_log_index);
            self.match_index.insert(peer_id, 0);
        }

        // Send initial heartbeat
        self.reset_heartbeat();
    }

    pub fn become_follower(&mut self, term: u64, leader_id: Option<String>) {
        if term > self.current_term {
            info!("Node {} becoming follower for term {} (was term {})",
                  self.node_id, term, self.current_term);

            self.current_term = term;
            self.voted_for = None;
        }

        self.role = NodeRole::Follower;
        self.current_leader = leader_id;
        self.reset_election_timeout();
    }

    pub fn become_candidate(&mut self) {
        info!("Node {} becoming candidate for term {}", self.node_id, self.current_term + 1);

        self.role = NodeRole::Candidate;
        self.current_term += 1;
        self.voted_for = Some(self.node_id.clone());
        self.current_leader = None;
        self.reset_election_timeout();
    }

    pub fn append_log_entry(&mut self, entry_type: LogEntryType, data: serde_json::Value) -> u64 {
        let index = self.log.len() as u64 + 1;
        let entry = LogEntry {
            term: self.current_term,
            index,
            entry_type,
            data,
            timestamp: chrono::Utc::now(),
        };

        self.log.push(entry);
        index
    }

    pub fn handle_vote_request(&mut self, request: VoteRequest) -> VoteResponse {
        let mut vote_granted = false;

        if request.term > self.current_term {
            self.become_follower(request.term, None);
        }

        if request.term == self.current_term {
            // Grant vote if we haven't voted or voted for this candidate
            if self.voted_for.is_none() || self.voted_for.as_ref() == Some(&request.candidate_id) {
                // Check if candidate's log is at least as up-to-date as ours
                let our_last_log_term = self.log.last().map(|e| e.term).unwrap_or(0);
                let our_last_log_index = self.log.len() as u64;

                if request.last_log_term > our_last_log_term ||
                   (request.last_log_term == our_last_log_term && request.last_log_index >= our_last_log_index) {
                    self.voted_for = Some(request.candidate_id.clone());
                    vote_granted = true;
                    self.reset_election_timeout();
                }
            }
        }

        VoteResponse {
            term: self.current_term,
            vote_granted,
        }
    }

    pub fn handle_append_entries(&mut self, request: AppendEntriesRequest) -> AppendEntriesResponse {
        if request.term > self.current_term {
            self.become_follower(request.term, Some(request.leader_id.clone()));
        }

        if request.term == self.current_term {
            self.become_follower(request.term, Some(request.leader_id.clone()));
            self.reset_heartbeat();

            // Check if we have the previous log entry
            if request.prev_log_index == 0 ||
               (request.prev_log_index <= self.log.len() as u64 &&
                self.log[(request.prev_log_index - 1) as usize].term == request.prev_log_term) {

                // Remove conflicting entries and append new ones
                if request.prev_log_index < self.log.len() as u64 {
                    self.log.truncate(request.prev_log_index as usize);
                }

                for entry in request.entries {
                    self.log.push(entry);
                }

                // Update commit index
                if request.leader_commit > self.commit_index {
                    self.commit_index = std::cmp::min(request.leader_commit, self.log.len() as u64);
                }

                return AppendEntriesResponse {
                    term: self.current_term,
                    success: true,
                    match_index: Some(self.log.len() as u64),
                };
            }
        }

        AppendEntriesResponse {
            term: self.current_term,
            success: false,
            match_index: None,
        }
    }

    pub fn is_election_timeout_expired(&self) -> bool {
        let elapsed = chrono::Utc::now()
            .signed_duration_since(self.last_heartbeat)
            .num_milliseconds() as u64;

        elapsed > self.election_timeout_ms
    }

    pub fn should_send_heartbeat(&self) -> bool {
        if !matches!(self.role, NodeRole::Leader) {
            return false;
        }

        let elapsed = chrono::Utc::now()
            .signed_duration_since(self.last_heartbeat)
            .num_milliseconds() as u64;

        elapsed > 50 // Send heartbeat every 50ms
    }

    pub fn reset_election_timeout(&mut self) {
        self.election_timeout_ms = Self::random_election_timeout();
        self.last_heartbeat = chrono::Utc::now();
    }

    pub fn reset_heartbeat(&mut self) {
        self.last_heartbeat = chrono::Utc::now();
    }

    fn random_election_timeout() -> u64 {
        // Random timeout between 150-300ms
        150 + (rand::random::<u64>() % 150)
    }

    pub fn apply_committed_entries(&mut self) -> Vec<LogEntry> {
        let mut applied_entries = Vec::new();

        while self.last_applied < self.commit_index {
            self.last_applied += 1;
            if let Some(entry) = self.log.get((self.last_applied - 1) as usize) {
                applied_entries.push(entry.clone());
            }
        }

        applied_entries
    }

    pub fn get_last_log_index(&self) -> u64 {
        self.log.len() as u64
    }

    pub fn get_last_log_term(&self) -> u64 {
        self.log.last().map(|e| e.term).unwrap_or(0)
    }
}

impl Default for RaftState {
    fn default() -> Self {
        Self::new("unknown".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raft_state_initialization() {
        let node_id = "test-node-1".to_string();
        let state = RaftState::new(node_id.clone());

        assert_eq!(state.node_id, node_id);
        assert_eq!(state.role, NodeRole::Follower);
        assert_eq!(state.current_term, 0);
        assert_eq!(state.voted_for, None);
        assert_eq!(state.current_leader, None);
        assert_eq!(state.log.len(), 0);
        assert_eq!(state.commit_index, 0);
        assert_eq!(state.last_applied, 0);
    }

    #[test]
    fn test_become_candidate() {
        let mut state = RaftState::new("test-node".to_string());
        let initial_term = state.current_term;

        state.become_candidate();

        assert_eq!(state.role, NodeRole::Candidate);
        assert_eq!(state.current_term, initial_term + 1);
        assert_eq!(state.voted_for, Some("test-node".to_string()));
        assert_eq!(state.current_leader, None);
    }

    #[test]
    fn test_append_log_entry() {
        let mut state = RaftState::new("test-node".to_string());
        state.current_term = 1;

        let data = serde_json::json!({"key": "value"});
        let index = state.append_log_entry(LogEntryType::SystemConfig, data.clone());

        assert_eq!(index, 1);
        assert_eq!(state.log.len(), 1);
        assert_eq!(state.log[0].term, 1);
        assert_eq!(state.log[0].index, 1);
        assert_eq!(state.log[0].data, data);
    }
}