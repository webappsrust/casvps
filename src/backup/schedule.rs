use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use crate::database::Database;
use super::restic::ResticClient;
use super::{BackupJob, BackupResult};

/// Backup scheduler using cron expressions
pub struct BackupScheduler {
    database: Arc<Database>,
    restic_client: Arc<ResticClient>,
    scheduled_jobs: Arc<RwLock<HashMap<String, ScheduledJob>>>,
    running: Arc<RwLock<bool>>,
}

impl BackupScheduler {
    pub async fn new(database: Arc<Database>, restic_client: Arc<ResticClient>) -> Result<Self> {
        Ok(Self {
            database,
            restic_client,
            scheduled_jobs: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Start the scheduler
    pub async fn start(&self) -> Result<()> {
        if *self.running.read().await {
            return Ok(());
        }

        info!("Starting backup scheduler");

        *self.running.write().await = true;

        // Load existing jobs from database
        self.load_scheduled_jobs().await?;

        // Start scheduler loop
        self.start_scheduler_loop().await?;

        info!("Backup scheduler started");
        Ok(())
    }

    /// Stop the scheduler
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping backup scheduler");
        *self.running.write().await = false;
        Ok(())
    }

    /// Add a new job to the scheduler
    pub async fn add_job(&self, job_id: &str) -> Result<()> {
        info!("Adding job to scheduler: {}", job_id);

        // Load job from database
        let job = self.get_backup_job_from_db(job_id).await?;

        if !job.enabled {
            info!("Job is disabled, not scheduling: {}", job_id);
            return Ok(());
        }

        // Parse cron schedule
        let cron_schedule = self.parse_cron_schedule(&job.schedule)?;

        let scheduled_job = ScheduledJob {
            job_id: job_id.to_string(),
            schedule: cron_schedule,
            next_run: self.calculate_next_run(&job.schedule)?,
            last_run: job.last_run.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc)),
            running: false,
        };

        let mut jobs = self.scheduled_jobs.write().await;
        jobs.insert(job_id.to_string(), scheduled_job);

        info!("Job scheduled: {} (next run: {})", job_id, scheduled_job.next_run.format("%Y-%m-%d %H:%M:%S UTC"));
        Ok(())
    }

    /// Remove a job from the scheduler
    pub async fn remove_job(&self, job_id: &str) -> Result<()> {
        info!("Removing job from scheduler: {}", job_id);

        let mut jobs = self.scheduled_jobs.write().await;
        jobs.remove(job_id);

        info!("Job removed from scheduler: {}", job_id);
        Ok(())
    }

    /// Get scheduled job info
    pub async fn get_scheduled_jobs(&self) -> Vec<ScheduledJobInfo> {
        let jobs = self.scheduled_jobs.read().await;
        jobs.values().map(|job| ScheduledJobInfo {
            job_id: job.job_id.clone(),
            next_run: job.next_run,
            last_run: job.last_run,
            running: job.running,
        }).collect()
    }

    async fn load_scheduled_jobs(&self) -> Result<()> {
        info!("Loading scheduled jobs from database");

        let jobs = sqlx::query_as::<_, BackupJobRow>(
            "SELECT job_id, name, schedule, source_type, source_id, destination, retention_policy, compression, deduplication, encryption_key, enabled, created_at, last_run
             FROM backup_jobs WHERE enabled = TRUE"
        )
        .fetch_all(&self.database.pool)
        .await?;

        let mut scheduled_jobs = self.scheduled_jobs.write().await;

        for job_row in jobs {
            if let Ok(cron_schedule) = self.parse_cron_schedule(&job_row.schedule) {
                if let Ok(next_run) = self.calculate_next_run(&job_row.schedule) {
                    let scheduled_job = ScheduledJob {
                        job_id: job_row.job_id.clone(),
                        schedule: cron_schedule,
                        next_run,
                        last_run: job_row.last_run.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&chrono::Utc)),
                        running: false,
                    };

                    scheduled_jobs.insert(job_row.job_id, scheduled_job);
                }
            } else {
                warn!("Invalid cron schedule for job {}: {}", job_row.job_id, job_row.schedule);
            }
        }

        info!("Loaded {} scheduled jobs", scheduled_jobs.len());
        Ok(())
    }

    async fn start_scheduler_loop(&self) -> Result<()> {
        let database = self.database.clone();
        let restic_client = self.restic_client.clone();
        let scheduled_jobs = self.scheduled_jobs.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60)); // Check every minute

            loop {
                interval.tick().await;

                if !*running.read().await {
                    break;
                }

                let now = chrono::Utc::now();
                let mut jobs_to_run = Vec::new();

                // Find jobs that need to run
                {
                    let jobs_guard = scheduled_jobs.read().await;
                    for (job_id, scheduled_job) in jobs_guard.iter() {
                        if !scheduled_job.running && now >= scheduled_job.next_run {
                            jobs_to_run.push(job_id.clone());
                        }
                    }
                }

                // Run jobs
                for job_id in jobs_to_run {
                    let database = database.clone();
                    let restic_client = restic_client.clone();
                    let scheduled_jobs = scheduled_jobs.clone();

                    tokio::spawn(async move {
                        if let Err(e) = Self::run_scheduled_backup(
                            job_id.clone(),
                            database,
                            restic_client,
                            scheduled_jobs,
                        ).await {
                            error!("Failed to run scheduled backup {}: {}", job_id, e);
                        }
                    });
                }
            }
        });

        Ok(())
    }

    async fn run_scheduled_backup(
        job_id: String,
        database: Arc<Database>,
        restic_client: Arc<ResticClient>,
        scheduled_jobs: Arc<RwLock<HashMap<String, ScheduledJob>>>,
    ) -> Result<()> {
        info!("Running scheduled backup: {}", job_id);

        // Mark job as running
        {
            let mut jobs = scheduled_jobs.write().await;
            if let Some(job) = jobs.get_mut(&job_id) {
                job.running = true;
            }
        }

        let result = async {
            // Load job from database
            let job = Self::get_backup_job_from_db_static(&database, &job_id).await?;

            // Run backup
            let backup_result = restic_client.backup(&job).await?;

            // Log result
            Self::log_backup_result_static(&database, &job_id, &backup_result).await?;

            Ok::<BackupResult, anyhow::Error>(backup_result)
        }.await;

        // Update job state and schedule next run
        {
            let mut jobs = scheduled_jobs.write().await;
            if let Some(scheduled_job) = jobs.get_mut(&job_id) {
                scheduled_job.running = false;
                scheduled_job.last_run = Some(chrono::Utc::now());

                // Calculate next run time
                if let Ok(job) = Self::get_backup_job_from_db_static(&database, &job_id).await {
                    if let Ok(next_run) = Self::calculate_next_run_static(&job.schedule) {
                        scheduled_job.next_run = next_run;
                        info!("Next backup for {} scheduled at: {}", job_id, next_run.format("%Y-%m-%d %H:%M:%S UTC"));
                    }
                }
            }
        }

        match result {
            Ok(backup_result) => {
                if backup_result.success {
                    info!("Scheduled backup completed successfully: {}", job_id);
                } else {
                    error!("Scheduled backup failed: {} - {}", job_id, backup_result.error.unwrap_or_default());
                }
            },
            Err(e) => {
                error!("Scheduled backup error: {} - {}", job_id, e);
            }
        }

        Ok(())
    }

    async fn get_backup_job_from_db(&self, job_id: &str) -> Result<BackupJob> {
        Self::get_backup_job_from_db_static(&self.database, job_id).await
    }

    async fn get_backup_job_from_db_static(database: &Arc<Database>, job_id: &str) -> Result<BackupJob> {
        let job = sqlx::query_as::<_, BackupJobRow>(
            "SELECT job_id, name, schedule, source_type, source_id, destination, retention_policy, compression, deduplication, encryption_key, enabled, created_at, last_run
             FROM backup_jobs WHERE job_id = ?"
        )
        .bind(job_id)
        .fetch_one(&database.pool)
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

    async fn log_backup_result_static(database: &Arc<Database>, job_id: &str, result: &BackupResult) -> Result<()> {
        let details = serde_json::json!({
            "job_id": job_id,
            "success": result.success,
            "files_backed_up": result.files_backed_up,
            "bytes_backed_up": result.bytes_backed_up,
            "duration_seconds": result.duration.as_secs(),
            "snapshot_id": result.snapshot_id,
            "error": result.error,
            "scheduled": true
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
        .execute(&database.pool)
        .await?;

        // Update last_run timestamp
        sqlx::query!(
            "UPDATE backup_jobs SET last_run = ? WHERE job_id = ?",
            chrono::Utc::now(),
            job_id
        )
        .execute(&database.pool)
        .await?;

        Ok(())
    }

    fn parse_cron_schedule(&self, schedule: &str) -> Result<CronSchedule> {
        // Parse cron expression (format: "minute hour day month weekday")
        let parts: Vec<&str> = schedule.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(anyhow::anyhow!("Invalid cron schedule format: {}", schedule));
        }

        Ok(CronSchedule {
            minute: parts[0].to_string(),
            hour: parts[1].to_string(),
            day: parts[2].to_string(),
            month: parts[3].to_string(),
            weekday: parts[4].to_string(),
        })
    }

    fn calculate_next_run(&self, schedule: &str) -> Result<chrono::DateTime<chrono::Utc>> {
        Self::calculate_next_run_static(schedule)
    }

    fn calculate_next_run_static(schedule: &str) -> Result<chrono::DateTime<chrono::Utc>> {
        // Simple next run calculation - in production, would use a proper cron library
        // For now, just add intervals based on schedule patterns

        let now = chrono::Utc::now();

        // Parse common patterns
        if schedule.starts_with("0 2 * * *") {
            // Daily at 2 AM
            let tomorrow = now.date_naive().succ_opt()
                .ok_or_else(|| anyhow::anyhow!("Failed to calculate next day"))?;
            let next_run = tomorrow.and_hms_opt(2, 0, 0)
                .ok_or_else(|| anyhow::anyhow!("Failed to create datetime"))?
                .and_local_timezone(chrono::Utc)
                .single()
                .ok_or_else(|| anyhow::anyhow!("Failed to convert to UTC"))?;
            Ok(next_run)
        } else if schedule.starts_with("0 */6 * * *") {
            // Every 6 hours
            Ok(now + chrono::Duration::hours(6))
        } else if schedule.starts_with("0 * * * *") {
            // Every hour
            Ok(now + chrono::Duration::hours(1))
        } else if schedule.starts_with("*/15 * * * *") {
            // Every 15 minutes
            Ok(now + chrono::Duration::minutes(15))
        } else {
            // Default: next hour
            let next_hour = now.with_minute(0)
                .ok_or_else(|| anyhow::anyhow!("Failed to set minute to 0"))?
                .with_second(0)
                .ok_or_else(|| anyhow::anyhow!("Failed to set second to 0"))?
                + chrono::Duration::hours(1);
            Ok(next_hour)
        }
    }
}

#[derive(Debug, Clone)]
struct ScheduledJob {
    job_id: String,
    schedule: CronSchedule,
    next_run: chrono::DateTime<chrono::Utc>,
    last_run: Option<chrono::DateTime<chrono::Utc>>,
    running: bool,
}

#[derive(Debug, Clone)]
pub struct ScheduledJobInfo {
    pub job_id: String,
    pub next_run: chrono::DateTime<chrono::Utc>,
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,
    pub running: bool,
}

#[derive(Debug, Clone)]
struct CronSchedule {
    minute: String,
    hour: String,
    day: String,
    month: String,
    weekday: String,
}

#[derive(sqlx::FromRow)]
struct BackupJobRow {
    job_id: String,
    name: String,
    schedule: String,
    source_type: String,
    source_id: String,
    destination: String,
    retention_policy: String,
    compression: String,
    deduplication: bool,
    encryption_key: String,
    enabled: bool,
    created_at: String,
    last_run: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cron_schedule_parsing() {
        let scheduler = BackupScheduler {
            database: Arc::new(crate::database::Database::new(":memory:").await.unwrap()),
            restic_client: Arc::new(crate::backup::restic::ResticClient::new("/tmp", "test", crate::backup::StorageLimit::Percentage(10)).await.unwrap()),
            scheduled_jobs: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        };

        let schedule = scheduler.parse_cron_schedule("0 2 * * *").unwrap();
        assert_eq!(schedule.minute, "0");
        assert_eq!(schedule.hour, "2");
        assert_eq!(schedule.day, "*");
        assert_eq!(schedule.month, "*");
        assert_eq!(schedule.weekday, "*");
    }

    #[test]
    fn test_next_run_calculation() {
        let now = chrono::Utc::now();

        // Test daily schedule
        let next_run = BackupScheduler::calculate_next_run_static("0 2 * * *").unwrap();
        assert!(next_run > now);

        // Test hourly schedule
        let next_run = BackupScheduler::calculate_next_run_static("0 * * * *").unwrap();
        assert!(next_run > now);
        assert!(next_run <= now + chrono::Duration::hours(2));
    }
}