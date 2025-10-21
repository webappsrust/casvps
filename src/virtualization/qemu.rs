use anyhow::Result;
use std::process::{Command, Stdio};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QemuVM {
    pub id: String,
    pub name: String,
    pub memory: u64,  // in bytes
    pub cpus: u32,
    pub disk_path: String,
    pub network: NetworkConfig,
    pub display: DisplayConfig,
    pub firmware: FirmwareType,
    pub machine_type: String,
    pub vnc_port: Option<u16>,
    pub spice_port: Option<u16>,
    pub monitor_socket: String,
    pub pid_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub bridge: String,
    pub mac: Option<String>,
    pub model: String,  // virtio-net, e1000, rtl8139
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub r#type: DisplayType,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisplayType {
    VNC,
    Spice,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FirmwareType {
    SeaBIOS,
    OVMF,  // UEFI
    Custom(String),
}

impl QemuVM {
    pub fn new(name: String, memory: u64, cpus: u32) -> Self {
        let id = Uuid::new_v4().to_string();

        Self {
            id: id.clone(),
            name,
            memory,
            cpus,
            disk_path: format!("/var/lib/casvps/instances/{}.qcow2", id),
            network: NetworkConfig {
                bridge: "casvps0".to_string(),
                mac: None,
                model: "virtio-net".to_string(),
            },
            display: DisplayConfig {
                r#type: DisplayType::VNC,
                port: None,
            },
            firmware: FirmwareType::SeaBIOS,
            machine_type: Self::detect_machine_type(),
            vnc_port: None,
            spice_port: None,
            monitor_socket: format!("/var/lib/casvps/instances/{}.monitor", id),
            pid_file: format!("/var/lib/casvps/instances/{}.pid", id),
        }
    }

    fn detect_machine_type() -> String {
        match std::env::consts::ARCH {
            "x86_64" => "pc-q35-6.2".to_string(),
            "aarch64" => "virt".to_string(),
            _ => "pc".to_string(),
        }
    }

    pub fn create_disk(&self, size_gb: u32) -> Result<()> {
        info!("Creating disk {} with size {}GB", self.disk_path, size_gb);

        let output = Command::new("qemu-img")
            .args(&[
                "create",
                "-f", "qcow2",
                "-o", "preallocation=metadata,lazy_refcounts=on,cluster_size=2M",
                &self.disk_path,
                &format!("{}G", size_gb),
            ])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to create disk: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        Ok(())
    }

    pub fn start(&mut self) -> Result<()> {
        info!("Starting VM {}", self.name);

        // Allocate ports if needed
        if self.display.port.is_none() {
            self.display.port = Some(self.allocate_display_port()?);
        }

        let mut args = self.build_qemu_args()?;

        debug!("QEMU command: qemu-system-x86_64 {}", args.join(" "));

        let child = Command::new(self.get_qemu_binary())
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        // Save PID
        std::fs::write(&self.pid_file, child.id().to_string())?;

        info!("VM {} started with PID {}", self.name, child.id());
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        info!("Stopping VM {}", self.name);

        // Send powerdown via monitor
        self.send_monitor_command("system_powerdown")?;

        // Wait for graceful shutdown (max 60 seconds)
        for _ in 0..60 {
            if !self.is_running()? {
                info!("VM {} stopped gracefully", self.name);
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        // Force kill if still running
        warn!("VM {} did not stop gracefully, force killing", self.name);
        self.force_kill()?;

        Ok(())
    }

    pub fn force_kill(&self) -> Result<()> {
        if let Ok(pid) = std::fs::read_to_string(&self.pid_file) {
            Command::new("kill")
                .args(&["-9", &pid.trim()])
                .output()?;
        }

        // Clean up PID file
        std::fs::remove_file(&self.pid_file).ok();

        Ok(())
    }

    pub fn is_running(&self) -> Result<bool> {
        if !std::path::Path::new(&self.pid_file).exists() {
            return Ok(false);
        }

        if let Ok(pid) = std::fs::read_to_string(&self.pid_file) {
            // Check if process exists
            let output = Command::new("kill")
                .args(&["-0", &pid.trim()])
                .output()?;

            Ok(output.status.success())
        } else {
            Ok(false)
        }
    }

    fn build_qemu_args(&self) -> Result<Vec<String>> {
        let mut args = Vec::new();

        // Machine type
        args.extend(&["-machine".to_string(), self.machine_type.clone()]);

        // Enable KVM if available
        if self.kvm_available() {
            args.push("-enable-kvm".to_string());
            args.extend(&["-cpu".to_string(), "host".to_string()]);
        } else {
            args.extend(&["-cpu".to_string(), "max".to_string()]);
        }

        // Memory
        args.extend(&["-m".to_string(), format!("{}", self.memory / (1024 * 1024))]);

        // CPUs
        args.extend(&["-smp".to_string(), format!("{}", self.cpus)]);

        // Disk
        args.extend(&[
            "-drive".to_string(),
            format!("file={},format=qcow2,if=virtio,cache=writeback,discard=unmap",
                    self.disk_path),
        ]);

        // Network
        let mac = self.network.mac.as_ref()
            .map(|m| format!(",mac={}", m))
            .unwrap_or_default();

        args.extend(&[
            "-netdev".to_string(),
            format!("bridge,id=net0,br={}", self.network.bridge),
            "-device".to_string(),
            format!("{},netdev=net0{}", self.network.model, mac),
        ]);

        // Display
        match self.display.r#type {
            DisplayType::VNC => {
                let port = self.display.port.unwrap_or(5900);
                args.extend(&["-vnc".to_string(), format!(":{}", port - 5900)]);
            }
            DisplayType::Spice => {
                let port = self.display.port.unwrap_or(5930);
                args.extend(&[
                    "-spice".to_string(),
                    format!("port={},disable-ticketing", port),
                    "-device".to_string(),
                    "virtio-serial".to_string(),
                    "-device".to_string(),
                    "virtserialport,chardev=spicechannel0,name=com.redhat.spice.0".to_string(),
                    "-chardev".to_string(),
                    "spicevmc,id=spicechannel0,name=vdagent".to_string(),
                ]);
            }
            DisplayType::None => {
                args.push("-nographic".to_string());
            }
        }

        // Firmware
        match &self.firmware {
            FirmwareType::OVMF => {
                args.extend(&[
                    "-drive".to_string(),
                    "if=pflash,format=raw,readonly,file=/usr/share/OVMF/OVMF_CODE.fd".to_string(),
                ]);
            }
            FirmwareType::Custom(path) => {
                args.extend(&["-bios".to_string(), path.clone()]);
            }
            FirmwareType::SeaBIOS => {
                // Default, no args needed
            }
        }

        // Monitor
        args.extend(&[
            "-monitor".to_string(),
            format!("unix:{},server,nowait", self.monitor_socket),
        ]);

        // PID file
        args.extend(&["-pidfile".to_string(), self.pid_file.clone()]);

        // Background
        args.push("-daemonize".to_string());

        Ok(args)
    }

    fn get_qemu_binary(&self) -> String {
        match std::env::consts::ARCH {
            "x86_64" => "qemu-system-x86_64",
            "aarch64" => "qemu-system-aarch64",
            _ => "qemu-system-x86_64",
        }.to_string()
    }

    fn kvm_available(&self) -> bool {
        std::path::Path::new("/dev/kvm").exists()
    }

    fn allocate_display_port(&self) -> Result<u16> {
        // Find available VNC port (5900-5999)
        for port in 5900..6000 {
            if !self.is_port_in_use(port) {
                return Ok(port);
            }
        }

        Err(anyhow::anyhow!("No available VNC ports"))
    }

    fn is_port_in_use(&self, port: u16) -> bool {
        std::net::TcpListener::bind(format!("0.0.0.0:{}", port)).is_err()
    }

    fn send_monitor_command(&self, command: &str) -> Result<()> {
        use std::io::Write;
        use std::os::unix::net::UnixStream;

        let mut stream = UnixStream::connect(&self.monitor_socket)?;
        writeln!(stream, "{}", command)?;

        Ok(())
    }

    pub fn live_migrate(&self, target_host: &str, target_port: u16) -> Result<()> {
        info!("Starting live migration of {} to {}:{}", self.name, target_host, target_port);

        // Set migration parameters
        self.send_monitor_command("migrate_set_parameter max-bandwidth 1000M")?;
        self.send_monitor_command("migrate_set_parameter downtime-limit 500")?;

        // Start migration
        let migrate_cmd = format!("migrate -d tcp:{}:{}", target_host, target_port);
        self.send_monitor_command(&migrate_cmd)?;

        info!("Live migration initiated for {}", self.name);
        Ok(())
    }

    pub fn create_snapshot(&self, name: &str) -> Result<()> {
        info!("Creating snapshot {} for VM {}", name, self.name);

        let output = Command::new("qemu-img")
            .args(&[
                "snapshot",
                "-c", name,
                &self.disk_path,
            ])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to create snapshot: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        Ok(())
    }

    pub fn restore_snapshot(&self, name: &str) -> Result<()> {
        info!("Restoring snapshot {} for VM {}", name, self.name);

        let output = Command::new("qemu-img")
            .args(&[
                "snapshot",
                "-a", name,
                &self.disk_path,
            ])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to restore snapshot: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        Ok(())
    }
}