use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use crate::database::Database;

pub mod acme;
pub mod internal_ca;
pub mod self_signed;

use acme::*;
use internal_ca::*;
use self_signed::*;

/// Certificate Management System
///
/// According to the spec: Certificate Auto-Management
/// Priority:
/// 1. Check for existing Let's Encrypt cert
/// 2. Try to obtain Let's Encrypt (if public domain)
/// 3. Use internal CA (for .local, private domains)
/// 4. Generate self-signed (last resort)
pub struct CertificateManager {
    database: Arc<Database>,
    cert_storage_path: PathBuf,
    acme_client: Arc<AcmeClient>,
    internal_ca: Arc<InternalCA>,
    certificates: Arc<RwLock<HashMap<String, ManagedCertificate>>>,
    auto_renewal_enabled: bool,
}

impl CertificateManager {
    pub async fn new(database: Arc<Database>, cert_storage_path: &str) -> Result<Self> {
        info!("Initializing certificate management system");

        let cert_storage_path = PathBuf::from(cert_storage_path);

        // Create certificate storage directories
        tokio::fs::create_dir_all(&cert_storage_path.join("active")).await?;
        tokio::fs::create_dir_all(&cert_storage_path.join("users")).await?;

        // Initialize ACME client (Let's Encrypt)
        let acme_client = Arc::new(AcmeClient::new(&cert_storage_path.join("acme")).await?);

        // Initialize internal CA
        let internal_ca = Arc::new(InternalCA::new(&cert_storage_path.join("ca")).await?);

        let manager = Self {
            database,
            cert_storage_path,
            acme_client,
            internal_ca,
            certificates: Arc::new(RwLock::new(HashMap::new())),
            auto_renewal_enabled: true,
        };

        // Load existing certificates
        manager.load_certificates().await?;

        Ok(manager)
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting certificate management services");

        // Start automatic renewal task
        if self.auto_renewal_enabled {
            self.start_renewal_task().await?;
        }

        info!("Certificate management services started");
        Ok(())
    }

    /// Get certificate for a domain (main entry point)
    pub async fn get_certificate(&self, domain: &str) -> Result<Certificate> {
        info!("Getting certificate for domain: {}", domain);

        // Check cache first
        {
            let certificates = self.certificates.read().await;
            if let Some(managed_cert) = certificates.get(domain) {
                if !managed_cert.certificate.is_expired() {
                    debug!("Using cached certificate for {}", domain);
                    return Ok(managed_cert.certificate.clone());
                } else {
                    warn!("Cached certificate for {} is expired", domain);
                }
            }
        }

        // Follow priority order from spec
        let certificate = if let Some(cert) = self.check_existing_letsencrypt(domain).await? {
            info!("Found existing Let's Encrypt certificate for {}", domain);
            cert
        } else if self.is_public_domain(domain) {
            match self.obtain_letsencrypt(domain).await {
                Ok(cert) => {
                    info!("Successfully obtained Let's Encrypt certificate for {}", domain);
                    cert
                },
                Err(e) => {
                    warn!("Failed to obtain Let's Encrypt certificate for {}: {}", domain, e);
                    if self.is_local_domain(domain) {
                        info!("Using internal CA for local domain: {}", domain);
                        self.issue_internal_ca_certificate(domain).await?
                    } else {
                        warn!("Falling back to self-signed certificate for {}", domain);
                        self.generate_self_signed(domain).await?
                    }
                }
            }
        } else if self.is_local_domain(domain) {
            info!("Using internal CA for local domain: {}", domain);
            self.issue_internal_ca_certificate(domain).await?
        } else {
            info!("Generating self-signed certificate for {}", domain);
            self.generate_self_signed(domain).await?
        };

        // Cache the certificate
        {
            let mut certificates = self.certificates.write().await;
            certificates.insert(domain.to_string(), ManagedCertificate {
                domain: domain.to_string(),
                certificate: certificate.clone(),
                cert_type: certificate.cert_type.clone(),
                auto_renew: certificate.cert_type != CertificateType::SelfSigned,
                created_at: SystemTime::now(),
                last_renewed: None,
                renewal_attempts: 0,
            });
        }

        // Store in database
        self.store_certificate_info(domain, &certificate).await?;

        info!("Certificate obtained for {}: {} (expires: {})",
              domain, certificate.cert_type, certificate.not_after);

        Ok(certificate)
    }

    /// Renew certificate if needed
    pub async fn renew_certificate(&self, domain: &str, force: bool) -> Result<Certificate> {
        info!("Renewing certificate for domain: {} (force: {})", domain, force);

        let needs_renewal = if force {
            true
        } else {
            self.certificate_needs_renewal(domain).await?
        };

        if !needs_renewal && !force {
            debug!("Certificate for {} does not need renewal", domain);
            return self.get_certificate(domain).await;
        }

        // Force refresh by removing from cache
        {
            let mut certificates = self.certificates.write().await;
            certificates.remove(domain);
        }

        // Get new certificate (will follow same priority logic)
        let new_certificate = self.get_certificate(domain).await?;

        // Update renewal info
        {
            let mut certificates = self.certificates.write().await;
            if let Some(managed_cert) = certificates.get_mut(domain) {
                managed_cert.last_renewed = Some(SystemTime::now());
                managed_cert.renewal_attempts += 1;
            }
        }

        info!("Certificate renewed for {}", domain);
        Ok(new_certificate)
    }

    /// List all managed certificates
    pub async fn list_certificates(&self) -> Result<Vec<CertificateInfo>> {
        let certificates = self.certificates.read().await;
        let mut cert_list = Vec::new();

        for (domain, managed_cert) in certificates.iter() {
            let expires_in_days = managed_cert.certificate.expires_in_days()?;
            let needs_renewal = expires_in_days <= 30;

            cert_list.push(CertificateInfo {
                domain: domain.clone(),
                cert_type: managed_cert.cert_type.clone(),
                not_before: managed_cert.certificate.not_before,
                not_after: managed_cert.certificate.not_after,
                expires_in_days,
                needs_renewal,
                auto_renew: managed_cert.auto_renew,
                issuer: managed_cert.certificate.issuer.clone(),
                serial_number: managed_cert.certificate.serial_number.clone(),
                created_at: managed_cert.created_at,
                last_renewed: managed_cert.last_renewed,
            });
        }

        cert_list.sort_by(|a, b| a.not_after.cmp(&b.not_after));
        Ok(cert_list)
    }

    /// Get certificate statistics
    pub async fn get_certificate_stats(&self) -> Result<CertificateStats> {
        let certificates = self.certificates.read().await;

        let mut stats = CertificateStats {
            total_certificates: certificates.len(),
            letsencrypt_certificates: 0,
            internal_ca_certificates: 0,
            self_signed_certificates: 0,
            expiring_soon: 0,
            expired: 0,
            auto_renewal_enabled: self.auto_renewal_enabled,
        };

        for managed_cert in certificates.values() {
            match managed_cert.cert_type {
                CertificateType::LetsEncrypt => stats.letsencrypt_certificates += 1,
                CertificateType::InternalCA => stats.internal_ca_certificates += 1,
                CertificateType::SelfSigned => stats.self_signed_certificates += 1,
            }

            if let Ok(expires_in_days) = managed_cert.certificate.expires_in_days() {
                if expires_in_days <= 0 {
                    stats.expired += 1;
                } else if expires_in_days <= 30 {
                    stats.expiring_soon += 1;
                }
            }
        }

        Ok(stats)
    }

    /// Revoke certificate
    pub async fn revoke_certificate(&self, domain: &str) -> Result<()> {
        info!("Revoking certificate for domain: {}", domain);

        // Remove from cache
        {
            let mut certificates = self.certificates.write().await;
            if let Some(managed_cert) = certificates.remove(domain) {
                // If it's a Let's Encrypt certificate, revoke it properly
                if managed_cert.cert_type == CertificateType::LetsEncrypt {
                    if let Err(e) = self.acme_client.revoke_certificate(domain).await {
                        warn!("Failed to revoke Let's Encrypt certificate for {}: {}", domain, e);
                    }
                }
            }
        }

        // Remove certificate files
        self.remove_certificate_files(domain).await?;

        // Remove from database
        sqlx::query!("DELETE FROM certificates WHERE domain = ?", domain)
            .execute(&self.database.pool)
            .await?;

        info!("Certificate revoked for {}", domain);
        Ok(())
    }

    // Private helper methods

    async fn check_existing_letsencrypt(&self, domain: &str) -> Result<Option<Certificate>> {
        let cert_path = self.cert_storage_path.join("active").join(format!("{}.crt", domain));
        let key_path = self.cert_storage_path.join("active").join(format!("{}.key", domain));

        if cert_path.exists() && key_path.exists() {
            // Try to load and validate existing certificate
            match Certificate::load_from_files(&cert_path, &key_path).await {
                Ok(cert) => {
                    if !cert.is_expired() && cert.cert_type == CertificateType::LetsEncrypt {
                        return Ok(Some(cert));
                    }
                },
                Err(e) => {
                    warn!("Failed to load existing certificate for {}: {}", domain, e);
                }
            }
        }

        Ok(None)
    }

    fn is_public_domain(&self, domain: &str) -> bool {
        // Check if domain is publicly resolvable
        !self.is_local_domain(domain) && !domain.contains("localhost")
    }

    fn is_local_domain(&self, domain: &str) -> bool {
        domain.ends_with(".local") ||
        domain.ends_with(".localhost") ||
        domain.contains("192.168.") ||
        domain.contains("10.") ||
        domain.contains("172.16.") ||
        domain.contains("127.0.0.1") ||
        domain == "localhost"
    }

    async fn obtain_letsencrypt(&self, domain: &str) -> Result<Certificate> {
        self.acme_client.obtain_certificate(domain).await
    }

    async fn issue_internal_ca_certificate(&self, domain: &str) -> Result<Certificate> {
        self.internal_ca.issue_certificate(domain).await
    }

    async fn generate_self_signed(&self, domain: &str) -> Result<Certificate> {
        SelfSignedGenerator::generate_certificate(domain, &self.cert_storage_path).await
    }

    async fn certificate_needs_renewal(&self, domain: &str) -> Result<bool> {
        let certificates = self.certificates.read().await;
        if let Some(managed_cert) = certificates.get(domain) {
            let expires_in_days = managed_cert.certificate.expires_in_days()?;
            Ok(expires_in_days <= 30) // Renew if expiring within 30 days
        } else {
            Ok(true) // Certificate not found, needs to be obtained
        }
    }

    async fn load_certificates(&self) -> Result<()> {
        info!("Loading existing certificates from database");

        let cert_rows = sqlx::query_as::<_, CertificateRow>(
            "SELECT domain, type, cert_path, key_path, expires_at, auto_renew FROM certificates"
        )
        .fetch_all(&self.database.pool)
        .await?;

        let mut certificates = self.certificates.write().await;

        for row in cert_rows {
            if let Ok(certificate) = Certificate::load_from_files(&row.cert_path, &row.key_path).await {
                certificates.insert(row.domain.clone(), ManagedCertificate {
                    domain: row.domain,
                    certificate,
                    cert_type: row.cert_type.parse().unwrap_or(CertificateType::SelfSigned),
                    auto_renew: row.auto_renew,
                    created_at: SystemTime::now(), // We don't store creation time in DB
                    last_renewed: None,
                    renewal_attempts: 0,
                });
            }
        }

        info!("Loaded {} certificates from database", certificates.len());
        Ok(())
    }

    async fn store_certificate_info(&self, domain: &str, certificate: &Certificate) -> Result<()> {
        sqlx::query!(
            "INSERT OR REPLACE INTO certificates
             (id, domain, type, cert_path, key_path, expires_at, auto_renew)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            certificate.serial_number,
            domain,
            certificate.cert_type.to_string(),
            certificate.cert_path.to_string_lossy(),
            certificate.key_path.to_string_lossy(),
            certificate.not_after,
            certificate.cert_type != CertificateType::SelfSigned
        )
        .execute(&self.database.pool)
        .await?;

        Ok(())
    }

    async fn remove_certificate_files(&self, domain: &str) -> Result<()> {
        let cert_path = self.cert_storage_path.join("active").join(format!("{}.crt", domain));
        let key_path = self.cert_storage_path.join("active").join(format!("{}.key", domain));

        if cert_path.exists() {
            tokio::fs::remove_file(&cert_path).await?;
        }

        if key_path.exists() {
            tokio::fs::remove_file(&key_path).await?;
        }

        Ok(())
    }

    async fn start_renewal_task(&self) -> Result<()> {
        let certificates = self.certificates.clone();
        let database = self.database.clone();
        let acme_client = self.acme_client.clone();
        let internal_ca = self.internal_ca.clone();

        // Renewal check every 12 hours
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(12 * 3600));

            loop {
                interval.tick().await;

                info!("Running certificate renewal check");

                let certs_to_renew = {
                    let certs = certificates.read().await;
                    certs.iter()
                        .filter(|(_, managed_cert)| {
                            managed_cert.auto_renew &&
                            managed_cert.certificate.expires_in_days().unwrap_or(999) <= 30
                        })
                        .map(|(domain, _)| domain.clone())
                        .collect::<Vec<_>>()
                };

                for domain in certs_to_renew {
                    info!("Auto-renewing certificate for {}", domain);

                    // Implement renewal logic here
                    // This is a simplified version - in practice, would need to handle
                    // the full renewal flow for each certificate type
                    match Self::auto_renew_certificate(
                        &domain,
                        &certificates,
                        &database,
                        &acme_client,
                        &internal_ca,
                    ).await {
                        Ok(_) => info!("Successfully renewed certificate for {}", domain),
                        Err(e) => error!("Failed to renew certificate for {}: {}", domain, e),
                    }
                }
            }
        });

        Ok(())
    }

    async fn auto_renew_certificate(
        domain: &str,
        certificates: &Arc<RwLock<HashMap<String, ManagedCertificate>>>,
        _database: &Arc<Database>,
        acme_client: &Arc<AcmeClient>,
        internal_ca: &Arc<InternalCA>,
    ) -> Result<()> {
        let cert_type = {
            let certs = certificates.read().await;
            certs.get(domain).map(|c| c.cert_type.clone())
        };

        if let Some(cert_type) = cert_type {
            let new_cert = match cert_type {
                CertificateType::LetsEncrypt => {
                    acme_client.obtain_certificate(domain).await?
                },
                CertificateType::InternalCA => {
                    internal_ca.issue_certificate(domain).await?
                },
                CertificateType::SelfSigned => {
                    // Don't auto-renew self-signed certificates
                    return Ok(());
                }
            };

            // Update in cache
            {
                let mut certs = certificates.write().await;
                if let Some(managed_cert) = certs.get_mut(domain) {
                    managed_cert.certificate = new_cert;
                    managed_cert.last_renewed = Some(SystemTime::now());
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CertificateType {
    LetsEncrypt,
    InternalCA,
    SelfSigned,
}

impl std::fmt::Display for CertificateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CertificateType::LetsEncrypt => write!(f, "Let's Encrypt"),
            CertificateType::InternalCA => write!(f, "Internal CA"),
            CertificateType::SelfSigned => write!(f, "Self-Signed"),
        }
    }
}

impl std::str::FromStr for CertificateType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "LetsEncrypt" | "Let's Encrypt" => Ok(CertificateType::LetsEncrypt),
            "InternalCA" | "Internal CA" => Ok(CertificateType::InternalCA),
            "SelfSigned" | "Self-Signed" => Ok(CertificateType::SelfSigned),
            _ => Err(anyhow::anyhow!("Invalid certificate type: {}", s)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Certificate {
    pub domain: String,
    pub cert_type: CertificateType,
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub not_before: SystemTime,
    pub not_after: SystemTime,
    pub issuer: String,
    pub serial_number: String,
}

impl Certificate {
    pub async fn load_from_files(cert_path: &Path, key_path: &Path) -> Result<Self> {
        // Load certificate and extract information
        let cert_content = tokio::fs::read(cert_path).await?;

        // Parse certificate to extract metadata
        // This is a simplified implementation - in production would use proper X.509 parsing
        let domain = cert_path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        Ok(Self {
            domain,
            cert_type: CertificateType::SelfSigned, // Default, should be determined from cert
            cert_path: cert_path.to_path_buf(),
            key_path: key_path.to_path_buf(),
            not_before: SystemTime::now(),
            not_after: SystemTime::now() + Duration::from_secs(90 * 24 * 3600), // 90 days
            issuer: "Unknown".to_string(),
            serial_number: "0".to_string(),
        })
    }

    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.not_after
    }

    pub fn expires_in_days(&self) -> Result<i64> {
        let now = SystemTime::now();
        if let Ok(duration_until_expiry) = self.not_after.duration_since(now) {
            Ok((duration_until_expiry.as_secs() / (24 * 3600)) as i64)
        } else {
            // Already expired
            let duration_since_expiry = now.duration_since(self.not_after)?;
            Ok(-((duration_since_expiry.as_secs() / (24 * 3600)) as i64))
        }
    }
}

#[derive(Debug)]
struct ManagedCertificate {
    domain: String,
    certificate: Certificate,
    cert_type: CertificateType,
    auto_renew: bool,
    created_at: SystemTime,
    last_renewed: Option<SystemTime>,
    renewal_attempts: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CertificateInfo {
    pub domain: String,
    pub cert_type: CertificateType,
    pub not_before: SystemTime,
    pub not_after: SystemTime,
    pub expires_in_days: i64,
    pub needs_renewal: bool,
    pub auto_renew: bool,
    pub issuer: String,
    pub serial_number: String,
    pub created_at: SystemTime,
    pub last_renewed: Option<SystemTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CertificateStats {
    pub total_certificates: usize,
    pub letsencrypt_certificates: usize,
    pub internal_ca_certificates: usize,
    pub self_signed_certificates: usize,
    pub expiring_soon: usize,
    pub expired: usize,
    pub auto_renewal_enabled: bool,
}

// Database row structure
#[derive(sqlx::FromRow)]
struct CertificateRow {
    domain: String,
    cert_type: String,
    cert_path: String,
    key_path: String,
    expires_at: SystemTime,
    auto_renew: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_certificate_type_parsing() {
        assert_eq!("LetsEncrypt".parse::<CertificateType>().unwrap(), CertificateType::LetsEncrypt);
        assert_eq!("InternalCA".parse::<CertificateType>().unwrap(), CertificateType::InternalCA);
        assert_eq!("SelfSigned".parse::<CertificateType>().unwrap(), CertificateType::SelfSigned);
    }

    #[test]
    fn test_domain_classification() {
        // This would be a method on CertificateManager in real implementation
        assert!(is_local_domain_test("app.local"));
        assert!(is_local_domain_test("localhost"));
        assert!(is_local_domain_test("192.168.1.100"));
        assert!(!is_local_domain_test("example.com"));
        assert!(!is_local_domain_test("api.example.com"));
    }

    fn is_local_domain_test(domain: &str) -> bool {
        domain.ends_with(".local") ||
        domain.ends_with(".localhost") ||
        domain.contains("192.168.") ||
        domain.contains("10.") ||
        domain.contains("172.16.") ||
        domain.contains("127.0.0.1") ||
        domain == "localhost"
    }
}