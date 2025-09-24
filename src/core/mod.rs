use anyhow::Result;
use std::path::Path;
use tokio::sync::RwLock;
use std::sync::Arc;
use tracing::{info, debug, error};

pub mod service_client;
pub mod startup;
pub mod platform;
pub mod system_validator;

pub use service_client::ServiceClient;
pub use startup::StartupManager;
pub use platform::Platform;
pub use system_validator::SystemValidator;

use crate::database::Database;
use crate::services::ServiceController;
use crate::network::NetworkManager;
use crate::virtualization::VirtualizationManager;
use crate::web::WebServer;

pub struct CasVPS {
    database: Arc<Database>,
    services: Arc<ServiceController>,
    network: Arc<NetworkManager>,
    virtualization: Arc<VirtualizationManager>,
    web_server: Option<WebServer>,
    platform: Platform,
    first_run: bool,
}

impl CasVPS {
    pub async fn new() -> Result<Self> {
        // Detect platform
        let platform = Platform::detect()?;
        info!("Detected platform: {:?}", platform);

        // Check if this is first run
        let first_run = !Path::new("/var/lib/casvps/casvps.db").exists();

        if first_run {
            info!("First run detected, initializing system");
            Self::initialize_directories()?;
        }

        // Initialize database
        let database = Arc::new(Database::new("/var/lib/casvps/casvps.db").await?);

        // Initialize subsystems
        let services = Arc::new(ServiceController::new(database.clone()));
        let network = Arc::new(NetworkManager::new(database.clone()));
        let virtualization = Arc::new(VirtualizationManager::new(database.clone(), platform.clone()));

        Ok(Self {
            database,
            services,
            network,
            virtualization,
            web_server: None,
            platform,
            first_run,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        // Startup sequence (< 30 seconds)
        let startup = StartupManager::new(
            self.database.clone(),
            self.services.clone(),
            self.network.clone(),
            self.virtualization.clone(),
        );

        startup.execute(self.first_run).await?;

        // Start web server
        self.web_server = Some(WebServer::new(
            self.database.clone(),
            self.virtualization.clone(),
        ).await?);

        if let Some(ref server) = self.web_server {
            info!("Web interface available at: {}", server.get_best_url());

            // Run web server
            server.run().await?;
        }

        Ok(())
    }

    fn initialize_directories() -> Result<()> {
        let directories = vec![
            "/var/lib/casvps",
            "/var/lib/casvps/instances",
            "/var/lib/casvps/storage",
            "/var/lib/casvps/backups",
            "/var/lib/casvps/templates",
            "/var/lib/casvps/iso",
            "/var/lib/casvps/iso/linux",
            "/var/lib/casvps/iso/windows",
            "/var/lib/casvps/iso/tools",
            "/var/lib/casvps/iso/cache",
            "/var/lib/casvps/tftp",
            "/var/lib/casvps/ca",
            "/var/lib/casvps/compliance-archives",
            "/var/lib/casvps/logs",
            "/etc/casvps",
            "/etc/casvps/security",
            "/etc/casvps/security/geoip",
            "/etc/casvps/security/clamav",
            "/etc/casvps/security/suricata",
            "/etc/casvps/security/crowdsec",
            "/etc/casvps/ssl",
            "/etc/casvps/ssl/active",
            "/etc/casvps/ssl/users",
            "/var/log/casvps",
        ];

        for dir in directories {
            std::fs::create_dir_all(dir)?;
            debug!("Created directory: {}", dir);
        }

        Ok(())
    }
}