use anyhow::Result;
use std::sync::Arc;
use std::collections::HashMap;
use std::process::Command;
use tracing::{debug, info, warn};
use crate::database::{Database, models::User};
use ipnetwork::IpNetwork;
use std::str::FromStr;

pub struct SDNManager {
    database: Arc<Database>,
    user_networks: HashMap<String, UserNetwork>,
    next_vlan: u16,
}

#[derive(Debug, Clone)]
struct UserNetwork {
    user_id: String,
    network_id: String,
    subnet: IpNetwork,
    vlan_id: u16,
    bridge_name: String,
    domain: String,
}

impl SDNManager {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            user_networks: HashMap::new(),
            next_vlan: 100, // Start VLANs from 100
        }
    }

    pub async fn create_user_network(&mut self, user: &User) -> Result<UserNetwork> {
        info!("Creating isolated network for user {}", user.username);

        // Allocate subnet and VLAN
        let subnet = self.allocate_subnet()?;
        let vlan_id = self.allocate_vlan();
        let network_id = format!("net-{}-{:03}", user.username, vlan_id);
        let bridge_name = format!("br-{}", vlan_id);
        let domain = format!("{}.casvps.local", user.username);

        // Create Linux bridge
        self.create_bridge(&bridge_name, vlan_id).await?;

        // Set up VLAN
        self.setup_vlan(vlan_id, &bridge_name).await?;

        // Configure iptables isolation
        self.setup_firewall_isolation(&subnet, vlan_id).await?;

        // Create NAT rules
        self.setup_nat(&subnet, &bridge_name).await?;

        let user_network = UserNetwork {
            user_id: user.user_id.clone(),
            network_id: network_id.clone(),
            subnet,
            vlan_id,
            bridge_name,
            domain,
        };

        // Save to database
        self.save_network_to_db(&user_network).await?;

        // Cache locally
        self.user_networks.insert(user.user_id.clone(), user_network.clone());

        info!("Created network {} for user {} with subnet {} and VLAN {}",
              network_id, user.username, subnet, vlan_id);

        Ok(user_network)
    }

    pub async fn delete_user_network(&mut self, user_id: &str) -> Result<()> {
        if let Some(network) = self.user_networks.remove(user_id) {
            info!("Deleting network {} for user {}", network.network_id, user_id);

            // Clean up NAT rules
            self.cleanup_nat(&network.subnet, &network.bridge_name).await?;

            // Clean up firewall rules
            self.cleanup_firewall_isolation(&network.subnet, network.vlan_id).await?;

            // Delete bridge
            self.delete_bridge(&network.bridge_name).await?;

            // Remove from database
            self.delete_network_from_db(&network.network_id).await?;
        }

        Ok(())
    }

    fn allocate_subnet(&self) -> Result<IpNetwork> {
        // Use 172.16.x.0/24 subnets to avoid conflicts
        for third_octet in 1..=254 {
            let subnet_str = format!("172.16.{}.0/24", third_octet);
            let subnet = IpNetwork::from_str(&subnet_str)?;

            // Check if subnet is already in use
            let in_use = self.user_networks.values()
                .any(|net| net.subnet.overlaps(&subnet));

            if !in_use {
                return Ok(subnet);
            }
        }

        Err(anyhow::anyhow!("No available subnets for user networks"))
    }

    fn allocate_vlan(&mut self) -> u16 {
        let vlan = self.next_vlan;
        self.next_vlan += 1;

        // Skip VLANs that are already in use
        while self.user_networks.values().any(|net| net.vlan_id == self.next_vlan) {
            self.next_vlan += 1;
            if self.next_vlan > 4094 {
                self.next_vlan = 100; // Wrap around
                break;
            }
        }

        vlan
    }

    async fn create_bridge(&self, bridge_name: &str, vlan_id: u16) -> Result<()> {
        debug!("Creating bridge {} for VLAN {}", bridge_name, vlan_id);

        // Create bridge
        Command::new("ip")
            .args(&["link", "add", "name", bridge_name, "type", "bridge"])
            .output()?;

        // Set bridge up
        Command::new("ip")
            .args(&["link", "set", bridge_name, "up"])
            .output()?;

        // Enable STP (Spanning Tree Protocol)
        std::fs::write(
            format!("/sys/class/net/{}/bridge/stp_state", bridge_name),
            "1"
        ).ok();

        Ok(())
    }

    async fn setup_vlan(&self, vlan_id: u16, bridge_name: &str) -> Result<()> {
        debug!("Setting up VLAN {} on bridge {}", vlan_id, bridge_name);

        let vlan_interface = format!("{}.{}", bridge_name, vlan_id);

        // Create VLAN interface
        Command::new("ip")
            .args(&["link", "add", "link", bridge_name, "name", &vlan_interface,
                    "type", "vlan", "id", &vlan_id.to_string()])
            .output()?;

        // Set VLAN interface up
        Command::new("ip")
            .args(&["link", "set", &vlan_interface, "up"])
            .output()?;

        Ok(())
    }

    async fn setup_firewall_isolation(&self, subnet: &IpNetwork, vlan_id: u16) -> Result<()> {
        debug!("Setting up firewall isolation for subnet {} VLAN {}", subnet, vlan_id);

        // Create nftables rules for isolation
        let table_name = format!("casvps_vlan_{}", vlan_id);

        // Create table
        Command::new("nft")
            .args(&["add", "table", "inet", &table_name])
            .output()?;

        // Block inter-VLAN communication (except to gateway)
        Command::new("nft")
            .args(&[
                "add", "chain", "inet", &table_name, "forward",
                "{ type filter hook forward priority 0; policy accept; }"
            ])
            .output()?;

        // Block traffic to other user networks
        for other_network in self.user_networks.values() {
            if other_network.vlan_id != vlan_id {
                Command::new("nft")
                    .args(&[
                        "add", "rule", "inet", &table_name, "forward",
                        "ip", "saddr", &subnet.to_string(),
                        "ip", "daddr", &other_network.subnet.to_string(),
                        "drop"
                    ])
                    .output()?;
            }
        }

        Ok(())
    }

    async fn setup_nat(&self, subnet: &IpNetwork, bridge_name: &str) -> Result<()> {
        debug!("Setting up NAT for subnet {} bridge {}", subnet, bridge_name);

        // Enable IP forwarding
        std::fs::write("/proc/sys/net/ipv4/ip_forward", "1")?;

        // Add MASQUERADE rule for this subnet
        Command::new("iptables")
            .args(&[
                "-t", "nat",
                "-A", "POSTROUTING",
                "-s", &subnet.to_string(),
                "!", "-d", &subnet.to_string(),
                "-j", "MASQUERADE"
            ])
            .output()?;

        // Allow forwarding for this subnet
        Command::new("iptables")
            .args(&[
                "-A", "FORWARD",
                "-i", bridge_name,
                "-j", "ACCEPT"
            ])
            .output()?;

        Command::new("iptables")
            .args(&[
                "-A", "FORWARD",
                "-o", bridge_name,
                "-j", "ACCEPT"
            ])
            .output()?;

        Ok(())
    }

    async fn cleanup_nat(&self, subnet: &IpNetwork, bridge_name: &str) -> Result<()> {
        debug!("Cleaning up NAT for subnet {} bridge {}", subnet, bridge_name);

        // Remove MASQUERADE rule
        Command::new("iptables")
            .args(&[
                "-t", "nat",
                "-D", "POSTROUTING",
                "-s", &subnet.to_string(),
                "!", "-d", &subnet.to_string(),
                "-j", "MASQUERADE"
            ])
            .output().ok();

        // Remove forwarding rules
        Command::new("iptables")
            .args(&[
                "-D", "FORWARD",
                "-i", bridge_name,
                "-j", "ACCEPT"
            ])
            .output().ok();

        Command::new("iptables")
            .args(&[
                "-D", "FORWARD",
                "-o", bridge_name,
                "-j", "ACCEPT"
            ])
            .output().ok();

        Ok(())
    }

    async fn cleanup_firewall_isolation(&self, subnet: &IpNetwork, vlan_id: u16) -> Result<()> {
        debug!("Cleaning up firewall isolation for subnet {} VLAN {}", subnet, vlan_id);

        let table_name = format!("casvps_vlan_{}", vlan_id);

        // Delete entire nftables table
        Command::new("nft")
            .args(&["delete", "table", "inet", &table_name])
            .output().ok();

        Ok(())
    }

    async fn delete_bridge(&self, bridge_name: &str) -> Result<()> {
        debug!("Deleting bridge {}", bridge_name);

        // Set bridge down
        Command::new("ip")
            .args(&["link", "set", bridge_name, "down"])
            .output().ok();

        // Delete bridge
        Command::new("ip")
            .args(&["link", "delete", bridge_name])
            .output().ok();

        Ok(())
    }

    async fn save_network_to_db(&self, network: &UserNetwork) -> Result<()> {
        sqlx::query(
            "INSERT INTO user_networks (network_id, user_id, subnet, vlan_id, domain, created_at)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)"
        )
        .bind(&network.network_id)
        .bind(&network.user_id)
        .bind(network.subnet.to_string())
        .bind(network.vlan_id as i32)
        .bind(&network.domain)
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn delete_network_from_db(&self, network_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM user_networks WHERE network_id = ?")
            .bind(network_id)
            .execute(&self.database.pool)
            .await?;

        Ok(())
    }

    pub async fn load_existing_networks(&mut self) -> Result<()> {
        let rows = sqlx::query(
            "SELECT network_id, user_id, subnet, vlan_id, domain FROM user_networks"
        )
        .fetch_all(&self.database.pool)
        .await?;

        for row in rows {
            let network_id: String = row.get("network_id");
            let user_id: String = row.get("user_id");
            let subnet_str: String = row.get("subnet");
            let vlan_id: i32 = row.get("vlan_id");
            let domain: String = row.get("domain");

            let subnet = IpNetwork::from_str(&subnet_str)?;
            let bridge_name = format!("br-{}", vlan_id);

            let network = UserNetwork {
                user_id: user_id.clone(),
                network_id,
                subnet,
                vlan_id: vlan_id as u16,
                bridge_name,
                domain,
            };

            self.user_networks.insert(user_id, network);

            // Update next VLAN counter
            if vlan_id as u16 >= self.next_vlan {
                self.next_vlan = vlan_id as u16 + 1;
            }
        }

        info!("Loaded {} existing user networks", self.user_networks.len());
        Ok(())
    }

    pub fn get_user_network(&self, user_id: &str) -> Option<&UserNetwork> {
        self.user_networks.get(user_id)
    }

    pub async fn attach_vm_to_network(&self, vm_id: &str, user_id: &str) -> Result<()> {
        if let Some(network) = self.user_networks.get(user_id) {
            debug!("Attaching VM {} to user network {}", vm_id, network.network_id);

            // Add VM's tap interface to the user's bridge
            let tap_interface = format!("tap{}", &vm_id[..8]);

            Command::new("ip")
                .args(&["link", "set", &tap_interface, "master", &network.bridge_name])
                .output()?;

            info!("Attached VM {} to network {} (bridge {})",
                  vm_id, network.network_id, network.bridge_name);
        }

        Ok(())
    }

    pub async fn detach_vm_from_network(&self, vm_id: &str, user_id: &str) -> Result<()> {
        if let Some(_network) = self.user_networks.get(user_id) {
            debug!("Detaching VM {} from user network", vm_id);

            let tap_interface = format!("tap{}", &vm_id[..8]);

            Command::new("ip")
                .args(&["link", "set", &tap_interface, "nomaster"])
                .output()?;
        }

        Ok(())
    }
}