use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::database::Database;

pub mod restic;
pub mod schedule;
pub mod recovery;

use restic::*;
use schedule::*;
use recovery::*;

/// Backup System using Restic
///
/// According to the spec: "Restic-Based Backups" with storage limits:
/// - Pi4: 5% max storage
/// - Homelab: 10% max storage
/// - Enterprise: 20% max storage
/// Default retention: 7d, 4w, 12m, 5y
pub struct BackupManager {
    database: Arc<Database>,
    restic_client: Arc<ResticClient>,
    scheduler: Arc<BackupScheduler>,
    recovery_manager: Arc<RecoveryManager>,
    storage_limit: StorageLimit,
    encryption_key: String,
    enabled: bool,
}

impl BackupManager {
    pub async fn new(database: Arc<Database>, backup_storage_path: &str) -> Result<Self> {
        info!("Initializing backup manager with Restic backend");

        // Detect platform for storage limits
        let platform = detect_platform().await?;
        let storage_limit = match platform {
            Platform::Pi4 => StorageLimit::Percentage(5),      // 5% max on Pi4
            Platform::Homelab => StorageLimit::Percentage(10), // 10% on homelab
            Platform::Enterprise => StorageLimit::Percentage(20), // 20% on enterprise
        };

        // Generate or load encryption key
        let encryption_key = Self::get_or_create_encryption_key(&database).await?;

        // Initialize Restic client
        let restic_client = Arc::new(ResticClient::new(
            backup_storage_path,
            &encryption_key,
            storage_limit.clone(),
        ).await?);

        // Initialize scheduler
        let scheduler = Arc::new(BackupScheduler::new(
            database.clone(),
            restic_client.clone(),
        ).await?);

        // Initialize recovery manager
        let recovery_manager = Arc::new(RecoveryManager::new(
            database.clone(),
            restic_client.clone(),
        ).await?);

        Ok(Self {
            database,
            restic_client,
            scheduler,
            recovery_manager,
            storage_limit,
            encryption_key,
            enabled: true,
        })
    }

    pub async fn start(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        info!("Starting backup services");

        // Initialize Restic repository
        self.restic_client.init_repository().await?;

        // Start scheduler
        self.scheduler.start().await?;

        // Start automatic cleanup
        self.start_cleanup_scheduler().await?;

        info!("Backup services started with {} storage limit", self.storage_limit);
        Ok(())
    }

    pub async fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Create backup job
    pub async fn create_backup_job(&self, job: &CreateBackupJobRequest) -> Result<String> {
        info!("Creating backup job: {}", job.name);

        let job_id = Uuid::new_v4().to_string();

        // Validate source paths
        self.validate_backup_sources(&job.sources).await?;

        // Create backup job in database
        sqlx::query!(
            "INSERT INTO backup_jobs (job_id, name, schedule, source_type, source_id, destination, retention_policy, compression, deduplication, encryption_key, enabled)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            job_id,
            job.name,
            job.schedule,
            job.source_type,
            job.source_ids.join(","),
            job.destination.unwrap_or_else(|| "default".to_string()),
            job.retention_policy.unwrap_or_else(|| "7d,4w,12m,5y".to_string()),
            job.compression.unwrap_or_else(|| "zstd".to_string()),
            job.deduplication.unwrap_or(true),
            self.encryption_key,
            job.enabled.unwrap_or(true)
        )
        .execute(&self.database.pool)
        .await?;

        // Add to scheduler
        self.scheduler.add_job(&job_id).await?;

        info!("Backup job created: {}", job_id);
        Ok(job_id)
    }

    /// Run backup job immediately
    pub async fn run_backup_now(&self, job_id: &str) -> Result<BackupResult> {
        info!("Running backup job immediately: {}", job_id);

        // Get job details
        let job = self.get_backup_job(job_id).await?;
        if !job.enabled {
            return Err(anyhow::anyhow!("Backup job is disabled: {}", job_id));
        }

        // Check storage limits
        self.check_storage_limits().await?;

        // Run backup
        let backup_result = self.restic_client.backup(&job).await?;

        // Log backup result
        self.log_backup_result(job_id, &backup_result).await?;

        // Clean up old backups if needed
        if backup_result.success {
            self.cleanup_old_backups(job_id).await?;
        }

        info!("Backup completed: {} ({})", job_id, if backup_result.success { "success" } else { "failed" });
        Ok(backup_result)
    }

    /// List all backup jobs
    pub async fn list_backup_jobs(&self) -> Result<Vec<BackupJob>> {
        let jobs = sqlx::query_as::<_, BackupJobRow>(
            "SELECT job_id, name, schedule, source_type, source_id, destination, retention_policy, compression, deduplication, encryption_key, enabled, created_at, last_run
             FROM backup_jobs ORDER BY created_at DESC"
        )
        .fetch_all(&self.database.pool)
        .await?;

        let backup_jobs = jobs.into_iter().map(|row| BackupJob {
            job_id: row.job_id,
            name: row.name,
            schedule: row.schedule,
            source_type: row.source_type,
            source_ids: row.source_id.split(',').map(|s| s.to_string()).collect(),
            destination: row.destination,
            retention_policy: row.retention_policy,
            compression: row.compression,
            deduplication: row.deduplication,
            enabled: row.enabled,
            created_at: row.created_at,
            last_run: row.last_run,
        }).collect();

        Ok(backup_jobs)
    }

    /// Get backup job details
    pub async fn get_backup_job(&self, job_id: &str) -> Result<BackupJob> {
        let job = sqlx::query_as::<_, BackupJobRow>(
            "SELECT job_id, name, schedule, source_type, source_id, destination, retention_policy, compression, deduplication, encryption_key, enabled, created_at, last_run
             FROM backup_jobs WHERE job_id = ?"
        )
        .bind(job_id)
        .fetch_one(&self.database.pool)
        .await?;

        Ok(BackupJob {
            job_id: job.job_id,
            name: job.name,
            schedule: job.schedule,
            source_type: job.source_type,
            source_ids: job.source_id.split(',').map(|s| s.to_string()).collect(),
            destination: job.destination,
            retention_policy: job.retention_policy,
            compression: job.compression,
            deduplication: job.deduplication,
            enabled: job.enabled,
            created_at: job.created_at,
            last_run: job.last_run,
        })
    }

    /// Delete backup job
    pub async fn delete_backup_job(&self, job_id: &str) -> Result<()> {
        info!("Deleting backup job: {}", job_id);

        // Remove from scheduler first
        self.scheduler.remove_job(job_id).await?;

        // Delete from database
        sqlx::query!("DELETE FROM backup_jobs WHERE job_id = ?", job_id)
            .execute(&self.database.pool)
            .await?;

        // Optionally remove backup data (user choice)
        // self.restic_client.forget_snapshots(job_id).await?;

        info!("Backup job deleted: {}", job_id);
        Ok(())
    }

    /// List backup snapshots
    pub async fn list_snapshots(&self, job_id: Option<&str>) -> Result<Vec<BackupSnapshot>> {
        self.restic_client.list_snapshots(job_id).await
    }

    /// Restore from backup
    pub async fn restore_backup(&self, request: &RestoreRequest) -> Result<RestoreResult> {
        info!("Restoring backup: snapshot {} to {}", request.snapshot_id, request.restore_path);

        // Validate restore target
        self.validate_restore_target(&request.restore_path).await?;

        // Perform restore
        let restore_result = self.recovery_manager.restore_snapshot(
            &request.snapshot_id,
            &request.restore_path,
            request.include_patterns.clone(),
            request.exclude_patterns.clone(),
        ).await?;

        // Log restore result
        self.log_restore_result(request, &restore_result).await?;

        info!("Restore completed: {} ({})", request.snapshot_id, if restore_result.success { "success" } else { "failed" });
        Ok(restore_result)
    }

    /// Get backup statistics
    pub async fn get_backup_stats(&self) -> Result<BackupStats> {
        let total_jobs = sqlx::query_scalar!("SELECT COUNT(*) FROM backup_jobs")
            .fetch_one(&self.database.pool)
            .await?;

        let enabled_jobs = sqlx::query_scalar!("SELECT COUNT(*) FROM backup_jobs WHERE enabled = TRUE")
            .fetch_one(&self.database.pool)
            .await?;

        let successful_backups_today = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM audit_log
             WHERE action = 'backup_completed'
             AND details LIKE '%\"success\":true%'
             AND timestamp > datetime('now', '-24 hours')"
        )
        .fetch_one(&self.database.pool)
        .await?;

        let failed_backups_today = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM audit_log
             WHERE action = 'backup_completed'
             AND details LIKE '%\"success\":false%'
             AND timestamp > datetime('now', '-24 hours')"
        )
        .fetch_one(&self.database.pool)
        .await?;

        // Get storage usage
        let storage_info = self.restic_client.get_repository_stats().await?;

        Ok(BackupStats {
            total_jobs: total_jobs as usize,
            enabled_jobs: enabled_jobs as usize,
            successful_backups_today: successful_backups_today as usize,
            failed_backups_today: failed_backups_today as usize,
            total_storage_used: storage_info.total_size,
            storage_limit: self.storage_limit.clone(),
            deduplication_ratio: storage_info.deduplication_ratio,
            last_backup: storage_info.last_backup,
        })
    }

    /// Manual cleanup of old backups
    pub async fn cleanup_old_backups(&self, job_id: &str) -> Result<()> {
        let job = self.get_backup_job(job_id).await?;
        self.restic_client.forget_old_snapshots(&job.retention_policy, Some(job_id)).await
    }

    async fn get_or_create_encryption_key(database: &Arc<Database>) -> Result<String> {
        // Try to get existing key
        if let Ok(Some(key)) = sqlx::query_scalar!(
            "SELECT value FROM system_config WHERE key = 'backup.encryption_key'"
        ).fetch_optional(&database.pool).await {
            let key_data: serde_json::Value = serde_json::from_str(&key)?;
            return Ok(key_data.as_str().unwrap_or_default().to_string());
        }

        // Generate new key
        let key = generate_encryption_key()?;

        sqlx::query!(
            "INSERT INTO system_config (key, value, category) VALUES (?, ?, ?)",
            "backup.encryption_key",
            serde_json::to_string(&key)?,
            "backup"
        )
        .execute(&database.pool)
        .await?;

        info!("Generated new backup encryption key");
        Ok(key)
    }

    async fn validate_backup_sources(&self, sources: &[String]) -> Result<()> {
        for source in sources {
            if !Path::new(source).exists() {
                return Err(anyhow::anyhow!("Backup source does not exist: {}", source));
            }
        }
        Ok(())
    }

    async fn validate_restore_target(&self, target: &str) -> Result<()> {
        let target_path = Path::new(target);

        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                return Err(anyhow::anyhow!("Restore target parent directory does not exist: {}", parent.display()));
            }
        }

        // Check if target exists and is not empty (safety check)
        if target_path.exists() && target_path.is_dir() {
            let mut entries = tokio::fs::read_dir(target_path).await?;
            if entries.next_entry().await?.is_some() {
                warn!("Restore target directory is not empty: {}", target);
            }
        }

        Ok(())
    }

    async fn check_storage_limits(&self) -> Result<()> {
        let stats = self.restic_client.get_repository_stats().await?;

        match &self.storage_limit {
            StorageLimit::Percentage(percent) => {
                let total_disk = get_total_disk_space().await?;
                let max_allowed = (total_disk * (*percent as u64)) / 100;

                if stats.total_size > max_allowed {
                    return Err(anyhow::anyhow!(
                        "Backup storage limit exceeded: {} bytes used, {} bytes allowed ({}% of disk)",
                        stats.total_size, max_allowed, percent
                    ));
                }
            },
            StorageLimit::Absolute(bytes) => {
                if stats.total_size > *bytes {
                    return Err(anyhow::anyhow!(
                        "Backup storage limit exceeded: {} bytes used, {} bytes allowed",
                        stats.total_size, bytes
                    ));
                }
            }
        }

        Ok(())
    }

    async fn log_backup_result(&self, job_id: &str, result: &BackupResult) -> Result<()> {
        let details = serde_json::json!({
            "job_id": job_id,
            "success": result.success,
            "files_backed_up": result.files_backed_up,
            "bytes_backed_up": result.bytes_backed_up,
            "duration_seconds": result.duration.as_secs(),
            "snapshot_id": result.snapshot_id,
            "error": result.error
        });

        sqlx::query!(
            "INSERT INTO audit_log (timestamp, user_id, action, resource_type, resource_id, details, ip_address)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            chrono::Utc::now(),
            "system",
            "backup_completed",
            "backup_job",
            job_id,
            serde_json::to_string(&details)?,
            None::<String>
        )
        .execute(&self.database.pool)
        .await?;

        // Update last_run timestamp
        sqlx::query!(
            "UPDATE backup_jobs SET last_run = ? WHERE job_id = ?",
            chrono::Utc::now(),
            job_id
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn log_restore_result(&self, request: &RestoreRequest, result: &RestoreResult) -> Result<()> {
        let details = serde_json::json!({
            "snapshot_id": request.snapshot_id,
            "restore_path": request.restore_path,
            "success": result.success,
            "files_restored": result.files_restored,
            "bytes_restored": result.bytes_restored,
            "duration_seconds": result.duration.as_secs(),
            "error": result.error
        });

        sqlx::query!(
            "INSERT INTO audit_log (timestamp, user_id, action, resource_type, resource_id, details, ip_address)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            chrono::Utc::now(),
            "system",
            "backup_restored",
            "backup_snapshot",
            request.snapshot_id,
            serde_json::to_string(&details)?,
            None::<String>
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn start_cleanup_scheduler(&self) -> Result<()> {
        let restic_client = self.restic_client.clone();
        let database = self.database.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_hours(24));

            loop {
                interval.tick().await;

                info!("Running daily backup cleanup");

                // Get all backup jobs for cleanup
                if let Ok(jobs) = sqlx::query_as::<_, BackupJobRow>(
                    "SELECT job_id, name, schedule, source_type, source_id, destination, retention_policy, compression, deduplication, encryption_key, enabled, created_at, last_run
                     FROM backup_jobs WHERE enabled = TRUE"
                ).fetch_all(&database.pool).await {

                    for job_row in jobs {
                        if let Err(e) = restic_client.forget_old_snapshots(&job_row.retention_policy, Some(&job_row.job_id)).await {
                            error!("Failed to cleanup backups for job {}: {}", job_row.job_id, e);
                        }
                    }
                }
            }
        });

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CreateBackupJobRequest {
    pub name: String,
    pub schedule: String, // Cron expression
    pub source_type: String, // 'vm', 'container', 'directory', 'database'
    pub source_ids: Vec<String>, // VM IDs, container IDs, or directory paths
    pub sources: Vec<String>, // Actual paths to backup
    pub destination: Option<String>, // Repository name (optional, defaults to 'default')
    pub retention_policy: Option<String>, // '7d,4w,12m,5y'
    pub compression: Option<String>, // 'zstd', 'lz4', 'none'
    pub deduplication: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct BackupJob {
    pub job_id: String,
    pub name: String,
    pub schedule: String,
    pub source_type: String,
    pub source_ids: Vec<String>,
    pub destination: String,
    pub retention_policy: String,
    pub compression: String,
    pub deduplication: bool,
    pub enabled: bool,
    pub created_at: String,
    pub last_run: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BackupResult {
    pub success: bool,
    pub snapshot_id: Option<String>,
    pub files_backed_up: u64,
    pub bytes_backed_up: u64,
    pub duration: std::time::Duration,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BackupSnapshot {
    pub id: String,
    pub job_id: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub paths: Vec<String>,
    pub hostname: String,
    pub username: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct RestoreRequest {
    pub snapshot_id: String,
    pub restore_path: String,
    pub include_patterns: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
    pub overwrite: bool,
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
pub enum StorageLimit {
    Percentage(u32), // Percentage of disk space
    Absolute(u64),   // Absolute bytes
}

impl std::fmt::Display for StorageLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageLimit::Percentage(p) => write!(f, "{}% of disk", p),
            StorageLimit::Absolute(b) => write!(f, "{} bytes", b),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackupStats {
    pub total_jobs: usize,
    pub enabled_jobs: usize,
    pub successful_backups_today: usize,
    pub failed_backups_today: usize,
    pub total_storage_used: u64,
    pub storage_limit: StorageLimit,
    pub deduplication_ratio: f64,
    pub last_backup: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
enum Platform {
    Pi4,
    Homelab,
    Enterprise,
}

#[derive(sqlx::FromRow)]
struct BackupJobRow {
    job_id: String,
    name: String,
    schedule: String,
    source_type: String,
    source_id: String, // Comma-separated IDs
    destination: String,
    retention_policy: String,
    compression: String,
    deduplication: bool,
    encryption_key: String,
    enabled: bool,
    created_at: String,
    last_run: Option<String>,
}

async fn detect_platform() -> Result<Platform> {
    // Check for Raspberry Pi
    if let Ok(model) = tokio::fs::read_to_string("/proc/device-tree/model").await {
        if model.contains("Raspberry Pi 4") || model.contains("Raspberry Pi 5") {
            return Ok(Platform::Pi4);
        }
    }

    // Check memory to determine platform type
    let memory_info = procfs::Meminfo::new()?;
    let total_memory_gb = memory_info.mem_total / 1024 / 1024; // Convert to GB

    if total_memory_gb < 16 {
        Ok(Platform::Pi4) // Assume Pi4 for low memory systems
    } else if total_memory_gb < 64 {
        Ok(Platform::Homelab)
    } else {
        Ok(Platform::Enterprise)
    }
}

async fn get_total_disk_space() -> Result<u64> {
    use nix::sys::statvfs::statvfs;

    let stat = statvfs("/var/lib/casvps")?;
    Ok(stat.blocks() * stat.block_size())
}

fn generate_encryption_key() -> Result<String> {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    Ok(base64::encode(&key))
}