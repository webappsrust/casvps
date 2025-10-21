use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use async_trait::async_trait;
use tracing::{debug, warn};
use serde::{Deserialize, Serialize};
use crate::database::Database;
use super::Metric;

/// Trait for metric collectors
#[async_trait]
pub trait MetricCollector: Send + Sync {
    async fn collect(&self) -> Result<Vec<Metric>>;
    async fn health_check(&self) -> Result<String>;
}

/// System metrics collector (CPU, Memory, Load)
pub struct SystemMetricsCollector {
    system: Arc<sysinfo::System>,
}

impl SystemMetricsCollector {
    pub async fn new() -> Result<Self> {
        let mut system = sysinfo::System::new_all();
        system.refresh_all();

        Ok(Self {
            system: Arc::new(system),
        })
    }
}

#[async_trait]
impl MetricCollector for SystemMetricsCollector {
    async fn collect(&self) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // Clone for refreshing (sysinfo requires mut access)
        let mut system = sysinfo::System::new_all();
        system.refresh_all();

        // CPU usage
        let cpu_usage = system.global_cpu_usage();
        metrics.push(Metric {
            name: "cpu_usage_percent".to_string(),
            value: cpu_usage as f64,
            labels: create_node_labels(),
            timestamp,
        });

        // CPU cores
        for (i, cpu) in system.cpus().iter().enumerate() {
            let mut labels = create_node_labels();
            labels.insert("cpu".to_string(), i.to_string());

            metrics.push(Metric {
                name: "cpu_core_usage_percent".to_string(),
                value: cpu.cpu_usage() as f64,
                labels,
                timestamp,
            });
        }

        // Memory usage
        let total_memory = system.total_memory();
        let available_memory = system.available_memory();
        let used_memory = total_memory - available_memory;

        metrics.push(Metric {
            name: "memory_total_bytes".to_string(),
            value: total_memory as f64,
            labels: create_node_labels(),
            timestamp,
        });

        metrics.push(Metric {
            name: "memory_available_bytes".to_string(),
            value: available_memory as f64,
            labels: create_node_labels(),
            timestamp,
        });

        metrics.push(Metric {
            name: "memory_used_bytes".to_string(),
            value: used_memory as f64,
            labels: create_node_labels(),
            timestamp,
        });

        metrics.push(Metric {
            name: "memory_usage_percent".to_string(),
            value: (used_memory as f64 / total_memory as f64) * 100.0,
            labels: create_node_labels(),
            timestamp,
        });

        // Swap usage
        let total_swap = system.total_swap();
        let used_swap = system.used_swap();

        if total_swap > 0 {
            metrics.push(Metric {
                name: "swap_total_bytes".to_string(),
                value: total_swap as f64,
                labels: create_node_labels(),
                timestamp,
            });

            metrics.push(Metric {
                name: "swap_used_bytes".to_string(),
                value: used_swap as f64,
                labels: create_node_labels(),
                timestamp,
            });

            metrics.push(Metric {
                name: "swap_usage_percent".to_string(),
                value: (used_swap as f64 / total_swap as f64) * 100.0,
                labels: create_node_labels(),
                timestamp,
            });
        }

        // Load average (Linux only)
        if let Ok(loadavg) = procfs::LoadAverage::new() {
            metrics.push(Metric {
                name: "load_average_1m".to_string(),
                value: loadavg.one,
                labels: create_node_labels(),
                timestamp,
            });

            metrics.push(Metric {
                name: "load_average_5m".to_string(),
                value: loadavg.five,
                labels: create_node_labels(),
                timestamp,
            });

            metrics.push(Metric {
                name: "load_average_15m".to_string(),
                value: loadavg.fifteen,
                labels: create_node_labels(),
                timestamp,
            });
        }

        debug!("Collected {} system metrics", metrics.len());
        Ok(metrics)
    }

    async fn health_check(&self) -> Result<String> {
        Ok("System metrics collector operational".to_string())
    }
}

/// Network metrics collector
pub struct NetworkMetricsCollector {
    interfaces: Vec<String>,
}

impl NetworkMetricsCollector {
    pub async fn new() -> Result<Self> {
        let interfaces = get_network_interfaces().await?;

        Ok(Self {
            interfaces,
        })
    }
}

#[async_trait]
impl MetricCollector for NetworkMetricsCollector {
    async fn collect(&self) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // Read network stats from /proc/net/dev
        if let Ok(net_dev) = procfs::net::dev_status() {
            for interface in net_dev.values() {
                if self.interfaces.contains(&interface.name) || interface.name == "lo" {
                    let mut labels = create_node_labels();
                    labels.insert("interface".to_string(), interface.name.clone());

                    // Received bytes/packets
                    metrics.push(Metric {
                        name: "network_receive_bytes".to_string(),
                        value: interface.recv_bytes as f64,
                        labels: labels.clone(),
                        timestamp,
                    });

                    metrics.push(Metric {
                        name: "network_receive_packets".to_string(),
                        value: interface.recv_packets as f64,
                        labels: labels.clone(),
                        timestamp,
                    });

                    // Transmitted bytes/packets
                    metrics.push(Metric {
                        name: "network_transmit_bytes".to_string(),
                        value: interface.sent_bytes as f64,
                        labels: labels.clone(),
                        timestamp,
                    });

                    metrics.push(Metric {
                        name: "network_transmit_packets".to_string(),
                        value: interface.sent_packets as f64,
                        labels: labels.clone(),
                        timestamp,
                    });

                    // Errors and drops
                    metrics.push(Metric {
                        name: "network_receive_errors".to_string(),
                        value: interface.recv_errors as f64,
                        labels: labels.clone(),
                        timestamp,
                    });

                    metrics.push(Metric {
                        name: "network_transmit_errors".to_string(),
                        value: interface.sent_errors as f64,
                        labels: labels.clone(),
                        timestamp,
                    });

                    metrics.push(Metric {
                        name: "network_receive_drop".to_string(),
                        value: interface.recv_drop as f64,
                        labels: labels.clone(),
                        timestamp,
                    });

                    metrics.push(Metric {
                        name: "network_transmit_drop".to_string(),
                        value: interface.sent_drop as f64,
                        labels,
                        timestamp,
                    });
                }
            }
        }

        debug!("Collected {} network metrics", metrics.len());
        Ok(metrics)
    }

    async fn health_check(&self) -> Result<String> {
        Ok(format!("Network metrics collector operational, monitoring {} interfaces",
                  self.interfaces.len()))
    }
}

/// VM metrics collector
pub struct VmMetricsCollector {
    database: Arc<Database>,
}

impl VmMetricsCollector {
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        Ok(Self {
            database,
        })
    }
}

#[async_trait]
impl MetricCollector for VmMetricsCollector {
    async fn collect(&self) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // Get VM statistics from database
        let vms = sqlx::query_as::<_, VmRow>(
            "SELECT vm_id, name, state, config FROM vms"
        )
        .fetch_all(&self.database.pool)
        .await?;

        let mut vm_states = HashMap::new();
        for vm in &vms {
            *vm_states.entry(vm.state.clone()).or_insert(0) += 1;

            // Per-VM metrics
            let mut labels = create_node_labels();
            labels.insert("vm_id".to_string(), vm.vm_id.clone());
            labels.insert("vm_name".to_string(), vm.name.clone());
            labels.insert("state".to_string(), vm.state.clone());

            // VM state as numeric (1 = running, 0 = stopped)
            let state_value = match vm.state.as_str() {
                "running" => 1.0,
                _ => 0.0,
            };

            metrics.push(Metric {
                name: "vm_state".to_string(),
                value: state_value,
                labels,
                timestamp,
            });

            // VM configuration metrics (if available)
            if let Ok(config) = serde_json::from_str::<VmConfig>(&vm.config) {
                let mut config_labels = create_node_labels();
                config_labels.insert("vm_id".to_string(), vm.vm_id.clone());
                config_labels.insert("vm_name".to_string(), vm.name.clone());

                if let Some(memory_mb) = config.memory_mb {
                    metrics.push(Metric {
                        name: "vm_memory_allocated_bytes".to_string(),
                        value: (memory_mb * 1024 * 1024) as f64,
                        labels: config_labels.clone(),
                        timestamp,
                    });
                }

                if let Some(vcpus) = config.vcpus {
                    metrics.push(Metric {
                        name: "vm_vcpus_allocated".to_string(),
                        value: vcpus as f64,
                        labels: config_labels,
                        timestamp,
                    });
                }
            }
        }

        // Aggregate VM state metrics
        for (state, count) in vm_states {
            let mut labels = create_node_labels();
            labels.insert("state".to_string(), state);

            metrics.push(Metric {
                name: "vms_count".to_string(),
                value: count as f64,
                labels,
                timestamp,
            });
        }

        metrics.push(Metric {
            name: "vms_total".to_string(),
            value: vms.len() as f64,
            labels: create_node_labels(),
            timestamp,
        });

        debug!("Collected {} VM metrics", metrics.len());
        Ok(metrics)
    }

    async fn health_check(&self) -> Result<String> {
        let vm_count = sqlx::query_scalar!("SELECT COUNT(*) FROM vms")
            .fetch_one(&self.database.pool)
            .await?;

        Ok(format!("VM metrics collector operational, monitoring {} VMs", vm_count))
    }
}

/// Container metrics collector
pub struct ContainerMetricsCollector {
    database: Arc<Database>,
}

impl ContainerMetricsCollector {
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        Ok(Self {
            database,
        })
    }
}

#[async_trait]
impl MetricCollector for ContainerMetricsCollector {
    async fn collect(&self) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // Get container statistics from database
        let containers = sqlx::query_as::<_, ContainerRow>(
            "SELECT container_id, name, state, image, config FROM containers"
        )
        .fetch_all(&self.database.pool)
        .await?;

        let mut container_states = HashMap::new();
        for container in &containers {
            *container_states.entry(container.state.clone()).or_insert(0) += 1;

            // Per-container metrics
            let mut labels = create_node_labels();
            labels.insert("container_id".to_string(), container.container_id.clone());
            labels.insert("container_name".to_string(), container.name.clone());
            labels.insert("image".to_string(), container.image.clone());
            labels.insert("state".to_string(), container.state.clone());

            // Container state as numeric
            let state_value = match container.state.as_str() {
                "running" => 1.0,
                _ => 0.0,
            };

            metrics.push(Metric {
                name: "container_state".to_string(),
                value: state_value,
                labels,
                timestamp,
            });
        }

        // Aggregate container state metrics
        for (state, count) in container_states {
            let mut labels = create_node_labels();
            labels.insert("state".to_string(), state);

            metrics.push(Metric {
                name: "containers_count".to_string(),
                value: count as f64,
                labels,
                timestamp,
            });
        }

        metrics.push(Metric {
            name: "containers_total".to_string(),
            value: containers.len() as f64,
            labels: create_node_labels(),
            timestamp,
        });

        debug!("Collected {} container metrics", metrics.len());
        Ok(metrics)
    }

    async fn health_check(&self) -> Result<String> {
        let container_count = sqlx::query_scalar!("SELECT COUNT(*) FROM containers")
            .fetch_one(&self.database.pool)
            .await?;

        Ok(format!("Container metrics collector operational, monitoring {} containers", container_count))
    }
}

/// Storage metrics collector
pub struct StorageMetricsCollector {
    mount_points: Vec<String>,
}

impl StorageMetricsCollector {
    pub async fn new() -> Result<Self> {
        let mount_points = vec![
            "/".to_string(),
            "/var/lib/casvps".to_string(),
            "/var/lib/casvps/storage".to_string(),
        ];

        Ok(Self {
            mount_points,
        })
    }
}

#[async_trait]
impl MetricCollector for StorageMetricsCollector {
    async fn collect(&self) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        for mount_point in &self.mount_points {
            if let Ok(stat) = nix::sys::statvfs::statvfs(mount_point.as_str()) {
                let mut labels = create_node_labels();
                labels.insert("mount_point".to_string(), mount_point.clone());

                let block_size = stat.block_size() as u64;
                let total_size = stat.blocks() * block_size;
                let available_size = stat.blocks_available() * block_size;
                let used_size = total_size - available_size;

                metrics.push(Metric {
                    name: "storage_total_bytes".to_string(),
                    value: total_size as f64,
                    labels: labels.clone(),
                    timestamp,
                });

                metrics.push(Metric {
                    name: "storage_available_bytes".to_string(),
                    value: available_size as f64,
                    labels: labels.clone(),
                    timestamp,
                });

                metrics.push(Metric {
                    name: "storage_used_bytes".to_string(),
                    value: used_size as f64,
                    labels: labels.clone(),
                    timestamp,
                });

                metrics.push(Metric {
                    name: "storage_usage_percent".to_string(),
                    value: (used_size as f64 / total_size as f64) * 100.0,
                    labels,
                    timestamp,
                });
            }
        }

        debug!("Collected {} storage metrics", metrics.len());
        Ok(metrics)
    }

    async fn health_check(&self) -> Result<String> {
        Ok(format!("Storage metrics collector operational, monitoring {} mount points",
                  self.mount_points.len()))
    }
}

/// Service metrics collector
pub struct ServiceMetricsCollector {
    database: Arc<Database>,
}

impl ServiceMetricsCollector {
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        Ok(Self {
            database,
        })
    }
}

#[async_trait]
impl MetricCollector for ServiceMetricsCollector {
    async fn collect(&self) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // Get monitored services from database
        let services = sqlx::query_as::<_, MonitoredServiceRow>(
            "SELECT id, service_name, last_status, check_interval, enabled FROM monitored_services WHERE enabled = TRUE"
        )
        .fetch_all(&self.database.pool)
        .await?;

        for service in &services {
            let mut labels = create_node_labels();
            labels.insert("service_id".to_string(), service.id.clone());
            labels.insert("service_name".to_string(), service.service_name.clone());
            labels.insert("status".to_string(), service.last_status.clone().unwrap_or_else(|| "unknown".to_string()));

            // Service status as numeric (1 = up, 0 = down)
            let status_value = match service.last_status.as_deref() {
                Some("up") => 1.0,
                Some("down") => 0.0,
                _ => -1.0, // Unknown
            };

            metrics.push(Metric {
                name: "service_status".to_string(),
                value: status_value,
                labels: labels.clone(),
                timestamp,
            });

            metrics.push(Metric {
                name: "service_check_interval_seconds".to_string(),
                value: service.check_interval as f64,
                labels,
                timestamp,
            });
        }

        // Service count by status
        let mut status_counts = HashMap::new();
        for service in &services {
            let status = service.last_status.clone().unwrap_or_else(|| "unknown".to_string());
            *status_counts.entry(status).or_insert(0) += 1;
        }

        for (status, count) in status_counts {
            let mut labels = create_node_labels();
            labels.insert("status".to_string(), status);

            metrics.push(Metric {
                name: "services_count".to_string(),
                value: count as f64,
                labels,
                timestamp,
            });
        }

        debug!("Collected {} service metrics", metrics.len());
        Ok(metrics)
    }

    async fn health_check(&self) -> Result<String> {
        let service_count = sqlx::query_scalar!("SELECT COUNT(*) FROM monitored_services WHERE enabled = TRUE")
            .fetch_one(&self.database.pool)
            .await?;

        Ok(format!("Service metrics collector operational, monitoring {} services", service_count))
    }
}

// Helper functions

fn create_node_labels() -> HashMap<String, String> {
    let mut labels = HashMap::new();

    if let Ok(hostname) = std::env::var("HOSTNAME") {
        labels.insert("node".to_string(), hostname);
    } else if let Ok(hostname) = gethostname::gethostname().into_string() {
        labels.insert("node".to_string(), hostname);
    } else {
        labels.insert("node".to_string(), "localhost".to_string());
    }

    labels.insert("job".to_string(), "casvps".to_string());
    labels
}

async fn get_network_interfaces() -> Result<Vec<String>> {
    let mut interfaces = Vec::new();

    // Read network interfaces from /sys/class/net
    let mut entries = tokio::fs::read_dir("/sys/class/net").await?;
    while let Some(entry) = entries.next_entry().await? {
        let interface_name = entry.file_name().to_string_lossy().to_string();

        // Skip loopback and virtual interfaces for main monitoring
        if !interface_name.starts_with("lo")
            && !interface_name.starts_with("docker")
            && !interface_name.starts_with("br-")
            && !interface_name.starts_with("veth") {
            interfaces.push(interface_name);
        }
    }

    if interfaces.is_empty() {
        // Fallback: at least include eth0 and wlan0 if they might exist
        interfaces.extend_from_slice(&[
            "eth0".to_string(),
            "enp0s3".to_string(),
            "wlan0".to_string(),
        ]);
    }

    Ok(interfaces)
}

// Database row structures
#[derive(sqlx::FromRow)]
struct VmRow {
    vm_id: String,
    name: String,
    state: String,
    config: String,
}

#[derive(sqlx::FromRow)]
struct ContainerRow {
    container_id: String,
    name: String,
    state: String,
    image: String,
    config: String,
}

#[derive(sqlx::FromRow)]
struct MonitoredServiceRow {
    id: String,
    service_name: String,
    last_status: Option<String>,
    check_interval: i64,
    enabled: bool,
}

// Configuration structures
#[derive(Deserialize)]
struct VmConfig {
    memory_mb: Option<u64>,
    vcpus: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_node_labels() {
        let labels = create_node_labels();

        assert!(labels.contains_key("job"));
        assert_eq!(labels.get("job"), Some(&"casvps".to_string()));
        assert!(labels.contains_key("node"));
    }

    #[tokio::test]
    async fn test_system_metrics_collector_creation() {
        let collector = SystemMetricsCollector::new().await;
        assert!(collector.is_ok());
    }

    #[tokio::test]
    async fn test_network_metrics_collector_creation() {
        let collector = NetworkMetricsCollector::new().await;
        assert!(collector.is_ok());
    }

    #[test]
    fn test_vm_config_parsing() {
        let config_json = r#"{"memory_mb": 2048, "vcpus": 2}"#;
        let config: VmConfig = serde_json::from_str(config_json).unwrap();

        assert_eq!(config.memory_mb, Some(2048));
        assert_eq!(config.vcpus, Some(2));
    }
}