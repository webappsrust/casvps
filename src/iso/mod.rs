use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::database::Database;

pub mod sources;
pub mod cache;
pub mod downloader;

use sources::*;
use cache::*;
use downloader::*;

/// ISO and Template Management System
///
/// According to the spec: "Keep major versions (Debian 10, 11, 12), auto-update minor releases"
/// and "URL Boot: Cache for 24 hours then auto-delete"
pub struct ISOManager {
    database: Arc<Database>,
    storage_path: PathBuf,
    cache_manager: Arc<CacheManager>,
    downloader: Arc<Downloader>,
    sources: HashMap<String, Box<dyn ISOSource>>,
    auto_update_enabled: bool,
}

impl ISOManager {
    pub async fn new(database: Arc<Database>, storage_path: &str) -> Result<Self> {
        info!("Initializing ISO manager with storage path: {}", storage_path);

        // Create storage directories
        let storage_path = PathBuf::from(storage_path);
        std::fs::create_dir_all(&storage_path)?;
        std::fs::create_dir_all(storage_path.join("linux"))?;
        std::fs::create_dir_all(storage_path.join("windows"))?;
        std::fs::create_dir_all(storage_path.join("tools"))?;
        std::fs::create_dir_all(storage_path.join("cache"))?;

        let cache_manager = Arc::new(CacheManager::new(storage_path.join("cache")).await?);
        let downloader = Arc::new(Downloader::new().await?);

        // Initialize ISO sources for all major distributions
        let mut sources: HashMap<String, Box<dyn ISOSource>> = HashMap::new();
        sources.insert("debian".to_string(), Box::new(DebianSource::new()));
        sources.insert("ubuntu".to_string(), Box::new(UbuntuSource::new()));
        sources.insert("almalinux".to_string(), Box::new(AlmaLinuxSource::new()));
        sources.insert("rocky".to_string(), Box::new(RockySource::new()));
        sources.insert("fedora".to_string(), Box::new(FedoraSource::new()));
        sources.insert("centos".to_string(), Box::new(CentOSSource::new()));
        sources.insert("opensuse".to_string(), Box::new(OpenSUSESource::new()));
        sources.insert("arch".to_string(), Box::new(ArchSource::new()));

        Ok(Self {
            database,
            storage_path,
            cache_manager,
            downloader,
            sources,
            auto_update_enabled: true,
        })
    }

    pub async fn start(&self) -> Result<()> {
        if !self.auto_update_enabled {
            return Ok(());
        }

        info!("Starting ISO management services");

        // Start periodic ISO updates
        self.start_auto_updates().await?;

        // Start cache cleanup
        self.start_cache_cleanup().await?;

        // Load existing ISOs from database
        self.load_existing_isos().await?;

        Ok(())
    }

    /// Get available ISOs by distribution
    pub async fn get_available_isos(&self, distro: Option<&str>) -> Result<Vec<ISOInfo>> {
        let mut query = "SELECT id, distro_name, major_version, minor_version, architecture, filename, source_url, local_path, auto_update, created_at FROM iso_library WHERE 1=1".to_string();

        let isos = if let Some(distro) = distro {
            sqlx::query_as::<_, ISORow>(&format!("{} AND distro_name = ?", query))
                .bind(distro)
                .fetch_all(&self.database.pool)
                .await?
        } else {
            sqlx::query_as::<_, ISORow>(&query)
                .fetch_all(&self.database.pool)
                .await?
        };

        let iso_infos = isos.into_iter().map(|row| ISOInfo {
            id: row.id,
            distro_name: row.distro_name,
            major_version: row.major_version,
            minor_version: row.minor_version,
            architecture: row.architecture,
            filename: row.filename,
            source_url: row.source_url,
            local_path: row.local_path,
            auto_update: row.auto_update,
            created_at: row.created_at,
            file_size: self.get_file_size(&row.local_path).await.unwrap_or(0),
        }).collect();

        Ok(iso_infos)
    }

    /// Download and add ISO to library
    pub async fn add_iso(&self, request: &AddISORequest) -> Result<String> {
        info!("Adding ISO: {} {} {}", request.distro_name, request.major_version, request.architecture);

        let iso_id = Uuid::new_v4().to_string();

        // Generate filename and local path
        let filename = self.generate_filename(&request.distro_name, &request.major_version,
                                            &request.minor_version, &request.architecture);
        let local_path = self.get_local_path(&request.distro_name, &filename);

        // Check if ISO already exists
        if let Some(existing) = self.find_existing_iso(&request.distro_name, &request.major_version, &request.architecture).await? {
            if request.auto_update {
                // Update existing ISO
                return self.update_iso(&existing.id, request).await;
            } else {
                return Err(anyhow::anyhow!("ISO already exists: {}", existing.id));
            }
        }

        // Determine source URL
        let source_url = if let Some(url) = &request.source_url {
            url.clone()
        } else {
            // Auto-detect from official sources
            self.detect_source_url(&request.distro_name, &request.major_version,
                                 &request.minor_version, &request.architecture).await?
        };

        // Download ISO
        self.downloader.download_file(&source_url, &local_path).await?;

        // Verify checksum if provided
        if let Some(expected_checksum) = &request.checksum {
            let actual_checksum = self.calculate_checksum(&local_path).await?;
            if &actual_checksum != expected_checksum {
                std::fs::remove_file(&local_path)?;
                return Err(anyhow::anyhow!("Checksum mismatch: expected {}, got {}", expected_checksum, actual_checksum));
            }
        }

        // Add to database
        sqlx::query!(
            "INSERT INTO iso_library (id, distro_name, major_version, minor_version, architecture, filename, source_url, local_path, auto_update)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            iso_id,
            request.distro_name,
            request.major_version,
            request.minor_version,
            request.architecture,
            filename,
            source_url,
            local_path,
            request.auto_update
        )
        .execute(&self.database.pool)
        .await?;

        info!("ISO added successfully: {}", iso_id);
        Ok(iso_id)
    }

    /// Remove ISO from library
    pub async fn remove_iso(&self, iso_id: &str) -> Result<()> {
        info!("Removing ISO: {}", iso_id);

        // Get ISO info
        let iso = sqlx::query_as::<_, ISORow>(
            "SELECT id, distro_name, major_version, minor_version, architecture, filename, source_url, local_path, auto_update, created_at FROM iso_library WHERE id = ?"
        )
        .bind(iso_id)
        .fetch_one(&self.database.pool)
        .await?;

        // Remove file
        if Path::new(&iso.local_path).exists() {
            std::fs::remove_file(&iso.local_path)?;
        }

        // Remove from database
        sqlx::query!("DELETE FROM iso_library WHERE id = ?", iso_id)
            .execute(&self.database.pool)
            .await?;

        info!("ISO removed successfully: {}", iso_id);
        Ok(())
    }

    /// Download ISO from URL and cache for 24 hours
    pub async fn cache_url_iso(&self, url: &str) -> Result<String> {
        info!("Caching ISO from URL: {}", url);

        // Check if already cached
        if let Some(cached_path) = self.cache_manager.get_cached_url(url).await {
            if Path::new(&cached_path).exists() {
                return Ok(cached_path);
            }
        }

        // Download to cache
        let cached_path = self.cache_manager.cache_url(url, &self.downloader).await?;

        Ok(cached_path)
    }

    /// Update all auto-update ISOs
    pub async fn update_all_isos(&self) -> Result<()> {
        info!("Updating all auto-update ISOs");

        let auto_update_isos = sqlx::query_as::<_, ISORow>(
            "SELECT id, distro_name, major_version, minor_version, architecture, filename, source_url, local_path, auto_update, created_at
             FROM iso_library WHERE auto_update = TRUE"
        )
        .fetch_all(&self.database.pool)
        .await?;

        for iso in auto_update_isos {
            if let Err(e) = self.update_single_iso(&iso).await {
                error!("Failed to update ISO {}: {}", iso.id, e);
            }
        }

        Ok(())
    }

    /// Get PXE boot menu structure
    pub async fn get_pxe_menu(&self) -> Result<PXEMenu> {
        info!("Generating PXE boot menu");

        let all_isos = self.get_available_isos(None).await?;

        let mut menu = PXEMenu {
            title: "CasVPS Network Boot Menu".to_string(),
            entries: Vec::new(),
        };

        // Group by distribution
        let mut distro_groups: HashMap<String, Vec<ISOInfo>> = HashMap::new();
        for iso in all_isos {
            distro_groups.entry(iso.distro_name.clone()).or_insert_with(Vec::new).push(iso);
        }

        // Create menu entries
        for (distro, isos) in distro_groups {
            let mut submenu = PXEMenuEntry {
                title: distro.clone(),
                entry_type: PXEMenuType::Submenu,
                items: Vec::new(),
                boot_config: None,
            };

            for iso in isos {
                let boot_entry = PXEMenuEntry {
                    title: format!("{} {} ({})", iso.distro_name, iso.major_version, iso.architecture),
                    entry_type: PXEMenuType::Boot,
                    items: Vec::new(),
                    boot_config: Some(PXEBootConfig {
                        kernel_path: self.get_kernel_path(&iso).await?,
                        initrd_path: self.get_initrd_path(&iso).await?,
                        kernel_args: self.get_kernel_args(&iso).await?,
                    }),
                };
                submenu.items.push(boot_entry);
            }

            menu.entries.push(submenu);
        }

        Ok(menu)
    }

    async fn start_auto_updates(&self) -> Result<()> {
        let database = self.database.clone();
        let sources = self.sources.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_hours(24));

            loop {
                interval.tick().await;

                info!("Running daily ISO update check");

                // Update logic would go here
                // Check for new minor versions and update auto-update ISOs

                if let Err(e) = Self::check_and_update_isos(database.clone()).await {
                    error!("Failed to update ISOs: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn start_cache_cleanup(&self) -> Result<()> {
        let cache_manager = self.cache_manager.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_hours(1));

            loop {
                interval.tick().await;

                info!("Running cache cleanup");

                if let Err(e) = cache_manager.cleanup_expired().await {
                    error!("Failed to cleanup cache: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn load_existing_isos(&self) -> Result<()> {
        let isos = self.get_available_isos(None).await?;
        info!("Loaded {} ISOs from database", isos.len());

        // Verify local files exist
        for iso in isos {
            if !Path::new(&iso.local_path).exists() {
                warn!("ISO file missing: {} ({})", iso.filename, iso.local_path);
                // Mark for re-download if auto_update is enabled
            }
        }

        Ok(())
    }

    async fn find_existing_iso(&self, distro: &str, major_version: &str, arch: &str) -> Result<Option<ISOInfo>> {
        let iso = sqlx::query_as::<_, ISORow>(
            "SELECT id, distro_name, major_version, minor_version, architecture, filename, source_url, local_path, auto_update, created_at
             FROM iso_library WHERE distro_name = ? AND major_version = ? AND architecture = ?"
        )
        .bind(distro)
        .bind(major_version)
        .bind(arch)
        .fetch_optional(&self.database.pool)
        .await?;

        if let Some(row) = iso {
            Ok(Some(ISOInfo {
                id: row.id,
                distro_name: row.distro_name,
                major_version: row.major_version,
                minor_version: row.minor_version,
                architecture: row.architecture,
                filename: row.filename,
                source_url: row.source_url,
                local_path: row.local_path,
                auto_update: row.auto_update,
                created_at: row.created_at,
                file_size: self.get_file_size(&row.local_path).await.unwrap_or(0),
            }))
        } else {
            Ok(None)
        }
    }

    async fn update_iso(&self, iso_id: &str, request: &AddISORequest) -> Result<String> {
        info!("Updating existing ISO: {}", iso_id);

        // Get current ISO info
        let current = sqlx::query_as::<_, ISORow>(
            "SELECT id, distro_name, major_version, minor_version, architecture, filename, source_url, local_path, auto_update, created_at
             FROM iso_library WHERE id = ?"
        )
        .bind(iso_id)
        .fetch_one(&self.database.pool)
        .await?;

        // Check if update is needed (minor version change)
        if current.minor_version == request.minor_version {
            info!("ISO is already up to date: {}", iso_id);
            return Ok(iso_id.to_string());
        }

        // Remove old file
        if Path::new(&current.local_path).exists() {
            std::fs::remove_file(&current.local_path)?;
        }

        // Generate new filename and path
        let filename = self.generate_filename(&request.distro_name, &request.major_version,
                                            &request.minor_version, &request.architecture);
        let local_path = self.get_local_path(&request.distro_name, &filename);

        // Download new version
        let source_url = if let Some(url) = &request.source_url {
            url.clone()
        } else {
            self.detect_source_url(&request.distro_name, &request.major_version,
                                 &request.minor_version, &request.architecture).await?
        };

        self.downloader.download_file(&source_url, &local_path).await?;

        // Update database
        sqlx::query!(
            "UPDATE iso_library SET minor_version = ?, filename = ?, source_url = ?, local_path = ? WHERE id = ?",
            request.minor_version,
            filename,
            source_url,
            local_path,
            iso_id
        )
        .execute(&self.database.pool)
        .await?;

        info!("ISO updated successfully: {}", iso_id);
        Ok(iso_id.to_string())
    }

    async fn update_single_iso(&self, iso: &ISORow) -> Result<()> {
        // Check for newer version from source
        if let Some(source) = self.sources.get(&iso.distro_name) {
            if let Some(latest_version) = source.get_latest_version(&iso.major_version, &iso.architecture).await? {
                if latest_version != iso.minor_version {
                    let request = AddISORequest {
                        distro_name: iso.distro_name.clone(),
                        major_version: iso.major_version.clone(),
                        minor_version: latest_version,
                        architecture: iso.architecture.clone(),
                        source_url: None, // Auto-detect
                        checksum: None,
                        auto_update: true,
                    };

                    self.update_iso(&iso.id, &request).await?;
                }
            }
        }

        Ok(())
    }

    async fn detect_source_url(&self, distro: &str, major: &str, minor: &str, arch: &str) -> Result<String> {
        if let Some(source) = self.sources.get(distro) {
            source.get_download_url(major, minor, arch).await
        } else {
            Err(anyhow::anyhow!("Unknown distribution: {}", distro))
        }
    }

    fn generate_filename(&self, distro: &str, major: &str, minor: &str, arch: &str) -> String {
        format!("{}-{}.{}-{}.iso", distro, major, minor, arch)
    }

    fn get_local_path(&self, distro: &str, filename: &str) -> String {
        self.storage_path
            .join(self.get_distro_category(distro))
            .join(distro)
            .join(filename)
            .to_string_lossy()
            .to_string()
    }

    fn get_distro_category(&self, distro: &str) -> &str {
        match distro {
            "windows" => "windows",
            "macos" => "macos",
            _ => "linux",
        }
    }

    async fn get_file_size(&self, path: &str) -> Result<u64> {
        let metadata = tokio::fs::metadata(path).await?;
        Ok(metadata.len())
    }

    async fn calculate_checksum(&self, path: &str) -> Result<String> {
        use sha2::{Sha256, Digest};
        use tokio::io::AsyncReadExt;

        let mut file = tokio::fs::File::open(path).await?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; 8192];

        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    async fn get_kernel_path(&self, iso: &ISOInfo) -> Result<String> {
        // Extract kernel from ISO for PXE boot
        // This would mount the ISO and extract vmlinuz
        Ok(format!("/var/lib/casvps/tftp/kernels/{}-{}/vmlinuz", iso.distro_name, iso.major_version))
    }

    async fn get_initrd_path(&self, iso: &ISOInfo) -> Result<String> {
        // Extract initrd from ISO for PXE boot
        Ok(format!("/var/lib/casvps/tftp/kernels/{}-{}/initrd.img", iso.distro_name, iso.major_version))
    }

    async fn get_kernel_args(&self, iso: &ISOInfo) -> Result<String> {
        // Generate appropriate kernel arguments for each distro
        match iso.distro_name.as_str() {
            "debian" | "ubuntu" => {
                Ok(format!("boot=live config quiet splash iso-url=http://casvps.local/isos/{}", iso.filename))
            },
            "almalinux" | "rocky" | "centos" => {
                Ok(format!("inst.stage2=hd:LABEL=CasVPS-ISO inst.ks=http://casvps.local/kickstart/{}.cfg", iso.distro_name))
            },
            _ => {
                Ok("quiet splash".to_string())
            }
        }
    }

    async fn check_and_update_isos(_database: Arc<Database>) -> Result<()> {
        // Implementation would check for updates from official sources
        info!("Checking for ISO updates");
        Ok(())
    }

    pub async fn get_stats(&self) -> ISOStats {
        let total_isos = sqlx::query_scalar!("SELECT COUNT(*) FROM iso_library")
            .fetch_one(&self.database.pool)
            .await
            .unwrap_or(0) as usize;

        let auto_update_isos = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM iso_library WHERE auto_update = TRUE"
        )
        .fetch_one(&self.database.pool)
        .await
        .unwrap_or(0) as usize;

        // Calculate total storage used
        let mut total_storage = 0u64;
        if let Ok(isos) = self.get_available_isos(None).await {
            for iso in isos {
                total_storage += iso.file_size;
            }
        }

        let cached_items = self.cache_manager.get_cache_count().await;

        ISOStats {
            total_isos,
            auto_update_isos,
            total_storage_bytes: total_storage,
            cached_items,
            supported_distros: self.sources.keys().cloned().collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ISOInfo {
    pub id: String,
    pub distro_name: String,
    pub major_version: String,
    pub minor_version: String,
    pub architecture: String,
    pub filename: String,
    pub source_url: String,
    pub local_path: String,
    pub auto_update: bool,
    pub created_at: String,
    pub file_size: u64,
}

#[derive(Debug, Clone)]
pub struct AddISORequest {
    pub distro_name: String,
    pub major_version: String,
    pub minor_version: String,
    pub architecture: String,
    pub source_url: Option<String>,
    pub checksum: Option<String>,
    pub auto_update: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PXEMenu {
    pub title: String,
    pub entries: Vec<PXEMenuEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PXEMenuEntry {
    pub title: String,
    pub entry_type: PXEMenuType,
    pub items: Vec<PXEMenuEntry>,
    pub boot_config: Option<PXEBootConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PXEMenuType {
    Submenu,
    Boot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PXEBootConfig {
    pub kernel_path: String,
    pub initrd_path: String,
    pub kernel_args: String,
}

#[derive(Debug, Clone)]
pub struct ISOStats {
    pub total_isos: usize,
    pub auto_update_isos: usize,
    pub total_storage_bytes: u64,
    pub cached_items: usize,
    pub supported_distros: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct ISORow {
    id: String,
    distro_name: String,
    major_version: String,
    minor_version: String,
    architecture: String,
    filename: String,
    source_url: String,
    local_path: String,
    auto_update: bool,
    created_at: String,
}