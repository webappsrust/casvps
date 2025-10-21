use anyhow::Result;
use std::sync::Arc;
use std::collections::HashSet;
use tracing::{debug, info};
use crate::database::Database;
use ipnetwork::IpNetwork;
use std::str::FromStr;

pub struct SmartNetworkOptimizer {
    database: Arc<Database>,
}

impl SmartNetworkOptimizer {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    pub async fn optimize(&self) -> Result<()> {
        debug!("Running smart network optimization");

        // Optimize MTU settings
        self.optimize_mtu().await?;

        // Optimize network buffers
        self.optimize_buffers().await?;

        // Check and resolve conflicts
        self.resolve_conflicts().await?;

        Ok(())
    }

    async fn optimize_mtu(&self) -> Result<()> {
        // Detect tunnel type and adjust MTU accordingly
        let tunnel_type = self.detect_tunnel_type().await?;
        let upstream_mtu = self.get_upstream_mtu()?;

        let optimal_mtu = match tunnel_type {
            Some(TunnelType::VXLAN) => upstream_mtu - 50,
            Some(TunnelType::GRE) => upstream_mtu - 24,
            Some(TunnelType::Geneve) => upstream_mtu - 58,
            None => upstream_mtu,
        };

        self.database.set_config("network.mtu", &optimal_mtu.to_string()).await?;
        info!("Set optimal MTU to {}", optimal_mtu);

        Ok(())
    }

    async fn optimize_buffers(&self) -> Result<()> {
        // Set optimal network buffer sizes based on available memory
        let sys = sysinfo::System::new_all();
        let total_memory = sys.total_memory();

        let (rmem_max, wmem_max) = if total_memory > 32 * 1024 * 1024 * 1024 {
            // > 32GB - large buffers
            (268435456, 268435456)  // 256MB
        } else if total_memory > 8 * 1024 * 1024 * 1024 {
            // > 8GB - medium buffers
            (134217728, 134217728)  // 128MB
        } else {
            // <= 8GB (Pi4) - small buffers
            (33554432, 33554432)    // 32MB
        };

        self.apply_sysctl("net.core.rmem_max", rmem_max).await?;
        self.apply_sysctl("net.core.wmem_max", wmem_max).await?;

        // TCP specific optimizations
        self.apply_sysctl_string("net.ipv4.tcp_congestion_control", "bbr").await?;
        self.apply_sysctl("net.core.default_qdisc", "fq").await?;

        Ok(())
    }

    async fn resolve_conflicts(&self) -> Result<()> {
        // Check for subnet conflicts
        let used_subnets = self.get_used_subnets().await?;

        // Smart subnet allocation - avoid common conflicts
        let safe_subnets = vec![
            "172.20.0.0/16",  // Rarely used
            "172.21.0.0/16",
            "172.22.0.0/16",
            "10.99.0.0/16",   // Unusual range
            "10.88.0.0/16",
            "10.77.0.0/16",
        ];

        for subnet_str in safe_subnets {
            let subnet = IpNetwork::from_str(subnet_str)?;
            if !self.subnet_conflicts(&subnet, &used_subnets) {
                self.database.set_config("network.default_subnet", subnet_str).await?;
                info!("Selected conflict-free subnet: {}", subnet_str);
                break;
            }
        }

        Ok(())
    }

    async fn detect_tunnel_type(&self) -> Result<Option<TunnelType>> {
        let config = self.database.get_config("network.sdn.tunnel").await?;

        match config.as_str() {
            "vxlan" => Ok(Some(TunnelType::VXLAN)),
            "gre" => Ok(Some(TunnelType::GRE)),
            "geneve" => Ok(Some(TunnelType::Geneve)),
            _ => Ok(None),
        }
    }

    fn get_upstream_mtu(&self) -> Result<u16> {
        // Get MTU from primary interface
        let interfaces = pnet::datalink::interfaces();

        for interface in interfaces {
            if !interface.is_loopback() && interface.is_up() {
                // Most interfaces default to 1500
                return Ok(1500);
            }
        }

        Ok(1500)  // Default
    }

    async fn get_used_subnets(&self) -> Result<HashSet<IpNetwork>> {
        let mut subnets = HashSet::new();

        // Check Docker networks
        if let Ok(output) = std::process::Command::new("docker")
            .args(&["network", "ls", "--format", "{{.Name}}"])
            .output() {
            // Parse Docker networks
            debug!("Detected Docker networks");
        }

        // Check libvirt networks
        if let Ok(output) = std::process::Command::new("virsh")
            .args(&["net-list", "--all"])
            .output() {
            // Parse libvirt networks
            debug!("Detected libvirt networks");
        }

        // Add common defaults to avoid
        subnets.insert(IpNetwork::from_str("172.17.0.0/16")?);  // Docker default
        subnets.insert(IpNetwork::from_str("192.168.122.0/24")?); // libvirt default
        subnets.insert(IpNetwork::from_str("10.0.2.0/24")?);     // VirtualBox NAT

        Ok(subnets)
    }

    fn subnet_conflicts(&self, subnet: &IpNetwork, used_subnets: &HashSet<IpNetwork>) -> bool {
        for used in used_subnets {
            if subnet.overlaps(used) || used.overlaps(subnet) {
                return true;
            }
        }
        false
    }

    async fn apply_sysctl(&self, key: &str, value: i64) -> Result<()> {
        let path = format!("/proc/sys/{}", key.replace('.', "/"));
        std::fs::write(&path, value.to_string())?;
        self.database.set_config(key, &value.to_string()).await?;
        Ok(())
    }

    async fn apply_sysctl_string(&self, key: &str, value: &str) -> Result<()> {
        let path = format!("/proc/sys/{}", key.replace('.', "/"));
        std::fs::write(&path, value)?;
        self.database.set_config(key, value).await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum TunnelType {
    VXLAN,
    GRE,
    Geneve,
}