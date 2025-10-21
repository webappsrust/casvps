use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use super::{BackupJob, BackupResult, BackupSnapshot, RestoreRequest, RestoreResult, StorageLimit};

/// Restic client for backup operations
pub struct ResticClient {
    repository_path: PathBuf,
    encryption_password: String,
    storage_limit: StorageLimit,
    repositories: HashMap<String, String>, // name -> path mapping
}

impl ResticClient {
    pub async fn new(base_path: &str, encryption_password: &str, storage_limit: StorageLimit) -> Result<Self> {
        let repository_path = PathBuf::from(base_path);
        std::fs::create_dir_all(&repository_path)?;

        let mut repositories = HashMap::new();
        repositories.insert("default".to_string(), repository_path.to_string_lossy().to_string());

        Ok(Self {
            repository_path,
            encryption_password: encryption_password.to_string(),
            storage_limit,
            repositories,
        })
    }

    /// Initialize Restic repository
    pub async fn init_repository(&self) -> Result<()> {
        // Check if repository is already initialized
        if self.is_repository_initialized("default").await? {
            info!("Restic repository already initialized");
            return Ok(());
        }

        info!("Initializing Restic repository");

        let output = self.run_restic_command(&[
            "init"
        ], Some("default")).await?;

        if !output.success {
            return Err(anyhow::anyhow!("Failed to initialize repository: {}", output.stderr));
        }

        info!("Restic repository initialized successfully");
        Ok(())
    }

    /// Check if repository is initialized
    pub async fn is_repository_initialized(&self, repo_name: &str) -> Result<bool> {
        let repo_path = self.repositories.get(repo_name)
            .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo_name))?;

        // Check for config file (indicates initialized repository)
        let config_path = Path::new(repo_path).join("config");
        Ok(config_path.exists())
    }

    /// Perform backup
    pub async fn backup(&self, job: &BackupJob) -> Result<BackupResult> {
        info!("Starting backup for job: {}", job.name);
        let start_time = std::time::Instant::now();

        // Build backup command
        let mut args = vec![
            "backup".to_string(),
            "--json".to_string(), // Get JSON output for parsing
            "--tag".to_string(), format!("job:{}", job.job_id),
            "--tag".to_string(), format!("type:{}", job.source_type),
        ];

        // Add compression if specified
        if job.compression != "none" {
            args.push("--compression".to_string());
            args.push(match job.compression.as_str() {
                "zstd" => "max".to_string(),
                "lz4" => "auto".to_string(),
                _ => "auto".to_string(),
            });
        }

        // Add source paths
        for source_id in &job.source_ids {
            let source_path = self.resolve_source_path(&job.source_type, source_id).await?;
            args.push(source_path);
        }

        // Run backup command
        let output = self.run_restic_command(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>(), Some(&job.destination)).await?;

        let duration = start_time.elapsed();

        if !output.success {
            return Ok(BackupResult {
                success: false,
                snapshot_id: None,
                files_backed_up: 0,
                bytes_backed_up: 0,
                duration,
                error: Some(output.stderr),
            });
        }

        // Parse JSON output to get backup statistics
        let backup_stats = self.parse_backup_output(&output.stdout)?;

        info!("Backup completed: {} files, {} bytes in {:?}",
              backup_stats.files_backed_up, backup_stats.bytes_backed_up, duration);

        Ok(BackupResult {
            success: true,
            snapshot_id: backup_stats.snapshot_id,
            files_backed_up: backup_stats.files_backed_up,
            bytes_backed_up: backup_stats.bytes_backed_up,
            duration,
            error: None,
        })
    }

    /// List snapshots
    pub async fn list_snapshots(&self, job_id: Option<&str>) -> Result<Vec<BackupSnapshot>> {
        let mut args = vec!["snapshots", "--json"];

        // Filter by job if specified
        if let Some(job_id) = job_id {
            args.push("--tag");
            args.push(&format!("job:{}", job_id));
        }

        let output = self.run_restic_command(&args, Some("default")).await?;

        if !output.success {
            return Err(anyhow::anyhow!("Failed to list snapshots: {}", output.stderr));
        }

        let snapshots: Vec<ResticSnapshot> = serde_json::from_str(&output.stdout)?;

        Ok(snapshots.into_iter().map(|s| BackupSnapshot {
            id: s.id,
            job_id: s.tags.iter()
                .find(|tag| tag.starts_with("job:"))
                .map(|tag| tag.strip_prefix("job:").unwrap().to_string()),
            timestamp: chrono::DateTime::parse_from_rfc3339(&s.time)?
                .with_timezone(&chrono::Utc),
            paths: s.paths,
            hostname: s.hostname,
            username: s.username.unwrap_or_default(),
            size: s.size.unwrap_or(0),
        }).collect())
    }

    /// Restore from backup
    pub async fn restore(&self, request: &RestoreRequest) -> Result<RestoreResult> {
        info!("Starting restore: snapshot {} to {}", request.snapshot_id, request.restore_path);
        let start_time = std::time::Instant::now();

        let mut args = vec![
            "restore".to_string(),
            request.snapshot_id.clone(),
            "--target".to_string(),
            request.restore_path.clone(),
            "--json".to_string(),
        ];

        // Add include patterns
        if let Some(includes) = &request.include_patterns {
            for pattern in includes {
                args.push("--include".to_string());
                args.push(pattern.clone());
            }
        }

        // Add exclude patterns
        if let Some(excludes) = &request.exclude_patterns {
            for pattern in excludes {
                args.push("--exclude".to_string());
                args.push(pattern.clone());
            }
        }

        // Overwrite existing files
        if request.overwrite {
            args.push("--overwrite".to_string());
        }

        let output = self.run_restic_command(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>(), Some("default")).await?;

        let duration = start_time.elapsed();

        if !output.success {
            return Ok(RestoreResult {
                success: false,
                files_restored: 0,
                bytes_restored: 0,
                duration,
                error: Some(output.stderr),
            });
        }

        // Parse restore output
        let restore_stats = self.parse_restore_output(&output.stdout)?;

        info!("Restore completed: {} files, {} bytes in {:?}",
              restore_stats.files_restored, restore_stats.bytes_restored, duration);

        Ok(RestoreResult {
            success: true,
            files_restored: restore_stats.files_restored,
            bytes_restored: restore_stats.bytes_restored,
            duration,
            error: None,
        })
    }

    /// Forget old snapshots based on retention policy
    pub async fn forget_old_snapshots(&self, retention_policy: &str, job_id: Option<&str>) -> Result<()> {
        info!("Cleaning up old snapshots with policy: {}", retention_policy);

        // Parse retention policy (format: "7d,4w,12m,5y")
        let retention = self.parse_retention_policy(retention_policy)?;

        let mut args = vec![
            "forget".to_string(),
            "--prune".to_string(), // Also run prune to free space
            "--json".to_string(),
        ];

        // Add retention options
        if let Some(daily) = retention.daily {
            args.push("--keep-daily".to_string());
            args.push(daily.to_string());
        }
        if let Some(weekly) = retention.weekly {
            args.push("--keep-weekly".to_string());
            args.push(weekly.to_string());
        }
        if let Some(monthly) = retention.monthly {
            args.push("--keep-monthly".to_string());
            args.push(monthly.to_string());
        }
        if let Some(yearly) = retention.yearly {
            args.push("--keep-yearly".to_string());
            args.push(yearly.to_string());
        }

        // Filter by job if specified
        if let Some(job_id) = job_id {
            args.push("--tag".to_string());
            args.push(format!("job:{}", job_id));
        }

        let output = self.run_restic_command(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>(), Some("default")).await?;

        if !output.success {
            warn!("Failed to cleanup snapshots: {}", output.stderr);
            return Err(anyhow::anyhow!("Cleanup failed: {}", output.stderr));
        }

        info!("Snapshot cleanup completed");
        Ok(())
    }

    /// Get repository statistics
    pub async fn get_repository_stats(&self) -> Result<RepositoryStats> {
        let output = self.run_restic_command(&["stats", "--json"], Some("default")).await?;

        if !output.success {
            return Err(anyhow::anyhow!("Failed to get repository stats: {}", output.stderr));
        }

        let stats: ResticStats = serde_json::from_str(&output.stdout)?;

        // Get latest snapshot timestamp
        let snapshots = self.list_snapshots(None).await?;
        let last_backup = snapshots.iter().map(|s| s.timestamp).max();

        Ok(RepositoryStats {
            total_size: stats.total_size,
            total_file_count: stats.total_file_count,
            deduplication_ratio: calculate_deduplication_ratio(&stats),
            last_backup,
        })
    }

    /// Check repository integrity
    pub async fn check_repository(&self) -> Result<bool> {
        info!("Checking repository integrity");

        let output = self.run_restic_command(&["check"], Some("default")).await?;

        if !output.success {
            warn!("Repository check failed: {}", output.stderr);
            return Ok(false);
        }

        info!("Repository integrity check passed");
        Ok(true)
    }

    async fn run_restic_command(&self, args: &[&str], repository: Option<&str>) -> Result<CommandOutput> {
        let repo_path = if let Some(repo) = repository {
            self.repositories.get(repo)
                .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo))?
        } else {
            &self.repository_path.to_string_lossy().to_string()
        };

        debug!("Running restic command: {:?}", args);

        let mut cmd = Command::new("restic");
        cmd.args(args)
            .env("RESTIC_REPOSITORY", repo_path)
            .env("RESTIC_PASSWORD", &self.encryption_password)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        debug!("Restic command output: success={}, stdout_len={}, stderr_len={}",
               output.status.success(), stdout.len(), stderr.len());

        Ok(CommandOutput {
            success: output.status.success(),
            stdout,
            stderr,
        })
    }

    async fn resolve_source_path(&self, source_type: &str, source_id: &str) -> Result<String> {
        match source_type {
            "vm" => {
                // VM disk images
                Ok(format!("/var/lib/casvps/instances/{}", source_id))
            },
            "container" => {
                // Container data
                Ok(format!("/var/lib/incus/containers/{}", source_id))
            },
            "directory" => {
                // Direct directory path
                Ok(source_id.to_string())
            },
            "database" => {
                // Database file
                Ok("/var/lib/casvps/casvps.db".to_string())
            },
            _ => Err(anyhow::anyhow!("Unknown source type: {}", source_type)),
        }
    }

    fn parse_backup_output(&self, output: &str) -> Result<BackupStats> {
        let lines: Vec<&str> = output.lines().collect();
        let summary_line = lines.iter()
            .find(|line| line.contains("\"message_type\":\"summary\""))
            .ok_or_else(|| anyhow::anyhow!("No summary found in backup output"))?;

        let summary: ResticBackupSummary = serde_json::from_str(summary_line)?;

        Ok(BackupStats {
            snapshot_id: Some(summary.snapshot_id),
            files_backed_up: summary.files_new + summary.files_changed,
            bytes_backed_up: summary.data_added,
        })
    }

    fn parse_restore_output(&self, output: &str) -> Result<RestoreStats> {
        // Restic restore doesn't provide JSON summary, so we'll parse text output
        // or estimate based on successful completion
        Ok(RestoreStats {
            files_restored: 0, // Would need to parse from text output
            bytes_restored: 0, // Would need to parse from text output
        })
    }

    fn parse_retention_policy(&self, policy: &str) -> Result<RetentionPolicy> {
        let parts: Vec<&str> = policy.split(',').collect();
        let mut retention = RetentionPolicy::default();

        for part in parts {
            let part = part.trim();
            if let Some(days) = part.strip_suffix('d') {
                retention.daily = Some(days.parse()?);
            } else if let Some(weeks) = part.strip_suffix('w') {
                retention.weekly = Some(weeks.parse()?);
            } else if let Some(months) = part.strip_suffix('m') {
                retention.monthly = Some(months.parse()?);
            } else if let Some(years) = part.strip_suffix('y') {
                retention.yearly = Some(years.parse()?);
            }
        }

        Ok(retention)
    }
}

#[derive(Debug)]
struct CommandOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

#[derive(Debug)]
struct BackupStats {
    snapshot_id: Option<String>,
    files_backed_up: u64,
    bytes_backed_up: u64,
}

#[derive(Debug)]
struct RestoreStats {
    files_restored: u64,
    bytes_restored: u64,
}

#[derive(Debug, Clone)]
pub struct RepositoryStats {
    pub total_size: u64,
    pub total_file_count: u64,
    pub deduplication_ratio: f64,
    pub last_backup: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Default)]
struct RetentionPolicy {
    daily: Option<u32>,
    weekly: Option<u32>,
    monthly: Option<u32>,
    yearly: Option<u32>,
}

// Restic JSON response structures
#[derive(Debug, Deserialize)]
struct ResticSnapshot {
    id: String,
    time: String,
    tree: String,
    paths: Vec<String>,
    hostname: String,
    username: Option<String>,
    uid: Option<u32>,
    gid: Option<u32>,
    tags: Vec<String>,
    #[serde(rename = "short_id")]
    short_id: String,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ResticStats {
    total_size: u64,
    total_file_count: u64,
    total_blob_count: u64,
    total_unique_size: u64,
}

#[derive(Debug, Deserialize)]
struct ResticBackupSummary {
    message_type: String,
    files_new: u64,
    files_changed: u64,
    files_unmodified: u64,
    dirs_new: u64,
    dirs_changed: u64,
    dirs_unmodified: u64,
    data_blobs: u64,
    tree_blobs: u64,
    data_added: u64,
    total_files_processed: u64,
    total_bytes_processed: u64,
    total_duration: f64,
    snapshot_id: String,
}

fn calculate_deduplication_ratio(stats: &ResticStats) -> f64 {
    if stats.total_size == 0 {
        return 1.0;
    }
    stats.total_unique_size as f64 / stats.total_size as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_restic_client_creation() {
        let temp_dir = TempDir::new().unwrap();
        let client = ResticClient::new(
            temp_dir.path().to_str().unwrap(),
            "test-password",
            StorageLimit::Percentage(10),
        ).await.unwrap();

        assert_eq!(client.repositories.get("default").unwrap(), temp_dir.path().to_str().unwrap());
    }

    #[test]
    fn test_retention_policy_parsing() {
        let client = ResticClient {
            repository_path: PathBuf::from("/tmp"),
            encryption_password: "test".to_string(),
            storage_limit: StorageLimit::Percentage(10),
            repositories: HashMap::new(),
        };

        let policy = client.parse_retention_policy("7d,4w,12m,5y").unwrap();
        assert_eq!(policy.daily, Some(7));
        assert_eq!(policy.weekly, Some(4));
        assert_eq!(policy.monthly, Some(12));
        assert_eq!(policy.yearly, Some(5));
    }

    #[test]
    fn test_deduplication_ratio() {
        let stats = ResticStats {
            total_size: 1000,
            total_file_count: 10,
            total_blob_count: 5,
            total_unique_size: 500,
        };

        let ratio = calculate_deduplication_ratio(&stats);
        assert_eq!(ratio, 0.5);
    }
}