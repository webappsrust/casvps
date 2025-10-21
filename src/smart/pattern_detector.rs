use anyhow::Result;
use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use crate::database::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityEvent {
    SSHAuthFail {
        source_ip: String,
        username: String,
        timestamp: Instant,
    },
    ConnectionAttempt {
        source_ip: String,
        dest_port: u16,
        timestamp: Instant,
    },
    HTTPRequest {
        source_ip: String,
        path: String,
        timestamp: Instant,
    },
}

#[derive(Debug, Clone)]
pub enum AttackType {
    SSHBruteForce,
    PortScan,
    DDoS,
    SQLInjection,
    DirectoryTraversal,
}

#[derive(Debug, Clone)]
pub enum PerformancePattern {
    Normal,
    MemoryLeak,
    IOBottleneck,
    CPUThrottling,
    NetworkCongestion,
}

pub struct PatternDetector {
    database: Arc<Database>,
    security_events: VecDeque<SecurityEvent>,
    performance_metrics: VecDeque<Metric>,
}

#[derive(Debug, Clone)]
pub struct Metric {
    timestamp: Instant,
    cpu_usage: f64,
    memory_used: u64,
    io_wait: f64,
    network_throughput: u64,
}

impl PatternDetector {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            security_events: VecDeque::with_capacity(10000),
            performance_metrics: VecDeque::with_capacity(1000),
        }
    }

    pub fn detect_attack_pattern(&self) -> Option<AttackType> {
        // SSH brute force detection
        if self.detect_ssh_brute_force() {
            return Some(AttackType::SSHBruteForce);
        }

        // Port scan detection
        if self.detect_port_scan() {
            return Some(AttackType::PortScan);
        }

        // DDoS detection
        if self.detect_ddos() {
            return Some(AttackType::DDoS);
        }

        None
    }

    pub fn detect_performance_pattern(&self) -> PerformancePattern {
        // Memory leak detection
        if self.detect_memory_leak() {
            return PerformancePattern::MemoryLeak;
        }

        // I/O bottleneck detection
        if self.detect_io_bottleneck() {
            return PerformancePattern::IOBottleneck;
        }

        // CPU throttling detection
        if self.detect_cpu_throttling() {
            return PerformancePattern::CPUThrottling;
        }

        // Network congestion detection
        if self.detect_network_congestion() {
            return PerformancePattern::NetworkCongestion;
        }

        PerformancePattern::Normal
    }

    fn detect_ssh_brute_force(&self) -> bool {
        let mut ip_attempts: HashMap<String, Vec<Instant>> = HashMap::new();

        for event in &self.security_events {
            if let SecurityEvent::SSHAuthFail { source_ip, timestamp, .. } = event {
                ip_attempts.entry(source_ip.clone())
                    .or_insert_with(Vec::new)
                    .push(*timestamp);
            }
        }

        // Check for rapid attempts from same IP
        for (ip, attempts) in ip_attempts {
            if attempts.len() > 5 {
                let time_span = attempts.last().unwrap().duration_since(*attempts.first().unwrap());
                if time_span < Duration::from_secs(60) {
                    warn!("SSH brute force detected from {}: {} attempts in {:?}",
                          ip, attempts.len(), time_span);
                    return true;
                }
            }
        }

        false
    }

    fn detect_port_scan(&self) -> bool {
        let mut ip_ports: HashMap<String, std::collections::HashSet<u16>> = HashMap::new();

        for event in &self.security_events {
            if let SecurityEvent::ConnectionAttempt { source_ip, dest_port, .. } = event {
                ip_ports.entry(source_ip.clone())
                    .or_insert_with(std::collections::HashSet::new)
                    .insert(*dest_port);
            }
        }

        // Check for connections to many different ports
        for (ip, ports) in ip_ports {
            if ports.len() > 20 {
                warn!("Port scan detected from {}: {} unique ports", ip, ports.len());
                return true;
            }
        }

        false
    }

    fn detect_ddos(&self) -> bool {
        let recent_cutoff = Instant::now() - Duration::from_secs(10);
        let recent_count = self.security_events.iter()
            .filter(|e| {
                match e {
                    SecurityEvent::HTTPRequest { timestamp, .. } |
                    SecurityEvent::ConnectionAttempt { timestamp, .. } => {
                        *timestamp > recent_cutoff
                    }
                    _ => false
                }
            })
            .count();

        if recent_count > 1000 {
            warn!("Potential DDoS detected: {} requests in 10 seconds", recent_count);
            return true;
        }

        false
    }

    fn detect_memory_leak(&self) -> bool {
        if self.performance_metrics.len() < 10 {
            return false;
        }

        // Calculate memory trend over last 10 samples
        let recent: Vec<_> = self.performance_metrics.iter()
            .rev()
            .take(10)
            .map(|m| m.memory_used as f64)
            .collect();

        let trend = self.calculate_trend(&recent);

        // Positive slope with high correlation indicates leak
        if trend.slope > 0.01 && trend.r_squared > 0.8 {
            warn!("Memory leak detected: slope={:.4}, r²={:.4}", trend.slope, trend.r_squared);
            return true;
        }

        false
    }

    fn detect_io_bottleneck(&self) -> bool {
        if self.performance_metrics.is_empty() {
            return false;
        }

        let avg_io_wait = self.performance_metrics.iter()
            .map(|m| m.io_wait)
            .sum::<f64>() / self.performance_metrics.len() as f64;

        if avg_io_wait > 30.0 {
            warn!("I/O bottleneck detected: average wait {:.1}%", avg_io_wait);
            return true;
        }

        false
    }

    fn detect_cpu_throttling(&self) -> bool {
        // Check for thermal throttling patterns
        if self.performance_metrics.len() < 5 {
            return false;
        }

        let recent: Vec<_> = self.performance_metrics.iter()
            .rev()
            .take(5)
            .map(|m| m.cpu_usage)
            .collect();

        // Look for sudden drops in CPU usage
        for window in recent.windows(2) {
            if window[0] - window[1] > 50.0 {
                info!("CPU throttling detected: dropped from {:.1}% to {:.1}%",
                      window[0], window[1]);
                return true;
            }
        }

        false
    }

    fn detect_network_congestion(&self) -> bool {
        if self.performance_metrics.len() < 5 {
            return false;
        }

        let recent: Vec<_> = self.performance_metrics.iter()
            .rev()
            .take(5)
            .map(|m| m.network_throughput)
            .collect();

        // Check for significant drops in throughput
        if let (Some(&max), Some(&min)) = (recent.iter().max(), recent.iter().min()) {
            if max > 0 && min as f64 / (max as f64) < 0.5 {
                info!("Network congestion detected: throughput dropped by >50%");
                return true;
            }
        }

        false
    }

    fn calculate_trend(&self, values: &[f64]) -> Trend {
        let n = values.len() as f64;
        if n < 2.0 {
            return Trend { slope: 0.0, r_squared: 0.0 };
        }

        // Simple linear regression
        let x_mean = (n - 1.0) / 2.0;
        let y_mean = values.iter().sum::<f64>() / n;

        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for (i, &y) in values.iter().enumerate() {
            let x = i as f64;
            numerator += (x - x_mean) * (y - y_mean);
            denominator += (x - x_mean) * (x - x_mean);
        }

        let slope = if denominator != 0.0 {
            numerator / denominator
        } else {
            0.0
        };

        // Calculate R²
        let mut ss_res = 0.0;
        let mut ss_tot = 0.0;

        for (i, &y) in values.iter().enumerate() {
            let x = i as f64;
            let y_pred = slope * (x - x_mean) + y_mean;
            ss_res += (y - y_pred) * (y - y_pred);
            ss_tot += (y - y_mean) * (y - y_mean);
        }

        let r_squared = if ss_tot != 0.0 {
            1.0 - (ss_res / ss_tot)
        } else {
            0.0
        };

        Trend { slope, r_squared }
    }

    pub fn add_security_event(&mut self, event: SecurityEvent) {
        // Keep only last 10000 events
        if self.security_events.len() >= 10000 {
            self.security_events.pop_front();
        }
        self.security_events.push_back(event);
    }

    pub fn add_performance_metric(&mut self, metric: Metric) {
        // Keep only last 1000 metrics
        if self.performance_metrics.len() >= 1000 {
            self.performance_metrics.pop_front();
        }
        self.performance_metrics.push_back(metric);
    }
}

#[derive(Debug, Clone)]
struct Trend {
    slope: f64,
    r_squared: f64,
}