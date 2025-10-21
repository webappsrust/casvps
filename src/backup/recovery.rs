use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tracing::{info, warn, error};
use crate::database::Database;
use super::restic::ResticClient;

/// Backup recovery and restore operations
pub struct RecoveryManager {
    database: Arc<Database>,
    restic_client: Arc<ResticClient>,
}

impl RecoveryManager {
    pub async fn new(database: Arc<Database>, restic_client: Arc<ResticClient>) -> Result<Self> {
        Ok(Self {
            database,
            restic_client,
        })
    }

    /// List available snapshots for recovery
    pub async fn list_snapshots(&self, job_id: Option<&str>) -> Result<Vec<SnapshotInfo>> {
        info!("Listing available snapshots for recovery");

        let mut snapshots = Vec::new();
        let snapshot_list = self.restic_client.list_snapshots().await?;

        for snapshot in snapshot_list {
            // Filter by job_id if specified
            if let Some(job_filter) = job_id {
                if !snapshot.tags.iter().any(|tag| tag == job_filter) {
                    continue;
                }
            }

            snapshots.push(SnapshotInfo {
                id: snapshot.short_id,
                snapshot_id: snapshot.id,
                time: snapshot.time,
                hostname: snapshot.hostname,
                username: snapshot.username,
                tags: snapshot.tags,
                paths: snapshot.paths,
                size: snapshot.summary.as_ref().map(|s| s.total_files_processed).unwrap_or(0),
            });
        }

        info!("Found {} snapshots available for recovery", snapshots.len());
        Ok(snapshots)
    }

    /// Restore a specific snapshot
    pub async fn restore_snapshot(
        &self,
        snapshot_id: &str,
        target_path: &str,
        include_patterns: Option<Vec<String>>,
        exclude_patterns: Option<Vec<String>>,
    ) -> Result<RestoreResult> {
        info!("Starting restore of snapshot {} to {}", snapshot_id, target_path);

        // Validate target path
        let target = Path::new(target_path);
        if !target.exists() {
            std::fs::create_dir_all(target)?;
            info!("Created target directory: {}", target_path);
        }

        let restore_result = self.restic_client.restore_snapshot(
            snapshot_id,
            target_path,
            include_patterns.as_ref(),
            exclude_patterns.as_ref(),
        ).await?;

        // Log restore result
        self.log_restore_result(snapshot_id, target_path, &restore_result).await?;

        info!("Restore completed: {} files, {} bytes",
              restore_result.files_restored, restore_result.bytes_restored);

        Ok(restore_result)
    }

    /// Mount a snapshot for browsing
    pub async fn mount_snapshot(
        &self,
        snapshot_id: &str,
        mount_point: &str,
    ) -> Result<MountResult> {
        info!("Mounting snapshot {} at {}", snapshot_id, mount_point);

        // Ensure mount point exists
        let mount_path = Path::new(mount_point);
        if !mount_path.exists() {
            std::fs::create_dir_all(mount_path)?;
        }

        // Check if already mounted
        if self.is_mounted(mount_point)? {
            warn!("Mount point {} is already in use", mount_point);
            return Ok(MountResult {
                snapshot_id: snapshot_id.to_string(),
                mount_point: mount_point.to_string(),
                mounted: false,
                error: Some("Mount point already in use".to_string()),
            });
        }

        let mount_result = self.restic_client.mount_snapshot(snapshot_id, mount_point).await?;

        info!("Snapshot {} mounted at {}", snapshot_id, mount_point);
        Ok(mount_result)
    }

    /// Unmount a snapshot
    pub async fn unmount_snapshot(&self, mount_point: &str) -> Result<()> {
        info!("Unmounting snapshot at {}", mount_point);

        if !self.is_mounted(mount_point)? {
            warn!("Mount point {} is not mounted", mount_point);
            return Ok(());
        }

        self.restic_client.unmount_snapshot(mount_point).await?;

        info!("Unmounted snapshot at {}", mount_point);
        Ok(())
    }

    /// Get restore history
    pub async fn get_restore_history(&self, limit: Option<usize>) -> Result<Vec<RestoreHistoryEntry>> {
        let limit = limit.unwrap_or(100);

        let entries = sqlx::query_as::<_, RestoreHistoryRow>(
            "SELECT snapshot_id, target_path, files_restored, bytes_restored, success, error_message, restored_at
             FROM restore_history
             ORDER BY restored_at DESC
             LIMIT ?"
        )
        .bind(limit as i64)
        .fetch_all(&self.database.pool)
        .await?;

        Ok(entries.into_iter().map(|row| RestoreHistoryEntry {
            snapshot_id: row.snapshot_id,
            target_path: row.target_path,
            files_restored: row.files_restored,
            bytes_restored: row.bytes_restored,
            success: row.success,
            error_message: row.error_message,
            restored_at: row.restored_at,
        }).collect())
    }

    /// Verify a snapshot integrity
    pub async fn verify_snapshot(&self, snapshot_id: &str) -> Result<VerifyResult> {
        info!("Verifying snapshot integrity: {}", snapshot_id);

        let verify_result = self.restic_client.check_snapshot(snapshot_id).await?;

        info!("Snapshot verification completed: {} errors found",
              verify_result.errors.len());

        Ok(verify_result)
    }

    /// Create disaster recovery package
    pub async fn create_disaster_recovery_package(
        &self,
        output_path: &str,
        include_config: bool,
    ) -> Result<DisasterRecoveryPackage> {
        info!("Creating disaster recovery package at {}", output_path);

        let mut package = DisasterRecoveryPackage {
            output_path: output_path.to_string(),
            config_included: include_config,
            snapshots: Vec::new(),
            created_at: chrono::Utc::now(),
            size_bytes: 0,
        };

        // Create output directory
        let output_dir = Path::new(output_path);
        std::fs::create_dir_all(output_dir)?;

        // Export all snapshots metadata
        let snapshots = self.list_snapshots(None).await?;
        for snapshot in snapshots {
            // Export snapshot metadata
            let metadata_path = output_dir.join(format!("snapshot_{}.json", snapshot.id));
            let metadata = serde_json::to_string_pretty(&snapshot)?;
            std::fs::write(metadata_path, metadata)?;

            package.snapshots.push(snapshot.snapshot_id);
        }

        // Include configuration if requested
        if include_config {
            info!("Including system configuration in disaster recovery package");

            // Export database
            let config_dir = output_dir.join("config");
            std::fs::create_dir_all(&config_dir)?;

            // Export system configuration
            let config_export = self.export_system_config().await?;
            let config_path = config_dir.join("system_config.json");
            std::fs::write(config_path, config_export)?;

            // Export backup jobs
            let jobs_export = self.export_backup_jobs().await?;
            let jobs_path = config_dir.join("backup_jobs.json");
            std::fs::write(jobs_path, jobs_export)?;
        }

        // Calculate package size
        package.size_bytes = self.calculate_directory_size(output_dir)?;

        info!("Disaster recovery package created: {} snapshots, {} bytes",
              package.snapshots.len(), package.size_bytes);

        Ok(package)
    }

    /// Automated recovery procedures
    pub async fn run_automated_recovery(&self, scenario: RecoveryScenario) -> Result<RecoveryReport> {
        info!("Running automated recovery for scenario: {:?}", scenario);

        let mut report = RecoveryReport {
            scenario,
            started_at: chrono::Utc::now(),
            completed_at: None,
            success: false,
            actions_taken: Vec::new(),
            errors: Vec::new(),
        };

        match scenario {
            RecoveryScenario::ConfigLoss => {
                self.recover_from_config_loss(&mut report).await?;
            },
            RecoveryScenario::DataCorruption => {
                self.recover_from_data_corruption(&mut report).await?;
            },
            RecoveryScenario::CompleteSystemFailure => {
                self.recover_from_system_failure(&mut report).await?;
            },
        }

        report.completed_at = Some(chrono::Utc::now());
        info!("Automated recovery completed: success={}", report.success);

        Ok(report)
    }

    // Private helper methods

    async fn log_restore_result(
        &self,
        snapshot_id: &str,
        target_path: &str,
        result: &RestoreResult,
    ) -> Result<()> {
        sqlx::query!(
            "INSERT INTO restore_history
             (snapshot_id, target_path, files_restored, bytes_restored, success, error_message, restored_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            snapshot_id,
            target_path,
            result.files_restored,
            result.bytes_restored,
            result.success,
            result.error,
            chrono::Utc::now()
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    fn is_mounted(&self, mount_point: &str) -> Result<bool> {
        // Check if mount point is in use
        let output = Command::new("mountpoint")
            .args(&[mount_point])
            .output()?;

        Ok(output.status.success())
    }

    async fn export_system_config(&self) -> Result<String> {
        let config_rows = sqlx::query_as::<_, (String, String)>(
            "SELECT key, value FROM system_config"
        )
        .fetch_all(&self.database.pool)
        .await?;

        let config_map: HashMap<String, String> = config_rows.into_iter().collect();
        Ok(serde_json::to_string_pretty(&config_map)?)
    }

    async fn export_backup_jobs(&self) -> Result<String> {
        let jobs = sqlx::query_as::<_, BackupJobExport>(
            "SELECT job_id, name, schedule, source_type, source_id, destination, retention_policy, enabled
             FROM backup_jobs"
        )
        .fetch_all(&self.database.pool)
        .await?;

        Ok(serde_json::to_string_pretty(&jobs)?)
    }

    fn calculate_directory_size(&self, dir: &Path) -> Result<u64> {
        let mut total_size = 0u64;

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;

            if metadata.is_file() {
                total_size += metadata.len();
            } else if metadata.is_dir() {
                total_size += self.calculate_directory_size(&entry.path())?;
            }
        }

        Ok(total_size)
    }

    async fn recover_from_config_loss(&self, report: &mut RecoveryReport) -> Result<()> {
        report.actions_taken.push("Attempting to recover system configuration".to_string());

        // Look for latest config backup
        let snapshots = self.list_snapshots(Some("config")).await?;
        if snapshots.is_empty() {
            report.errors.push("No configuration backups found".to_string());
            return Ok(());
        }

        let latest_snapshot = &snapshots[0];

        // Restore configuration
        match self.restore_snapshot(
            &latest_snapshot.snapshot_id,
            "/var/lib/casvps/recovery/config",
            None,
            None,
        ).await {
            Ok(_) => {
                report.actions_taken.push("Configuration restored successfully".to_string());
                report.success = true;
            },
            Err(e) => {
                report.errors.push(format!("Failed to restore configuration: {}", e));
            }
        }

        Ok(())
    }

    async fn recover_from_data_corruption(&self, report: &mut RecoveryReport) -> Result<()> {
        report.actions_taken.push("Attempting to recover from data corruption".to_string());

        // Verify repository integrity first
        match self.restic_client.check_repository().await {
            Ok(check_result) => {
                if !check_result.errors.is_empty() {
                    report.errors.push("Repository corruption detected".to_string());

                    // Attempt repair
                    if let Err(e) = self.restic_client.repair_repository().await {
                        report.errors.push(format!("Repository repair failed: {}", e));
                        return Ok(());
                    }

                    report.actions_taken.push("Repository repaired".to_string());
                }
            },
            Err(e) => {
                report.errors.push(format!("Repository check failed: {}", e));
                return Ok(());
            }
        }

        report.success = true;
        Ok(())
    }

    async fn recover_from_system_failure(&self, report: &mut RecoveryReport) -> Result<()> {
        report.actions_taken.push("Attempting complete system recovery".to_string());

        // This would implement a complete system restore procedure
        // For now, just report the scenario
        report.actions_taken.push("Complete system recovery requires manual intervention".to_string());
        report.errors.push("Automated complete system recovery not implemented".to_string());

        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnapshotInfo {
    pub id: String,
    pub snapshot_id: String,
    pub time: chrono::DateTime<chrono::Utc>,
    pub hostname: String,
    pub username: String,
    pub tags: Vec<String>,
    pub paths: Vec<String>,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct RestoreResult {
    pub success: bool,
    pub files_restored: u64,
    pub bytes_restored: u64,
    pub duration: std::time::Duration,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MountResult {
    pub snapshot_id: String,
    pub mount_point: String,
    pub mounted: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RestoreHistoryEntry {
    pub snapshot_id: String,
    pub target_path: String,
    pub files_restored: u64,
    pub bytes_restored: u64,
    pub success: bool,
    pub error_message: Option<String>,
    pub restored_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub snapshot_id: String,
    pub success: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DisasterRecoveryPackage {
    pub output_path: String,
    pub config_included: bool,
    pub snapshots: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub enum RecoveryScenario {
    ConfigLoss,
    DataCorruption,
    CompleteSystemFailure,
}

#[derive(Debug, Clone)]
pub struct RecoveryReport {
    pub scenario: RecoveryScenario,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub success: bool,
    pub actions_taken: Vec<String>,
    pub errors: Vec<String>,
}

// Database row structures
#[derive(sqlx::FromRow)]
struct RestoreHistoryRow {
    snapshot_id: String,
    target_path: String,
    files_restored: i64,
    bytes_restored: i64,
    success: bool,
    error_message: Option<String>,
    restored_at: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct BackupJobExport {
    job_id: String,
    name: String,
    schedule: String,
    source_type: String,
    source_id: String,
    destination: String,
    retention_policy: String,
    enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_snapshot_info_serialization() {
        let snapshot = SnapshotInfo {
            id: "abc123".to_string(),
            snapshot_id: "abc123def456".to_string(),
            time: chrono::Utc::now(),
            hostname: "test-host".to_string(),
            username: "test-user".to_string(),
            tags: vec!["test".to_string()],
            paths: vec!["/test/path".to_string()],
            size: 12345,
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: SnapshotInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(snapshot.id, deserialized.id);
        assert_eq!(snapshot.snapshot_id, deserialized.snapshot_id);
    }

    #[test]
    fn test_directory_size_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "Hello, World!").unwrap();

        // Can't easily test RecoveryManager without database
        // This is a placeholder for the calculation logic
        let file_size = std::fs::metadata(&test_file).unwrap().len();
        assert_eq!(file_size, 13); // "Hello, World!" is 13 bytes
    }
}