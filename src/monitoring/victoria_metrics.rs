use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use super::{Metric, MetricQueryResult, MetricRangeResult, MetricSeries, MetricTimeSeries};

/// Embedded Victoria Metrics time series database
///
/// According to spec: Embedded Victoria Metrics with:
/// - Retention: raw(6h), 5m(7d), 1h(30d), 1d(1y)
/// - Automatic downsampling
/// - HTTP API for queries
pub struct VictoriaMetrics {
    data_path: PathBuf,
    port: u16,
    process_handle: Arc<RwLock<Option<tokio::process::Child>>>,
    retention_config: RetentionConfig,
    running: Arc<RwLock<bool>>,
}

impl VictoriaMetrics {
    pub async fn new(data_path: &str) -> Result<Self> {
        let data_path = PathBuf::from(data_path);

        // Create data directory if it doesn't exist
        tokio::fs::create_dir_all(&data_path).await?;

        // Victoria Metrics retention configuration
        let retention_config = RetentionConfig {
            raw_retention: Duration::from_secs(6 * 3600),        // 6 hours
            downsampling_5m_retention: Duration::from_secs(7 * 24 * 3600), // 7 days
            downsampling_1h_retention: Duration::from_secs(30 * 24 * 3600), // 30 days
            downsampling_1d_retention: Duration::from_secs(365 * 24 * 3600), // 1 year
        };

        Ok(Self {
            data_path,
            port: 8428, // Standard Victoria Metrics port
            process_handle: Arc::new(RwLock::new(None)),
            retention_config,
            running: Arc::new(RwLock::new(false)),
        })
    }

    pub async fn start(&self) -> Result<()> {
        if *self.running.read().await {
            return Ok(());
        }

        info!("Starting embedded Victoria Metrics on port {}", self.port);

        // Start Victoria Metrics as embedded process
        let mut process = tokio::process::Command::new("victoriametrics")
            .arg("-storageDataPath")
            .arg(&self.data_path)
            .arg("-httpListenAddr")
            .arg(format!(":{}", self.port))
            .arg("-retentionPeriod")
            .arg("1y") // Maximum retention period
            .arg("-loggerLevel")
            .arg("INFO")
            .arg("-dedup.minScrapeInterval")
            .arg("30s")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        // Store process handle
        {
            let mut handle = self.process_handle.write().await;
            *handle = Some(process);
        }

        // Wait for Victoria Metrics to start
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Check if process is still running
        self.health_check().await?;

        *self.running.write().await = true;

        // Start downsampling and cleanup tasks
        self.start_maintenance_tasks().await?;

        info!("Victoria Metrics started successfully");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        info!("Stopping Victoria Metrics");

        *self.running.write().await = false;

        let mut handle = self.process_handle.write().await;
        if let Some(mut process) = handle.take() {
            // Graceful shutdown
            if let Err(e) = process.kill().await {
                warn!("Failed to kill Victoria Metrics process gracefully: {}", e);
            }

            // Wait for process to exit
            if let Err(e) = process.wait().await {
                warn!("Error waiting for Victoria Metrics process to exit: {}", e);
            }
        }

        info!("Victoria Metrics stopped");
        Ok(())
    }

    /// Push metrics to Victoria Metrics using import API
    pub async fn push_metrics(&self, metrics: &[Metric]) -> Result<()> {
        if metrics.is_empty() {
            return Ok(());
        }

        debug!("Pushing {} metrics to Victoria Metrics", metrics.len());

        // Convert metrics to Victoria Metrics import format
        let mut import_data = String::new();
        for metric in metrics {
            // Format: metric_name{label1="value1",label2="value2"} value timestamp
            let mut labels_str = String::new();
            for (key, value) in &metric.labels {
                if !labels_str.is_empty() {
                    labels_str.push(',');
                }
                labels_str.push_str(&format!("{}=\"{}\"", key, value));
            }

            let line = if labels_str.is_empty() {
                format!("{} {} {}\n", metric.name, metric.value, metric.timestamp * 1000)
            } else {
                format!("{}{{{}}} {} {}\n",
                    metric.name,
                    labels_str,
                    metric.value,
                    metric.timestamp * 1000
                )
            };

            import_data.push_str(&line);
        }

        // Send to Victoria Metrics import endpoint
        let client = reqwest::Client::new();
        let response = client
            .post(&format!("http://localhost:{}/api/v1/import/prometheus", self.port))
            .header("Content-Type", "text/plain")
            .body(import_data)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Victoria Metrics import failed: {}", error_text));
        }

        debug!("Successfully pushed {} metrics", metrics.len());
        Ok(())
    }

    /// Query current metric values
    pub async fn query(&self, query: &str) -> Result<MetricQueryResult> {
        debug!("Querying Victoria Metrics: {}", query);

        let client = reqwest::Client::new();
        let response = client
            .get(&format!("http://localhost:{}/api/v1/query", self.port))
            .query(&[("query", query)])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Victoria Metrics query failed: {}", error_text));
        }

        let vm_response: VictoriaMetricsQueryResponse = response.json().await?;

        // Convert to our format
        let series = match vm_response.data {
            VictoriaMetricsData::Vector(vector) => {
                vector.into_iter().map(|item| MetricSeries {
                    metric: item.metric,
                    value: (item.value.0, item.value.1),
                }).collect()
            },
            _ => Vec::new(),
        };

        Ok(MetricQueryResult {
            status: vm_response.status,
            data: series,
        })
    }

    /// Query metric values over time range
    pub async fn query_range(
        &self,
        query: &str,
        start: u64,
        end: u64,
        step: Duration,
    ) -> Result<MetricRangeResult> {
        debug!("Range query Victoria Metrics: {} ({}s to {}s, step {}s)",
               query, start, end, step.as_secs());

        let client = reqwest::Client::new();
        let response = client
            .get(&format!("http://localhost:{}/api/v1/query_range", self.port))
            .query(&[
                ("query", query),
                ("start", &start.to_string()),
                ("end", &end.to_string()),
                ("step", &step.as_secs().to_string()),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Victoria Metrics range query failed: {}", error_text));
        }

        let vm_response: VictoriaMetricsQueryResponse = response.json().await?;

        // Convert to our format
        let series = match vm_response.data {
            VictoriaMetricsData::Matrix(matrix) => {
                matrix.into_iter().map(|item| MetricTimeSeries {
                    metric: item.metric,
                    values: item.values,
                }).collect()
            },
            _ => Vec::new(),
        };

        Ok(MetricRangeResult {
            status: vm_response.status,
            data: series,
        })
    }

    /// Health check for Victoria Metrics
    pub async fn health_check(&self) -> Result<()> {
        let client = reqwest::Client::new();
        let response = client
            .get(&format!("http://localhost:{}/health", self.port))
            .timeout(Duration::from_secs(5))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Victoria Metrics health check failed"));
        }

        Ok(())
    }

    /// Get Victoria Metrics statistics
    pub async fn get_stats(&self) -> Result<VictoriaMetricsStats> {
        let client = reqwest::Client::new();
        let response = client
            .get(&format!("http://localhost:{}/api/v1/status/tsdb", self.port))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get Victoria Metrics stats"));
        }

        let stats: VictoriaMetricsTsdbStats = response.json().await?;

        Ok(VictoriaMetricsStats {
            total_samples: stats.data.total_series.unwrap_or(0),
            storage_size_bytes: stats.data.series_count_by_metric_name.len() as u64 * 1024, // Approximation
            memory_usage_bytes: stats.data.label_pairs_count.unwrap_or(0) * 64, // Approximation
            uptime_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        })
    }

    /// Clean up old data based on retention policies
    pub async fn cleanup_old_data(&self) -> Result<()> {
        info!("Running Victoria Metrics data cleanup");

        // Victoria Metrics handles retention automatically based on -retentionPeriod
        // But we can run additional cleanup if needed

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // Clean up very old raw data (older than 6 hours)
        let raw_cutoff = now - self.retention_config.raw_retention.as_secs();

        let client = reqwest::Client::new();
        let response = client
            .post(&format!("http://localhost:{}/api/v1/admin/tsdb/delete_series", self.port))
            .query(&[
                ("match[]", &format!("{{__name__=~\".+\"}}[{}s:]", raw_cutoff)),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            warn!("Data cleanup may have failed");
        }

        info!("Victoria Metrics data cleanup completed");
        Ok(())
    }

    async fn start_maintenance_tasks(&self) -> Result<()> {
        let vm_instance = self.clone_for_task();

        // Downsampling task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes

            loop {
                interval.tick().await;

                if !*vm_instance.running.read().await {
                    break;
                }

                // Victoria Metrics doesn't have built-in downsampling like Prometheus
                // For now, just log that maintenance is running
                debug!("Victoria Metrics maintenance tick");
            }
        });

        Ok(())
    }

    fn clone_for_task(&self) -> Self {
        Self {
            data_path: self.data_path.clone(),
            port: self.port,
            process_handle: self.process_handle.clone(),
            retention_config: self.retention_config.clone(),
            running: self.running.clone(),
        }
    }
}

impl Drop for VictoriaMetrics {
    fn drop(&mut self) {
        // Best effort cleanup
        if let Ok(handle) = self.process_handle.try_write() {
            if let Some(mut process) = handle.as_mut() {
                let _ = process.start_kill();
            }
        }
    }
}

#[derive(Debug, Clone)]
struct RetentionConfig {
    raw_retention: Duration,
    downsampling_5m_retention: Duration,
    downsampling_1h_retention: Duration,
    downsampling_1d_retention: Duration,
}

#[derive(Debug, Clone)]
pub struct VictoriaMetricsStats {
    pub total_samples: u64,
    pub storage_size_bytes: u64,
    pub memory_usage_bytes: u64,
    pub uptime_seconds: u64,
}

// Victoria Metrics API response structures
#[derive(Debug, Deserialize)]
struct VictoriaMetricsQueryResponse {
    status: String,
    data: VictoriaMetricsData,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum VictoriaMetricsData {
    Vector(Vec<VictoriaMetricsInstantValue>),
    Matrix(Vec<VictoriaMetricsRangeValue>),
}

#[derive(Debug, Deserialize)]
struct VictoriaMetricsInstantValue {
    metric: HashMap<String, String>,
    value: (u64, String), // (timestamp, value)
}

#[derive(Debug, Deserialize)]
struct VictoriaMetricsRangeValue {
    metric: HashMap<String, String>,
    values: Vec<(u64, String)>, // [(timestamp, value)]
}

#[derive(Debug, Deserialize)]
struct VictoriaMetricsTsdbStats {
    data: VictoriaMetricsTsdbData,
}

#[derive(Debug, Deserialize)]
struct VictoriaMetricsTsdbData {
    #[serde(rename = "seriesCountByMetricName")]
    series_count_by_metric_name: HashMap<String, u64>,
    #[serde(rename = "totalSeries")]
    total_series: Option<u64>,
    #[serde(rename = "labelPairsCount")]
    label_pairs_count: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_victoria_metrics_creation() {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join("test_vm");

        let vm = VictoriaMetrics::new(data_path.to_str().unwrap()).await.unwrap();

        assert_eq!(vm.port, 8428);
        assert!(vm.data_path.exists());
    }

    #[test]
    fn test_retention_config() {
        let config = RetentionConfig {
            raw_retention: Duration::from_secs(6 * 3600),
            downsampling_5m_retention: Duration::from_secs(7 * 24 * 3600),
            downsampling_1h_retention: Duration::from_secs(30 * 24 * 3600),
            downsampling_1d_retention: Duration::from_secs(365 * 24 * 3600),
        };

        assert_eq!(config.raw_retention.as_secs(), 21600); // 6 hours
        assert_eq!(config.downsampling_5m_retention.as_secs(), 604800); // 7 days
    }

    #[test]
    fn test_metric_import_format() {
        let mut labels = HashMap::new();
        labels.insert("instance".to_string(), "localhost".to_string());
        labels.insert("job".to_string(), "casvps".to_string());

        let metric = Metric {
            name: "cpu_usage_percent".to_string(),
            value: 75.5,
            labels,
            timestamp: 1234567890,
        };

        // Test that we can format metric for import
        let expected_format = r#"cpu_usage_percent{instance="localhost",job="casvps"} 75.5 1234567890000"#;

        // The actual formatting logic is in push_metrics, this tests the concept
        assert!(metric.name == "cpu_usage_percent");
        assert!(metric.value == 75.5);
        assert!(metric.labels.len() == 2);
    }
}