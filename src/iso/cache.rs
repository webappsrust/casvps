use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use super::downloader::Downloader;

/// Cache manager for temporary ISO downloads
/// According to spec: "URL Boot: Cache for 24 hours then auto-delete"
pub struct CacheManager {
    cache_dir: PathBuf,
    cache_index: Arc<RwLock<HashMap<String, CacheEntry>>>,
    max_cache_size: u64,
}

impl CacheManager {
    pub async fn new(cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir)?;

        let mut manager = Self {
            cache_dir,
            cache_index: Arc::new(RwLock::new(HashMap::new())),
            max_cache_size: 50 * 1024 * 1024 * 1024, // 50GB default cache limit
        };

        // Load existing cache index
        manager.load_cache_index().await?;

        Ok(manager)
    }

    /// Cache a file from URL for 24 hours
    pub async fn cache_url(&self, url: &str, downloader: &Downloader) -> Result<String> {
        info!("Caching URL: {}", url);

        let url_hash = self.hash_url(url);
        let cache_path = self.get_cache_path(&url_hash);

        // Check if already cached and not expired
        {
            let index = self.cache_index.read().await;
            if let Some(entry) = index.get(&url_hash) {
                if !entry.is_expired() && Path::new(&cache_path).exists() {
                    info!("Using existing cached file: {}", cache_path);
                    return Ok(cache_path);
                }
            }
        }

        // Download to cache
        info!("Downloading {} to cache", url);
        downloader.download_file(url, &cache_path).await?;

        // Get file size
        let file_size = tokio::fs::metadata(&cache_path).await?.len();

        // Update cache index
        {
            let mut index = self.cache_index.write().await;
            index.insert(url_hash.clone(), CacheEntry {
                url: url.to_string(),
                file_path: cache_path.clone(),
                cached_at: chrono::Utc::now(),
                file_size,
                access_count: 0,
            });
        }

        // Save cache index
        self.save_cache_index().await?;

        // Check cache size limits
        self.enforce_cache_limits().await?;

        info!("File cached successfully: {}", cache_path);
        Ok(cache_path)
    }

    /// Get cached file path if exists and not expired
    pub async fn get_cached_url(&self, url: &str) -> Option<String> {
        let url_hash = self.hash_url(url);
        let index = self.cache_index.read().await;

        if let Some(entry) = index.get(&url_hash) {
            if !entry.is_expired() && Path::new(&entry.file_path).exists() {
                return Some(entry.file_path.clone());
            }
        }

        None
    }

    /// Clean up expired cache entries
    pub async fn cleanup_expired(&self) -> Result<()> {
        info!("Cleaning up expired cache entries");

        let mut expired_keys = Vec::new();
        let mut total_cleaned = 0u64;

        // Find expired entries
        {
            let index = self.cache_index.read().await;
            for (key, entry) in index.iter() {
                if entry.is_expired() {
                    expired_keys.push(key.clone());
                    total_cleaned += entry.file_size;

                    // Remove file
                    if Path::new(&entry.file_path).exists() {
                        if let Err(e) = std::fs::remove_file(&entry.file_path) {
                            warn!("Failed to remove cached file {}: {}", entry.file_path, e);
                        } else {
                            info!("Removed expired cache file: {}", entry.file_path);
                        }
                    }
                }
            }
        }

        // Remove from index
        if !expired_keys.is_empty() {
            let mut index = self.cache_index.write().await;
            for key in &expired_keys {
                index.remove(key);
            }

            // Save updated index
            self.save_cache_index().await?;

            info!("Cleaned up {} expired cache entries, freed {} bytes",
                  expired_keys.len(), total_cleaned);
        }

        Ok(())
    }

    /// Get cache statistics
    pub async fn get_cache_count(&self) -> usize {
        let index = self.cache_index.read().await;
        index.len()
    }

    pub async fn get_cache_size(&self) -> u64 {
        let index = self.cache_index.read().await;
        index.values().map(|entry| entry.file_size).sum()
    }

    /// Clear all cache
    pub async fn clear_cache(&self) -> Result<()> {
        info!("Clearing all cache");

        // Remove all files
        {
            let index = self.cache_index.read().await;
            for entry in index.values() {
                if Path::new(&entry.file_path).exists() {
                    if let Err(e) = std::fs::remove_file(&entry.file_path) {
                        warn!("Failed to remove cached file {}: {}", entry.file_path, e);
                    }
                }
            }
        }

        // Clear index
        {
            let mut index = self.cache_index.write().await;
            index.clear();
        }

        // Save empty index
        self.save_cache_index().await?;

        info!("Cache cleared successfully");
        Ok(())
    }

    /// Set maximum cache size
    pub async fn set_max_cache_size(&mut self, size: u64) {
        self.max_cache_size = size;
        if let Err(e) = self.enforce_cache_limits().await {
            warn!("Failed to enforce cache limits: {}", e);
        }
    }

    fn hash_url(&self, url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn get_cache_path(&self, url_hash: &str) -> String {
        self.cache_dir
            .join(format!("{}.iso", url_hash))
            .to_string_lossy()
            .to_string()
    }

    async fn load_cache_index(&self) -> Result<()> {
        let index_path = self.cache_dir.join("cache_index.json");

        if !index_path.exists() {
            return Ok(()); // No existing index
        }

        let content = tokio::fs::read_to_string(&index_path).await?;
        let cache_data: HashMap<String, CacheEntry> = serde_json::from_str(&content)?;

        // Verify files still exist
        let mut valid_entries = HashMap::new();
        for (key, entry) in cache_data {
            if Path::new(&entry.file_path).exists() {
                valid_entries.insert(key, entry);
            } else {
                info!("Cached file missing, removing from index: {}", entry.file_path);
            }
        }

        let mut index = self.cache_index.write().await;
        *index = valid_entries;

        info!("Loaded {} cache entries from index", index.len());
        Ok(())
    }

    async fn save_cache_index(&self) -> Result<()> {
        let index_path = self.cache_dir.join("cache_index.json");
        let index = self.cache_index.read().await;

        let content = serde_json::to_string_pretty(&*index)?;
        tokio::fs::write(&index_path, content).await?;

        Ok(())
    }

    async fn enforce_cache_limits(&self) -> Result<()> {
        let current_size = self.get_cache_size().await;

        if current_size <= self.max_cache_size {
            return Ok();
        }

        info!("Cache size {} bytes exceeds limit {} bytes, cleaning up",
              current_size, self.max_cache_size);

        // Sort by access time (LRU eviction)
        let mut entries_to_remove = Vec::new();
        {
            let index = self.cache_index.read().await;
            let mut entries: Vec<(&String, &CacheEntry)> = index.iter().collect();
            entries.sort_by_key(|(_, entry)| entry.cached_at);

            let mut size_to_remove = current_size - self.max_cache_size;
            for (key, entry) in entries {
                if size_to_remove == 0 {
                    break;
                }

                entries_to_remove.push((key.clone(), entry.file_path.clone(), entry.file_size));
                size_to_remove = size_to_remove.saturating_sub(entry.file_size);
            }
        }

        // Remove oldest entries
        for (key, file_path, file_size) in entries_to_remove {
            if Path::new(&file_path).exists() {
                if let Err(e) = std::fs::remove_file(&file_path) {
                    warn!("Failed to remove cached file {}: {}", file_path, e);
                    continue;
                }
            }

            let mut index = self.cache_index.write().await;
            index.remove(&key);

            info!("Removed cache entry for size limit: {} ({} bytes)", file_path, file_size);
        }

        // Save updated index
        self.save_cache_index().await?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    url: String,
    file_path: String,
    cached_at: chrono::DateTime<chrono::Utc>,
    file_size: u64,
    access_count: u32,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        let now = chrono::Utc::now();
        let age = now.signed_duration_since(self.cached_at);
        age.num_hours() >= 24 // 24 hour expiry as per spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cache_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let cache_manager = CacheManager::new(temp_dir.path().to_path_buf()).await.unwrap();

        assert_eq!(cache_manager.get_cache_count().await, 0);
        assert_eq!(cache_manager.get_cache_size().await, 0);
    }

    #[tokio::test]
    async fn test_url_hashing() {
        let temp_dir = TempDir::new().unwrap();
        let cache_manager = CacheManager::new(temp_dir.path().to_path_buf()).await.unwrap();

        let hash1 = cache_manager.hash_url("https://example.com/file1.iso");
        let hash2 = cache_manager.hash_url("https://example.com/file2.iso");
        let hash3 = cache_manager.hash_url("https://example.com/file1.iso");

        assert_ne!(hash1, hash2);
        assert_eq!(hash1, hash3);
    }

    #[tokio::test]
    async fn test_cache_expiry() {
        let mut entry = CacheEntry {
            url: "https://example.com/test.iso".to_string(),
            file_path: "/tmp/test.iso".to_string(),
            cached_at: chrono::Utc::now() - chrono::Duration::hours(25), // 25 hours ago
            file_size: 1000,
            access_count: 0,
        };

        assert!(entry.is_expired());

        entry.cached_at = chrono::Utc::now() - chrono::Duration::hours(12); // 12 hours ago
        assert!(!entry.is_expired());
    }
}