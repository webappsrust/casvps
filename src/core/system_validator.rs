use anyhow::Result;
use std::path::Path;
use tracing::{debug, warn};

pub struct SystemValidator;

impl SystemValidator {
    pub fn new() -> Self {
        Self
    }

    pub async fn validate_everything(&self) -> Result<()> {
        debug!("Running comprehensive system validation");

        // Validate network configuration
        self.validate_network_config()?;

        // Validate storage configuration
        self.validate_storage_config()?;

        // Validate service configurations
        self.validate_service_configs()?;

        // Validate permissions
        self.validate_permissions()?;

        // Validate dependencies
        self.validate_dependencies()?;

        // Validate no conflicts
        self.validate_no_conflicts()?;

        // Validate database integrity
        self.validate_database_integrity().await?;

        debug!("System validation completed successfully");
        Ok(())
    }

    fn validate_network_config(&self) -> Result<()> {
        // Check for network conflicts
        let interfaces = pnet::datalink::interfaces();

        for interface in interfaces {
            if interface.name.starts_with("casvps") {
                // Verify our managed interfaces
                if !interface.is_up() {
                    warn!("Managed interface {} is down", interface.name);
                }
            }
        }

        Ok(())
    }

    fn validate_storage_config(&self) -> Result<()> {
        // Check storage paths exist and are writable
        let paths = vec![
            "/var/lib/casvps",
            "/var/lib/casvps/instances",
            "/var/lib/casvps/storage",
        ];

        for path_str in paths {
            let path = Path::new(path_str);
            if !path.exists() {
                return Err(anyhow::anyhow!("Storage path {} does not exist", path_str));
            }

            // Check if writable
            let test_file = path.join(".write_test");
            match std::fs::write(&test_file, b"test") {
                Ok(_) => {
                    std::fs::remove_file(test_file).ok();
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Storage path {} is not writable: {}", path_str, e));
                }
            }
        }

        Ok(())
    }

    fn validate_service_configs(&self) -> Result<()> {
        // Validate generated service configuration files
        // This would check nginx.conf, postfix configs, etc.

        // Check nginx config if it exists
        if Path::new("/etc/nginx/nginx.conf").exists() {
            let output = std::process::Command::new("nginx")
                .args(&["-t"])
                .output()?;

            if !output.status.success() {
                return Err(anyhow::anyhow!("Invalid nginx configuration"));
            }
        }

        Ok(())
    }

    fn validate_permissions(&self) -> Result<()> {
        use std::os::unix::fs::MetadataExt;

        // Check critical file permissions
        let checks = vec![
            ("/var/lib/casvps/casvps.db", 0o600),
            ("/etc/casvps", 0o755),
        ];

        for (path_str, expected_mode) in checks {
            if let Ok(metadata) = std::fs::metadata(path_str) {
                let mode = metadata.mode() & 0o777;
                if mode != expected_mode {
                    warn!(
                        "Incorrect permissions on {}: {:o} (expected {:o})",
                        path_str, mode, expected_mode
                    );
                }
            }
        }

        Ok(())
    }

    fn validate_dependencies(&self) -> Result<()> {
        // Check for required system binaries
        let required_binaries = vec![
            "qemu-system-x86_64",
            "qemu-img",
            "nginx",
            "postfix",
        ];

        for binary in required_binaries {
            if which::which(binary).is_err() {
                warn!("Required binary {} not found in PATH", binary);
            }
        }

        Ok(())
    }

    fn validate_no_conflicts(&self) -> Result<()> {
        // Check for port conflicts
        let our_ports = vec![8006, 53, 67, 69];

        for port in our_ports {
            if self.is_port_in_use(port) {
                warn!("Port {} is already in use", port);
            }
        }

        Ok(())
    }

    async fn validate_database_integrity(&self) -> Result<()> {
        // This would be implemented when database module is ready
        Ok(())
    }

    fn is_port_in_use(&self, port: u16) -> bool {
        use std::net::TcpListener;

        TcpListener::bind(format!("0.0.0.0:{}", port)).is_err()
    }
}