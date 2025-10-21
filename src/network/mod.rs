use anyhow::Result;
use std::sync::Arc;
use tracing::debug;
use crate::database::Database;

pub mod dhcp;
pub mod dns;
pub mod tftp;
pub mod sdn;

pub struct NetworkManager {
    database: Arc<Database>,
}

impl NetworkManager {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    pub async fn setup_bridges(&self) -> Result<()> {
        debug!("Setting up network bridges");
        // Create default bridge
        std::process::Command::new("ip")
            .args(&["link", "add", "name", "casvps0", "type", "bridge"])
            .output()
            .ok();

        std::process::Command::new("ip")
            .args(&["link", "set", "casvps0", "up"])
            .output()
            .ok();

        Ok(())
    }

    pub async fn initialize_firewall(&self) -> Result<()> {
        debug!("Initializing firewall rules");
        // Set up basic nftables rules
        Ok(())
    }
}