use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Trait for ISO sources that provide download URLs and version information
#[async_trait]
pub trait ISOSource: Send + Sync {
    async fn get_latest_version(&self, major_version: &str, architecture: &str) -> Result<Option<String>>;
    async fn get_download_url(&self, major_version: &str, minor_version: &str, architecture: &str) -> Result<String>;
    async fn get_checksum(&self, major_version: &str, minor_version: &str, architecture: &str) -> Result<Option<String>>;
    fn get_supported_architectures(&self) -> Vec<String>;
    fn get_supported_major_versions(&self) -> Vec<String>;
}

/// Debian ISO source
pub struct DebianSource;

impl DebianSource {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ISOSource for DebianSource {
    async fn get_latest_version(&self, major_version: &str, _architecture: &str) -> Result<Option<String>> {
        // In full implementation, this would scrape Debian releases API
        match major_version {
            "12" => Ok(Some("12.8".to_string())), // Current as of implementation
            "11" => Ok(Some("11.11".to_string())),
            "10" => Ok(Some("10.13".to_string())),
            _ => Ok(None),
        }
    }

    async fn get_download_url(&self, major_version: &str, minor_version: &str, architecture: &str) -> Result<String> {
        let arch_map = match architecture {
            "x86_64" | "amd64" => "amd64",
            "aarch64" | "arm64" => "arm64",
            _ => return Err(anyhow::anyhow!("Unsupported architecture: {}", architecture)),
        };

        Ok(format!(
            "https://cdimage.debian.org/debian-cd/current/{}/iso-cd/debian-{}-{}-netinst.iso",
            arch_map, minor_version, arch_map
        ))
    }

    async fn get_checksum(&self, major_version: &str, minor_version: &str, architecture: &str) -> Result<Option<String>> {
        // In full implementation, this would fetch SHA256SUMS
        Ok(None)
    }

    fn get_supported_architectures(&self) -> Vec<String> {
        vec!["amd64".to_string(), "arm64".to_string()]
    }

    fn get_supported_major_versions(&self) -> Vec<String> {
        vec!["10".to_string(), "11".to_string(), "12".to_string()]
    }
}

/// Ubuntu ISO source
pub struct UbuntuSource;

impl UbuntuSource {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ISOSource for UbuntuSource {
    async fn get_latest_version(&self, major_version: &str, _architecture: &str) -> Result<Option<String>> {
        match major_version {
            "24.04" => Ok(Some("24.04.1".to_string())),
            "22.04" => Ok(Some("22.04.5".to_string())),
            "20.04" => Ok(Some("20.04.6".to_string())),
            _ => Ok(None),
        }
    }

    async fn get_download_url(&self, major_version: &str, minor_version: &str, architecture: &str) -> Result<String> {
        let arch_map = match architecture {
            "x86_64" | "amd64" => "amd64",
            "aarch64" | "arm64" => "arm64",
            _ => return Err(anyhow::anyhow!("Unsupported architecture: {}", architecture)),
        };

        Ok(format!(
            "https://releases.ubuntu.com/{}/ubuntu-{}-desktop-{}.iso",
            major_version, minor_version, arch_map
        ))
    }

    async fn get_checksum(&self, _major_version: &str, _minor_version: &str, _architecture: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn get_supported_architectures(&self) -> Vec<String> {
        vec!["amd64".to_string(), "arm64".to_string()]
    }

    fn get_supported_major_versions(&self) -> Vec<String> {
        vec!["20.04".to_string(), "22.04".to_string(), "24.04".to_string()]
    }
}

/// AlmaLinux ISO source
pub struct AlmaLinuxSource;

impl AlmaLinuxSource {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ISOSource for AlmaLinuxSource {
    async fn get_latest_version(&self, major_version: &str, _architecture: &str) -> Result<Option<String>> {
        match major_version {
            "9" => Ok(Some("9.5".to_string())),
            "8" => Ok(Some("8.10".to_string())),
            _ => Ok(None),
        }
    }

    async fn get_download_url(&self, major_version: &str, minor_version: &str, architecture: &str) -> Result<String> {
        let arch_map = match architecture {
            "x86_64" | "amd64" => "x86_64",
            "aarch64" | "arm64" => "aarch64",
            _ => return Err(anyhow::anyhow!("Unsupported architecture: {}", architecture)),
        };

        Ok(format!(
            "https://repo.almalinux.org/almalinux/{}/isos/{}/AlmaLinux-{}-{}-boot.iso",
            major_version, arch_map, minor_version, arch_map
        ))
    }

    async fn get_checksum(&self, _major_version: &str, _minor_version: &str, _architecture: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn get_supported_architectures(&self) -> Vec<String> {
        vec!["x86_64".to_string(), "aarch64".to_string()]
    }

    fn get_supported_major_versions(&self) -> Vec<String> {
        vec!["8".to_string(), "9".to_string()]
    }
}

/// Rocky Linux ISO source
pub struct RockySource;

impl RockySource {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ISOSource for RockySource {
    async fn get_latest_version(&self, major_version: &str, _architecture: &str) -> Result<Option<String>> {
        match major_version {
            "9" => Ok(Some("9.5".to_string())),
            "8" => Ok(Some("8.10".to_string())),
            _ => Ok(None),
        }
    }

    async fn get_download_url(&self, major_version: &str, minor_version: &str, architecture: &str) -> Result<String> {
        let arch_map = match architecture {
            "x86_64" | "amd64" => "x86_64",
            "aarch64" | "arm64" => "aarch64",
            _ => return Err(anyhow::anyhow!("Unsupported architecture: {}", architecture)),
        };

        Ok(format!(
            "https://download.rockylinux.org/pub/rocky/{}/isos/{}/Rocky-{}-{}-boot.iso",
            major_version, arch_map, minor_version, arch_map
        ))
    }

    async fn get_checksum(&self, _major_version: &str, _minor_version: &str, _architecture: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn get_supported_architectures(&self) -> Vec<String> {
        vec!["x86_64".to_string(), "aarch64".to_string()]
    }

    fn get_supported_major_versions(&self) -> Vec<String> {
        vec!["8".to_string(), "9".to_string()]
    }
}

/// Fedora ISO source
pub struct FedoraSource;

impl FedoraSource {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ISOSource for FedoraSource {
    async fn get_latest_version(&self, major_version: &str, _architecture: &str) -> Result<Option<String>> {
        // Fedora uses single version numbers
        Ok(Some(major_version.to_string()))
    }

    async fn get_download_url(&self, major_version: &str, _minor_version: &str, architecture: &str) -> Result<String> {
        let arch_map = match architecture {
            "x86_64" | "amd64" => "x86_64",
            "aarch64" | "arm64" => "aarch64",
            _ => return Err(anyhow::anyhow!("Unsupported architecture: {}", architecture)),
        };

        Ok(format!(
            "https://download.fedoraproject.org/pub/fedora/linux/releases/{}/Workstation/{}/iso/Fedora-Workstation-Live-{}-{}.iso",
            major_version, arch_map, arch_map, major_version
        ))
    }

    async fn get_checksum(&self, _major_version: &str, _minor_version: &str, _architecture: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn get_supported_architectures(&self) -> Vec<String> {
        vec!["x86_64".to_string(), "aarch64".to_string()]
    }

    fn get_supported_major_versions(&self) -> Vec<String> {
        vec!["39".to_string(), "40".to_string(), "41".to_string()]
    }
}

/// CentOS Stream ISO source
pub struct CentOSSource;

impl CentOSSource {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ISOSource for CentOSSource {
    async fn get_latest_version(&self, major_version: &str, _architecture: &str) -> Result<Option<String>> {
        match major_version {
            "9" => Ok(Some("9".to_string())),
            "8" => Ok(Some("8".to_string())),
            _ => Ok(None),
        }
    }

    async fn get_download_url(&self, major_version: &str, _minor_version: &str, architecture: &str) -> Result<String> {
        let arch_map = match architecture {
            "x86_64" | "amd64" => "x86_64",
            "aarch64" | "arm64" => "aarch64",
            _ => return Err(anyhow::anyhow!("Unsupported architecture: {}", architecture)),
        };

        Ok(format!(
            "https://mirror.stream.centos.org/{}-stream/BaseOS/{}/iso/CentOS-Stream-{}-{}-boot.iso",
            major_version, arch_map, major_version, arch_map
        ))
    }

    async fn get_checksum(&self, _major_version: &str, _minor_version: &str, _architecture: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn get_supported_architectures(&self) -> Vec<String> {
        vec!["x86_64".to_string(), "aarch64".to_string()]
    }

    fn get_supported_major_versions(&self) -> Vec<String> {
        vec!["8".to_string(), "9".to_string()]
    }
}

/// openSUSE ISO source
pub struct OpenSUSESource;

impl OpenSUSESource {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ISOSource for OpenSUSESource {
    async fn get_latest_version(&self, major_version: &str, _architecture: &str) -> Result<Option<String>> {
        match major_version {
            "15.6" => Ok(Some("15.6".to_string())),
            "15.5" => Ok(Some("15.5".to_string())),
            "tumbleweed" => Ok(Some("current".to_string())),
            _ => Ok(None),
        }
    }

    async fn get_download_url(&self, major_version: &str, _minor_version: &str, architecture: &str) -> Result<String> {
        let arch_map = match architecture {
            "x86_64" | "amd64" => "x86_64",
            "aarch64" | "arm64" => "aarch64",
            _ => return Err(anyhow::anyhow!("Unsupported architecture: {}", architecture)),
        };

        if major_version == "tumbleweed" {
            Ok(format!(
                "https://download.opensuse.org/tumbleweed/iso/openSUSE-Tumbleweed-DVD-{}-Current.iso",
                arch_map
            ))
        } else {
            Ok(format!(
                "https://download.opensuse.org/distribution/leap/{}/iso/openSUSE-Leap-{}-DVD-{}.iso",
                major_version, major_version, arch_map
            ))
        }
    }

    async fn get_checksum(&self, _major_version: &str, _minor_version: &str, _architecture: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn get_supported_architectures(&self) -> Vec<String> {
        vec!["x86_64".to_string(), "aarch64".to_string()]
    }

    fn get_supported_major_versions(&self) -> Vec<String> {
        vec!["15.5".to_string(), "15.6".to_string(), "tumbleweed".to_string()]
    }
}

/// Arch Linux ISO source
pub struct ArchSource;

impl ArchSource {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ISOSource for ArchSource {
    async fn get_latest_version(&self, _major_version: &str, _architecture: &str) -> Result<Option<String>> {
        // Arch Linux uses rolling releases with date-based versions
        let today = chrono::Utc::now();
        Ok(Some(today.format("%Y.%m.%d").to_string()))
    }

    async fn get_download_url(&self, _major_version: &str, minor_version: &str, architecture: &str) -> Result<String> {
        let arch_map = match architecture {
            "x86_64" | "amd64" => "x86_64",
            _ => return Err(anyhow::anyhow!("Unsupported architecture: {}", architecture)),
        };

        Ok(format!(
            "https://archive.archlinux.org/iso/{}/archlinux-{}-{}.iso",
            minor_version, minor_version, arch_map
        ))
    }

    async fn get_checksum(&self, _major_version: &str, _minor_version: &str, _architecture: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn get_supported_architectures(&self) -> Vec<String> {
        vec!["x86_64".to_string()]
    }

    fn get_supported_major_versions(&self) -> Vec<String> {
        vec!["current".to_string()]
    }
}