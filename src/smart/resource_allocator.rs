use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, info};
use crate::database::Database;
use serde::{Deserialize, Serialize};

const GB: u64 = 1024 * 1024 * 1024;
const MB: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OSType {
    Windows(u32),  // Version number (10, 11)
    Linux(String), // Distribution name
    MacOS(String), // Version
    BSD(String),   // Variant
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Workload {
    Desktop,
    Server,
    Database,
    Container,
    Development,
    FileServer,
    WebServer,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryRequest {
    Auto,
    Fixed(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequest {
    pub os_type: OSType,
    pub workload: Workload,
    pub memory: MemoryRequest,
    pub cpu: Option<u32>,
    pub storage: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    pub memory: u64,
    pub cpu: u32,
    pub storage: u64,
    pub network_bandwidth: Option<u64>,
}

pub struct SmartResourceAllocator {
    database: Arc<Database>,
}

impl SmartResourceAllocator {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    pub async fn allocate(&self, request: &ResourceRequest) -> Result<ResourceAllocation> {
        let available = self.get_available_resources().await?;

        // Smart memory allocation
        let memory = self.calculate_smart_memory(&request, &available)?;

        // Smart CPU allocation
        let cpu = self.calculate_smart_cpu(&request, &available)?;

        // Smart storage allocation
        let storage = self.calculate_smart_storage(&request, &available)?;

        // Smart network bandwidth (if SDN enabled)
        let network_bandwidth = self.calculate_network_bandwidth(&request, &available).await.ok();

        Ok(ResourceAllocation {
            memory,
            cpu,
            storage,
            network_bandwidth,
        })
    }

    fn calculate_smart_memory(&self, request: &ResourceRequest, available: &AvailableResources) -> Result<u64> {
        let memory = match request.memory {
            MemoryRequest::Auto => {
                match (&request.os_type, &request.workload) {
                    // Windows Desktop
                    (OSType::Windows(11), Workload::Desktop) => {
                        u64::min(available.memory / 4, 4 * GB)
                    }
                    (OSType::Windows(10), Workload::Desktop) => {
                        u64::min(available.memory / 6, 2 * GB)
                    }

                    // Windows Server
                    (OSType::Windows(_), Workload::Server) => {
                        u64::min(available.memory / 3, 8 * GB)
                    }

                    // Linux variations
                    (OSType::Linux(_), Workload::Container) => 512 * MB,
                    (OSType::Linux(_), Workload::Database) => {
                        u64::min(available.memory * 70 / 100, 32 * GB)
                    }
                    (OSType::Linux(_), Workload::WebServer) => {
                        u64::min(available.memory / 8, 2 * GB)
                    }
                    (OSType::Linux(_), Workload::Development) => {
                        u64::min(available.memory / 4, 4 * GB)
                    }
                    (OSType::Linux(_), Workload::FileServer) => {
                        u64::min(available.memory / 10, 1 * GB)
                    }

                    // macOS
                    (OSType::MacOS(_), _) => {
                        u64::min(available.memory / 2, 8 * GB)
                    }

                    // BSD
                    (OSType::BSD(_), _) => {
                        u64::min(available.memory / 6, 2 * GB)
                    }

                    // Default fallback
                    _ => u64::min(available.memory / 10, 512 * MB),
                }
            }
            MemoryRequest::Fixed(size) => {
                // Smart validation - never allow more than 90% of available
                if size > available.memory * 90 / 100 {
                    info!("Requested memory {} exceeds safe limit, adjusting to 80%", size);
                    available.memory * 80 / 100
                } else {
                    u64::max(size, 128 * MB)  // Minimum viable memory
                }
            }
        };

        debug!("Smart memory allocation: {} MB", memory / MB);
        Ok(memory)
    }

    fn calculate_smart_cpu(&self, request: &ResourceRequest, available: &AvailableResources) -> Result<u32> {
        let cpu = match request.cpu {
            Some(requested) => {
                // Never overcommit more than 4x on Pi4, 8x on homelab, 16x on enterprise
                let max_overcommit = match available.total_cores {
                    1..=4 => 4,     // Pi4
                    5..=16 => 8,    // Homelab
                    _ => 16,        // Enterprise
                };

                let max_vcpus = available.total_cores * max_overcommit;
                u32::min(requested, max_vcpus)
            }
            None => {
                // Auto-allocate based on workload
                match &request.workload {
                    Workload::Database => u32::min(available.total_cores / 2, 8),
                    Workload::Server => u32::min(available.total_cores / 4, 4),
                    Workload::Desktop => u32::min(2, available.total_cores),
                    Workload::Container => 1,
                    _ => u32::min(available.total_cores / 8, 2),
                }
            }
        };

        debug!("Smart CPU allocation: {} cores", cpu);
        Ok(cpu)
    }

    fn calculate_smart_storage(&self, request: &ResourceRequest, available: &AvailableResources) -> Result<u64> {
        let storage = match request.storage {
            Some(requested) => requested,
            None => {
                // Auto-allocate based on OS and workload
                match (&request.os_type, &request.workload) {
                    (OSType::Windows(11), _) => 64 * GB,
                    (OSType::Windows(10), _) => 32 * GB,
                    (OSType::MacOS(_), _) => 64 * GB,
                    (OSType::Linux(_), Workload::Database) => 100 * GB,
                    (OSType::Linux(_), Workload::FileServer) => 500 * GB,
                    (OSType::Linux(_), Workload::Container) => 10 * GB,
                    _ => 20 * GB,
                }
            }
        };

        // Allow thin provisioning up to 200% of physical storage
        let max_storage = available.storage * 2;
        Ok(u64::min(storage, max_storage))
    }

    async fn calculate_network_bandwidth(&self, request: &ResourceRequest, _available: &AvailableResources) -> Result<u64> {
        // Smart network bandwidth allocation
        let bandwidth = match &request.workload {
            Workload::WebServer => 100 * 1024 * 1024,  // 100 Mbps
            Workload::FileServer => 1000 * 1024 * 1024, // 1 Gbps
            Workload::Database => 500 * 1024 * 1024,    // 500 Mbps
            _ => 10 * 1024 * 1024,                      // 10 Mbps default
        };

        Ok(bandwidth)
    }

    async fn get_available_resources(&self) -> Result<AvailableResources> {
        let sys = sysinfo::System::new_all();

        Ok(AvailableResources {
            memory: sys.available_memory(),
            total_cores: sys.cpus().len() as u32,
            storage: self.get_available_storage().await?,
        })
    }

    async fn get_available_storage(&self) -> Result<u64> {
        // Get available storage from primary pool
        // For now, return a reasonable default
        Ok(100 * GB)
    }

    pub async fn balloon_vms(&self, reduction_factor: f64) -> Result<()> {
        info!("Ballooning VMs by {}%", reduction_factor * 100.0);
        // Implementation would interact with QEMU balloon driver
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct AvailableResources {
    memory: u64,
    total_cores: u32,
    storage: u64,
}