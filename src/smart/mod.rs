use anyhow::Result;
use std::sync::Arc;
use sysinfo::System;
use tracing::{info, debug, warn};
use crate::database::Database;

pub mod resource_allocator;
pub mod network_optimizer;
pub mod storage_optimizer;
pub mod pattern_detector;
pub mod recovery_engine;

pub use resource_allocator::SmartResourceAllocator;
pub use network_optimizer::SmartNetworkOptimizer;
pub use storage_optimizer::SmartStorageOptimizer;
pub use pattern_detector::PatternDetector;
pub use recovery_engine::RecoveryEngine;

pub struct SmartLogicSystem {
    database: Arc<Database>,
    resource_allocator: SmartResourceAllocator,
    network_optimizer: SmartNetworkOptimizer,
    storage_optimizer: SmartStorageOptimizer,
    pattern_detector: PatternDetector,
    recovery_engine: RecoveryEngine,
}

impl SmartLogicSystem {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database: database.clone(),
            resource_allocator: SmartResourceAllocator::new(database.clone()),
            network_optimizer: SmartNetworkOptimizer::new(database.clone()),
            storage_optimizer: SmartStorageOptimizer::new(database.clone()),
            pattern_detector: PatternDetector::new(database.clone()),
            recovery_engine: RecoveryEngine::new(database.clone()),
        }
    }

    pub async fn optimize_system(&self) -> Result<()> {
        debug!("Running smart system optimization");

        // Gather system metrics
        let sys = System::new_all();
        let metrics = self.gather_system_metrics(&sys)?;

        // Optimize based on current state
        self.optimize_memory(&metrics).await?;
        self.optimize_cpu(&metrics).await?;
        self.optimize_storage(&metrics).await?;
        self.optimize_network(&metrics).await?;

        Ok(())
    }

    fn gather_system_metrics(&self, sys: &System) -> Result<SystemMetrics> {
        Ok(SystemMetrics {
            cpu_usage: sys.global_cpu_info().cpu_usage(),
            memory_total: sys.total_memory(),
            memory_used: sys.used_memory(),
            memory_available: sys.available_memory(),
            swap_total: sys.total_swap(),
            swap_used: sys.used_swap(),
            load_avg: sys.load_average(),
        })
    }

    async fn optimize_memory(&self, metrics: &SystemMetrics) -> Result<()> {
        let memory_pressure = metrics.memory_used as f64 / metrics.memory_total as f64;

        if memory_pressure > 0.9 {
            warn!("High memory pressure detected: {:.1}%", memory_pressure * 100.0);

            // Enable aggressive KSM
            self.enable_aggressive_ksm().await?;

            // Increase swappiness if swap is available
            if metrics.swap_total > 0 {
                self.adjust_swappiness(60).await?;
            }

            // Balloon down VMs if needed
            self.resource_allocator.balloon_vms(0.1).await?;
        } else if memory_pressure < 0.5 {
            // Reduce KSM aggressiveness to save CPU
            self.reduce_ksm_scanning().await?;

            // Reduce swappiness
            self.adjust_swappiness(10).await?;
        }

        Ok(())
    }

    async fn optimize_cpu(&self, _metrics: &SystemMetrics) -> Result<()> {
        // CPU optimization logic
        Ok(())
    }

    async fn optimize_storage(&self, _metrics: &SystemMetrics) -> Result<()> {
        // Storage optimization logic
        self.storage_optimizer.optimize().await?;
        Ok(())
    }

    async fn optimize_network(&self, _metrics: &SystemMetrics) -> Result<()> {
        // Network optimization logic
        self.network_optimizer.optimize().await?;
        Ok(())
    }

    async fn enable_aggressive_ksm(&self) -> Result<()> {
        debug!("Enabling aggressive KSM");
        std::fs::write("/sys/kernel/mm/ksm/pages_to_scan", "1000")?;
        std::fs::write("/sys/kernel/mm/ksm/sleep_millisecs", "10")?;
        Ok(())
    }

    async fn reduce_ksm_scanning(&self) -> Result<()> {
        debug!("Reducing KSM scanning");
        std::fs::write("/sys/kernel/mm/ksm/pages_to_scan", "100")?;
        std::fs::write("/sys/kernel/mm/ksm/sleep_millisecs", "100")?;
        Ok(())
    }

    async fn adjust_swappiness(&self, value: u32) -> Result<()> {
        debug!("Adjusting swappiness to {}", value);
        std::fs::write("/proc/sys/vm/swappiness", value.to_string())?;
        self.database.set_config("vm.swappiness", &value.to_string()).await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub memory_available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub load_avg: sysinfo::LoadAvg,
}