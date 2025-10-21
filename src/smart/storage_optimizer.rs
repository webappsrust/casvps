use anyhow::Result;
use std::sync::Arc;
use std::path::Path;
use tracing::{debug, info, warn};
use crate::database::Database;

pub struct SmartStorageOptimizer {
    database: Arc<Database>,
}

impl SmartStorageOptimizer {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    pub async fn optimize(&self) -> Result<()> {
        debug!("Running smart storage optimization");

        // Check storage health
        self.check_storage_health().await?;

        // Optimize I/O scheduler
        self.optimize_io_scheduler().await?;

        // Enable compression if beneficial
        self.optimize_compression().await?;

        // Clean up if needed
        self.smart_cleanup().await?;

        Ok(())
    }

    async fn check_storage_health(&self) -> Result<()> {
        // Check disk usage
        let usage = self.get_storage_usage("/var/lib/casvps")?;

        if usage > 95.0 {
            warn!("Critical storage usage: {:.1}%", usage);
            self.emergency_cleanup().await?;
        } else if usage > 85.0 {
            warn!("High storage usage: {:.1}%", usage);
            self.routine_cleanup().await?;
        }

        Ok(())
    }

    async fn optimize_io_scheduler(&self) -> Result<()> {
        // Detect storage type and set optimal scheduler
        let devices = self.detect_storage_devices()?;

        for device in devices {
            let scheduler = match device.device_type {
                StorageType::NVMe => "none",
                StorageType::SSD => "noop",
                StorageType::HDD => "mq-deadline",
            };

            self.set_io_scheduler(&device.name, scheduler)?;
            debug!("Set {} scheduler to {}", device.name, scheduler);
        }

        Ok(())
    }

    async fn optimize_compression(&self) -> Result<()> {
        // Enable compression for specific file types
        let cpu_count = num_cpus::get();

        let compression_algo = if cpu_count >= 8 {
            "zstd"  // Better compression ratio, more CPU intensive
        } else if cpu_count >= 4 {
            "lz4"   // Fast compression
        } else {
            "none"  // Pi4 - avoid compression overhead
        };

        self.database.set_config("storage.compression", compression_algo).await?;
        info!("Set storage compression to {}", compression_algo);

        Ok(())
    }

    async fn smart_cleanup(&self) -> Result<()> {
        let usage = self.get_storage_usage("/var/lib/casvps")?;

        if usage > 80.0 {
            info!("Running smart cleanup (usage: {:.1}%)", usage);

            // Clean in order of importance
            self.cleanup_old_logs().await?;
            self.cleanup_orphaned_disks().await?;
            self.cleanup_old_snapshots().await?;
            self.cleanup_temp_isos().await?;
        }

        Ok(())
    }

    async fn emergency_cleanup(&self) -> Result<()> {
        warn!("Running emergency cleanup");

        // Aggressive cleanup
        self.cleanup_all_logs().await?;
        self.cleanup_all_temp_files().await?;
        self.cleanup_old_backups().await?;

        Ok(())
    }

    async fn routine_cleanup(&self) -> Result<()> {
        debug!("Running routine cleanup");

        self.cleanup_old_logs().await?;
        self.cleanup_temp_isos().await?;

        Ok(())
    }

    fn get_storage_usage(&self, path: &str) -> Result<f64> {
        let stat = nix::sys::statvfs::statvfs(path)?;
        let total = stat.blocks() * stat.block_size();
        let available = stat.blocks_available() * stat.block_size();
        let used = total - available;

        Ok((used as f64 / total as f64) * 100.0)
    }

    fn detect_storage_devices(&self) -> Result<Vec<StorageDevice>> {
        let mut devices = Vec::new();

        // Read from /sys/block
        for entry in std::fs::read_dir("/sys/block")? {
            let entry = entry?;
            let name = entry.file_name().into_string().unwrap_or_default();

            // Skip loop devices and ram disks
            if name.starts_with("loop") || name.starts_with("ram") {
                continue;
            }

            let rotational_path = format!("/sys/block/{}/queue/rotational", name);
            let device_type = if Path::new(&rotational_path).exists() {
                let rotational = std::fs::read_to_string(&rotational_path)?
                    .trim()
                    .parse::<i32>()
                    .unwrap_or(1);

                if name.starts_with("nvme") {
                    StorageType::NVMe
                } else if rotational == 0 {
                    StorageType::SSD
                } else {
                    StorageType::HDD
                }
            } else {
                StorageType::HDD  // Default
            };

            devices.push(StorageDevice {
                name: name.clone(),
                device_type,
            });
        }

        Ok(devices)
    }

    fn set_io_scheduler(&self, device: &str, scheduler: &str) -> Result<()> {
        let path = format!("/sys/block/{}/queue/scheduler", device);

        if Path::new(&path).exists() {
            std::fs::write(&path, scheduler).ok();  // Ignore errors for virtual devices
        }

        Ok(())
    }

    async fn cleanup_old_logs(&self) -> Result<()> {
        // Clean logs older than 7 days
        std::process::Command::new("find")
            .args(&[
                "/var/log/casvps",
                "-type", "f",
                "-mtime", "+7",
                "-delete"
            ])
            .output()?;

        Ok(())
    }

    async fn cleanup_all_logs(&self) -> Result<()> {
        // Emergency: truncate all logs
        std::process::Command::new("find")
            .args(&[
                "/var/log/casvps",
                "-type", "f",
                "-exec", "truncate", "-s", "0", "{}", ";"
            ])
            .output()?;

        Ok(())
    }

    async fn cleanup_orphaned_disks(&self) -> Result<()> {
        // Find and remove orphaned VM disks
        info!("Checking for orphaned disks");

        // TODO: Query database for valid VM disks and compare with filesystem

        Ok(())
    }

    async fn cleanup_old_snapshots(&self) -> Result<()> {
        // Remove snapshots older than retention policy
        let retention_days = self.database
            .get_config("snapshot.retention.days")
            .await?
            .parse::<i32>()
            .unwrap_or(7);

        info!("Cleaning snapshots older than {} days", retention_days);

        // TODO: Query database and remove old snapshots

        Ok(())
    }

    async fn cleanup_temp_isos(&self) -> Result<()> {
        // Clean cached ISOs older than 24 hours
        std::process::Command::new("find")
            .args(&[
                "/var/lib/casvps/iso/cache",
                "-type", "f",
                "-mtime", "+1",
                "-delete"
            ])
            .output()?;

        Ok(())
    }

    async fn cleanup_all_temp_files(&self) -> Result<()> {
        // Emergency: remove all temp files
        std::process::Command::new("rm")
            .args(&["-rf", "/var/lib/casvps/tmp/*"])
            .output()?;

        Ok(())
    }

    async fn cleanup_old_backups(&self) -> Result<()> {
        // Remove backups beyond retention
        warn!("Cleaning old backups to free space");

        // TODO: Implement backup cleanup logic

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct StorageDevice {
    name: String,
    device_type: StorageType,
}

#[derive(Debug, Clone)]
enum StorageType {
    NVMe,
    SSD,
    HDD,
}