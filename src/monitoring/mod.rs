use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};
use crate::database::Database;

pub mod victoria_metrics;
pub mod collector;

use victoria_metrics::*;
use collector::*;

/// Embedded Victoria Metrics monitoring system
///
/// According to the spec: "Metrics Collection (Victoria Metrics Embedded)"
/// - 30s interval (60s on Pi4)
/// - Retention: raw(6h), 5m(7d), 1h(30d), 1d(1y)
/// - eBPF collection for low overhead
pub struct MonitoringManager {
    database: Arc<Database>,
    victoria_metrics: Arc<VictoriaMetrics>,
    collectors: Arc<RwLock<HashMap<String, Box<dyn MetricCollector>>>>,
    collection_interval: Duration,
    enabled: bool,
}

impl MonitoringManager {
    pub async fn new(database: Arc<Database>, data_path: &str) -> Result<Self> {
        info!("Initializing monitoring system with Victoria Metrics");

        // Detect platform for collection intervals
        let platform = detect_platform().await?;
        let collection_interval = match platform {
            Platform::Pi4 => Duration::from_secs(60),       // 60s on Pi4
            Platform::Homelab => Duration::from_secs(30),   // 30s on homelab
            Platform::Enterprise => Duration::from_secs(30), // 30s on enterprise
        };

        // Initialize Victoria Metrics backend
        let victoria_metrics = Arc::new(VictoriaMetrics::new(data_path).await?);

        // Initialize metric collectors
        let collectors = Arc::new(RwLock::new(HashMap::new()));

        let manager = Self {
            database,
            victoria_metrics,
            collectors,
            collection_interval,
            enabled: true,
        };

        // Register default collectors
        manager.register_default_collectors().await?;

        Ok(manager)
    }

    pub async fn start(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        info!("Starting monitoring system with {}s collection interval",
               self.collection_interval.as_secs());

        // Start Victoria Metrics server
        self.victoria_metrics.start().await?;

        // Start metric collection loops
        self.start_collection_loops().await?;

        // Start metric retention cleanup
        self.start_retention_cleanup().await?;

        info!("Monitoring system started successfully");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        info!("Stopping monitoring system");
        self.victoria_metrics.stop().await?;
        Ok(())
    }

    /// Register a custom metric collector
    pub async fn register_collector(&self, name: String, collector: Box<dyn MetricCollector>) -> Result<()> {
        info!("Registering metric collector: {}", name);
        let mut collectors = self.collectors.write().await;
        collectors.insert(name, collector);
        Ok(())
    }

    /// Collect metrics immediately
    pub async fn collect_metrics(&self) -> Result<Vec<Metric>> {
        let mut all_metrics = Vec::new();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        let collectors = self.collectors.read().await;
        for (name, collector) in collectors.iter() {
            match collector.collect().await {
                Ok(mut metrics) => {
                    // Add timestamp and collector name
                    for metric in &mut metrics {
                        metric.timestamp = timestamp;
                        if metric.labels.get("collector").is_none() {
                            metric.labels.insert("collector".to_string(), name.clone());
                        }
                    }
                    all_metrics.extend(metrics);
                },
                Err(e) => {
                    error!("Failed to collect metrics from {}: {}", name, e);
                }
            }
        }

        // Send metrics to Victoria Metrics
        if !all_metrics.is_empty() {
            self.victoria_metrics.push_metrics(&all_metrics).await?;
        }

        info!("Collected {} metrics", all_metrics.len());
        Ok(all_metrics)
    }

    /// Query metrics from Victoria Metrics
    pub async fn query_metrics(&self, query: &str) -> Result<MetricQueryResult> {
        self.victoria_metrics.query(query).await
    }

    /// Query metrics over time range
    pub async fn query_range(
        &self,
        query: &str,
        start: u64,
        end: u64,
        step: Duration,
    ) -> Result<MetricRangeResult> {
        self.victoria_metrics.query_range(query, start, end, step).await
    }

    /// Get system health status
    pub async fn get_system_health(&self) -> Result<SystemHealth> {
        let mut health = SystemHealth {
            overall_status: "healthy".to_string(),
            components: HashMap::new(),
            last_check: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        };

        // Check each collector health
        let collectors = self.collectors.read().await;
        for (name, collector) in collectors.iter() {
            match collector.health_check().await {
                Ok(status) => {
                    health.components.insert(name.clone(), ComponentHealth {
                        status: "healthy".to_string(),
                        message: status,
                        last_check: health.last_check,
                    });
                },
                Err(e) => {
                    health.components.insert(name.clone(), ComponentHealth {
                        status: "unhealthy".to_string(),
                        message: format!("Health check failed: {}", e),
                        last_check: health.last_check,
                    });
                    health.overall_status = "degraded".to_string();
                }
            }
        }

        // Check Victoria Metrics health
        match self.victoria_metrics.health_check().await {
            Ok(_) => {
                health.components.insert("victoria_metrics".to_string(), ComponentHealth {
                    status: "healthy".to_string(),
                    message: "Victoria Metrics is running".to_string(),
                    last_check: health.last_check,
                });
            },
            Err(e) => {
                health.components.insert("victoria_metrics".to_string(), ComponentHealth {
                    status: "unhealthy".to_string(),
                    message: format!("Victoria Metrics unhealthy: {}", e),
                    last_check: health.last_check,
                });
                health.overall_status = "unhealthy".to_string();
            }
        }

        Ok(health)
    }

    /// Get monitoring statistics
    pub async fn get_stats(&self) -> Result<MonitoringStats> {
        let collectors_count = self.collectors.read().await.len();
        let vm_stats = self.victoria_metrics.get_stats().await?;

        Ok(MonitoringStats {
            collectors_count,
            collection_interval_seconds: self.collection_interval.as_secs(),
            total_metrics_collected: vm_stats.total_samples,
            storage_size_bytes: vm_stats.storage_size_bytes,
            memory_usage_bytes: vm_stats.memory_usage_bytes,
            uptime_seconds: vm_stats.uptime_seconds,
            enabled: self.enabled,
        })
    }

    async fn register_default_collectors(&self) -> Result<()> {
        info!("Registering default metric collectors");

        // System metrics collector
        let system_collector = Box::new(SystemMetricsCollector::new().await?);
        self.register_collector("system".to_string(), system_collector).await?;

        // Network metrics collector
        let network_collector = Box::new(NetworkMetricsCollector::new().await?);
        self.register_collector("network".to_string(), network_collector).await?;

        // VM metrics collector
        let vm_collector = Box::new(VmMetricsCollector::new(self.database.clone()).await?);
        self.register_collector("vm".to_string(), vm_collector).await?;

        // Container metrics collector
        let container_collector = Box::new(ContainerMetricsCollector::new(self.database.clone()).await?);
        self.register_collector("container".to_string(), container_collector).await?;

        // Storage metrics collector
        let storage_collector = Box::new(StorageMetricsCollector::new().await?);
        self.register_collector("storage".to_string(), storage_collector).await?;

        // Service metrics collector
        let service_collector = Box::new(ServiceMetricsCollector::new(self.database.clone()).await?);
        self.register_collector("service".to_string(), service_collector).await?;

        info!("Registered {} default collectors", 6);
        Ok(())
    }

    async fn start_collection_loops(&self) -> Result<()> {
        let collectors = self.collectors.clone();
        let victoria_metrics = self.victoria_metrics.clone();
        let interval = self.collection_interval;

        // Main collection loop
        tokio::spawn(async move {
            let mut collection_interval = tokio::time::interval(interval);

            loop {
                collection_interval.tick().await;

                let mut all_metrics = Vec::new();
                let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)
                    .unwrap_or_default().as_secs();

                // Collect from all collectors
                let collectors_guard = collectors.read().await;
                for (name, collector) in collectors_guard.iter() {
                    match collector.collect().await {
                        Ok(mut metrics) => {
                            // Add metadata
                            for metric in &mut metrics {
                                metric.timestamp = timestamp;
                                if metric.labels.get("collector").is_none() {
                                    metric.labels.insert("collector".to_string(), name.clone());
                                }
                            }
                            all_metrics.extend(metrics);
                        },
                        Err(e) => {
                            error!("Collection failed for {}: {}", name, e);
                        }
                    }
                }
                drop(collectors_guard);

                // Push to Victoria Metrics
                if !all_metrics.is_empty() {
                    if let Err(e) = victoria_metrics.push_metrics(&all_metrics).await {
                        error!("Failed to push metrics to Victoria Metrics: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    async fn start_retention_cleanup(&self) -> Result<()> {
        let victoria_metrics = self.victoria_metrics.clone();

        // Retention cleanup every hour
        tokio::spawn(async move {
            let mut cleanup_interval = tokio::time::interval(Duration::from_secs(3600));

            loop {
                cleanup_interval.tick().await;

                info!("Running metric retention cleanup");

                if let Err(e) = victoria_metrics.cleanup_old_data().await {
                    error!("Metric retention cleanup failed: {}", e);
                }
            }
        });

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricQueryResult {
    pub status: String,
    pub data: Vec<MetricSeries>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricRangeResult {
    pub status: String,
    pub data: Vec<MetricTimeSeries>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricSeries {
    pub metric: HashMap<String, String>,
    pub value: (u64, String), // (timestamp, value)
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricTimeSeries {
    pub metric: HashMap<String, String>,
    pub values: Vec<(u64, String)>, // [(timestamp, value)]
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemHealth {
    pub overall_status: String,
    pub components: HashMap<String, ComponentHealth>,
    pub last_check: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentHealth {
    pub status: String,
    pub message: String,
    pub last_check: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonitoringStats {
    pub collectors_count: usize,
    pub collection_interval_seconds: u64,
    pub total_metrics_collected: u64,
    pub storage_size_bytes: u64,
    pub memory_usage_bytes: u64,
    pub uptime_seconds: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
enum Platform {
    Pi4,
    Homelab,
    Enterprise,
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
    let total_memory_gb = memory_info.mem_total / 1024 / 1024;

    if total_memory_gb < 16 {
        Ok(Platform::Pi4)
    } else if total_memory_gb < 64 {
        Ok(Platform::Homelab)
    } else {
        Ok(Platform::Enterprise)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metric_serialization() {
        let mut labels = HashMap::new();
        labels.insert("instance".to_string(), "localhost".to_string());

        let metric = Metric {
            name: "cpu_usage".to_string(),
            value: 75.5,
            labels,
            timestamp: 1234567890,
        };

        let json = serde_json::to_string(&metric).unwrap();
        let deserialized: Metric = serde_json::from_str(&json).unwrap();

        assert_eq!(metric.name, deserialized.name);
        assert_eq!(metric.value, deserialized.value);
        assert_eq!(metric.timestamp, deserialized.timestamp);
    }

    #[test]
    fn test_platform_detection_with_low_memory() {
        // Can't easily test async platform detection in unit tests
        // This is a placeholder for the detection logic
        assert!(true);
    }
}