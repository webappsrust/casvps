use anyhow::Result;
use std::fs;
use sysinfo::System;

#[derive(Debug, Clone)]
pub enum Platform {
    RaspberryPi4,
    RaspberryPi5,
    X86_64Homelab,
    X86_64Enterprise,
    ARM64Server,
}

impl Platform {
    pub fn detect() -> Result<Self> {
        let sys = System::new_all();
        let cpu_info = fs::read_to_string("/proc/cpuinfo")?;

        // Check for Raspberry Pi
        if cpu_info.contains("Raspberry Pi 4") {
            return Ok(Platform::RaspberryPi4);
        }
        if cpu_info.contains("Raspberry Pi 5") {
            return Ok(Platform::RaspberryPi5);
        }

        // Check architecture
        let arch = std::env::consts::ARCH;
        let total_memory = sys.total_memory();
        let cpu_count = sys.cpus().len();

        match arch {
            "aarch64" | "arm64" => {
                Ok(Platform::ARM64Server)
            }
            "x86_64" => {
                // Determine scale based on resources
                if total_memory > 128 * 1024 * 1024 * 1024 || cpu_count > 32 {
                    Ok(Platform::X86_64Enterprise)
                } else {
                    Ok(Platform::X86_64Homelab)
                }
            }
            _ => Err(anyhow::anyhow!("Unsupported platform: {}", arch))
        }
    }

    pub fn max_vms(&self) -> usize {
        match self {
            Platform::RaspberryPi4 => 5,
            Platform::RaspberryPi5 => 10,
            Platform::X86_64Homelab => 100,
            Platform::X86_64Enterprise => 10000,
            Platform::ARM64Server => 50,
        }
    }

    pub fn max_containers(&self) -> usize {
        match self {
            Platform::RaspberryPi4 => 20,
            Platform::RaspberryPi5 => 40,
            Platform::X86_64Homelab => 200,
            Platform::X86_64Enterprise => usize::MAX,
            Platform::ARM64Server => 100,
        }
    }

    pub fn default_memory_per_vm(&self) -> u64 {
        match self {
            Platform::RaspberryPi4 => 512 * 1024 * 1024,     // 512MB
            Platform::RaspberryPi5 => 1024 * 1024 * 1024,    // 1GB
            Platform::X86_64Homelab => 2 * 1024 * 1024 * 1024, // 2GB
            Platform::X86_64Enterprise => 4 * 1024 * 1024 * 1024, // 4GB
            Platform::ARM64Server => 1024 * 1024 * 1024,     // 1GB
        }
    }

    pub fn supports_nested_virtualization(&self) -> bool {
        match self {
            Platform::RaspberryPi4 | Platform::RaspberryPi5 => false,
            _ => true,
        }
    }

    pub fn supports_gpu_passthrough(&self) -> bool {
        match self {
            Platform::RaspberryPi4 | Platform::RaspberryPi5 => false,
            _ => true,
        }
    }
}