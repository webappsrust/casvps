use anyhow::Result;
use std::path::Path;
use std::io::Write;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn, error};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// HTTP downloader with resume support and progress tracking
pub struct Downloader {
    client: Client,
    concurrent_downloads: Arc<Semaphore>,
    chunk_size: usize,
}

impl Downloader {
    pub async fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent("CasVPS/1.0.0 (Virtualization Platform)")
            .timeout(std::time::Duration::from_secs(300)) // 5 minute timeout per request
            .build()?;

        Ok(Self {
            client,
            concurrent_downloads: Arc::new(Semaphore::new(3)), // Max 3 concurrent downloads
            chunk_size: 1024 * 1024, // 1MB chunks
        })
    }

    /// Download a file from URL to local path with resume support
    pub async fn download_file(&self, url: &str, local_path: &str) -> Result<()> {
        let _permit = self.concurrent_downloads.acquire().await?;

        info!("Starting download: {} -> {}", url, local_path);

        // Create parent directories if they don't exist
        if let Some(parent) = Path::new(local_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Check if partial file exists for resume
        let start_pos = if Path::new(local_path).exists() {
            let metadata = tokio::fs::metadata(local_path).await?;
            metadata.len()
        } else {
            0
        };

        // Create request with range header for resume
        let mut request = self.client.get(url);
        if start_pos > 0 {
            info!("Resuming download from byte {}", start_pos);
            request = request.header("Range", format!("bytes={}-", start_pos));
        }

        let response = request.send().await?;

        if !response.status().is_success() && response.status().as_u16() != 206 {
            return Err(anyhow::anyhow!("HTTP error {}: {}", response.status(), response.status().canonical_reason().unwrap_or("Unknown")));
        }

        // Get total file size from headers
        let content_length = response.headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let total_size = if start_pos > 0 {
            start_pos + content_length
        } else {
            content_length
        };

        info!("Download size: {} bytes (resuming from {})", total_size, start_pos);

        // Open file for writing (append if resuming)
        let mut file = if start_pos > 0 {
            tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(local_path)
                .await?
        } else {
            tokio::fs::File::create(local_path).await?
        };

        // Download in chunks with progress tracking
        let mut downloaded = start_pos;
        let mut stream = response.bytes_stream();

        use futures::StreamExt;

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;

            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            // Log progress every 10MB
            if downloaded % (10 * 1024 * 1024) == 0 || downloaded == total_size {
                let progress = if total_size > 0 {
                    (downloaded * 100) / total_size
                } else {
                    0
                };
                info!("Download progress: {} / {} bytes ({}%)", downloaded, total_size, progress);
            }
        }

        file.sync_all().await?;
        info!("Download completed: {} ({} bytes)", local_path, downloaded);

        // Verify file size if known
        if total_size > 0 && downloaded != total_size {
            warn!("Download size mismatch: expected {}, got {}", total_size, downloaded);
        }

        Ok(())
    }

    /// Download with custom progress callback
    pub async fn download_with_progress<F>(&self, url: &str, local_path: &str, mut progress_callback: F) -> Result<()>
    where
        F: FnMut(u64, u64) + Send,
    {
        let _permit = self.concurrent_downloads.acquire().await?;

        info!("Starting download with progress tracking: {} -> {}", url, local_path);

        // Create parent directories
        if let Some(parent) = Path::new(local_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("HTTP error {}: {}", response.status(), response.status().canonical_reason().unwrap_or("Unknown")));
        }

        let total_size = response.headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let mut file = tokio::fs::File::create(local_path).await?;
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();

        use futures::StreamExt;

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;

            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            // Call progress callback
            progress_callback(downloaded, total_size);
        }

        file.sync_all().await?;
        info!("Download with progress completed: {} ({} bytes)", local_path, downloaded);

        Ok(())
    }

    /// Check if URL is accessible and get file info
    pub async fn get_file_info(&self, url: &str) -> Result<FileInfo> {
        let response = self.client.head(url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("HTTP error {}: {}", response.status(), response.status().canonical_reason().unwrap_or("Unknown")));
        }

        let size = response.headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let content_type = response.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        let last_modified = response.headers()
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| httpdate::parse_http_date(s).ok());

        let accepts_ranges = response.headers()
            .get("accept-ranges")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.contains("bytes"))
            .unwrap_or(false);

        Ok(FileInfo {
            url: url.to_string(),
            size,
            content_type,
            last_modified,
            supports_resume: accepts_ranges,
        })
    }

    /// Download multiple files concurrently
    pub async fn download_batch(&self, downloads: Vec<DownloadRequest>) -> Vec<DownloadResult> {
        let mut results = Vec::new();
        let mut handles = Vec::new();

        for request in downloads {
            let downloader = self.clone();
            let handle = tokio::spawn(async move {
                let start_time = std::time::Instant::now();
                let result = downloader.download_file(&request.url, &request.local_path).await;
                let duration = start_time.elapsed();

                DownloadResult {
                    url: request.url,
                    local_path: request.local_path,
                    success: result.is_ok(),
                    error: result.err().map(|e| e.to_string()),
                    download_time: duration,
                    file_size: if result.is_ok() {
                        tokio::fs::metadata(&request.local_path)
                            .await
                            .map(|m| m.len())
                            .unwrap_or(0)
                    } else {
                        0
                    },
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            if let Ok(result) = handle.await {
                results.push(result);
            }
        }

        results
    }

    /// Verify downloaded file integrity
    pub async fn verify_checksum(&self, file_path: &str, expected_checksum: &str, algorithm: ChecksumAlgorithm) -> Result<bool> {
        use sha2::{Sha256, Digest};
        use md5::Md5;
        use sha1::Sha1;
        use tokio::io::AsyncReadExt;

        let mut file = tokio::fs::File::open(file_path).await?;
        let mut buffer = vec![0; self.chunk_size];

        let actual_checksum = match algorithm {
            ChecksumAlgorithm::MD5 => {
                let mut hasher = Md5::new();
                loop {
                    let n = file.read(&mut buffer).await?;
                    if n == 0 { break; }
                    hasher.update(&buffer[..n]);
                }
                format!("{:x}", hasher.finalize())
            }
            ChecksumAlgorithm::SHA1 => {
                let mut hasher = Sha1::new();
                loop {
                    let n = file.read(&mut buffer).await?;
                    if n == 0 { break; }
                    hasher.update(&buffer[..n]);
                }
                format!("{:x}", hasher.finalize())
            }
            ChecksumAlgorithm::SHA256 => {
                let mut hasher = Sha256::new();
                loop {
                    let n = file.read(&mut buffer).await?;
                    if n == 0 { break; }
                    hasher.update(&buffer[..n]);
                }
                format!("{:x}", hasher.finalize())
            }
        };

        Ok(actual_checksum.eq_ignore_ascii_case(expected_checksum))
    }
}

impl Clone for Downloader {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            concurrent_downloads: self.concurrent_downloads.clone(),
            chunk_size: self.chunk_size,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub url: String,
    pub size: u64,
    pub content_type: String,
    pub last_modified: Option<std::time::SystemTime>,
    pub supports_resume: bool,
}

#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: String,
    pub local_path: String,
}

#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub url: String,
    pub local_path: String,
    pub success: bool,
    pub error: Option<String>,
    pub download_time: std::time::Duration,
    pub file_size: u64,
}

#[derive(Debug, Clone)]
pub enum ChecksumAlgorithm {
    MD5,
    SHA1,
    SHA256,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_downloader_creation() {
        let downloader = Downloader::new().await.unwrap();
        assert_eq!(downloader.chunk_size, 1024 * 1024);
    }

    #[tokio::test]
    async fn test_file_info_mock() {
        // This would test against a mock HTTP server in full implementation
        let info = FileInfo {
            url: "https://example.com/test.iso".to_string(),
            size: 1000000,
            content_type: "application/octet-stream".to_string(),
            last_modified: None,
            supports_resume: true,
        };

        assert_eq!(info.size, 1000000);
        assert!(info.supports_resume);
    }

    #[tokio::test]
    async fn test_download_request() {
        let request = DownloadRequest {
            url: "https://example.com/test.iso".to_string(),
            local_path: "/tmp/test.iso".to_string(),
        };

        assert_eq!(request.url, "https://example.com/test.iso");
        assert_eq!(request.local_path, "/tmp/test.iso");
    }
}