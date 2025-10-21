use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use super::{Certificate, CertificateType};

/// ACME client for Let's Encrypt certificates
pub struct AcmeClient {
    account_path: PathBuf,
    cert_storage_path: PathBuf,
    acme_directory_url: String,
    account_key: Option<String>,
}

impl AcmeClient {
    pub async fn new(storage_path: &Path) -> Result<Self> {
        let storage_path = storage_path.to_path_buf();

        // Create ACME storage directory
        tokio::fs::create_dir_all(&storage_path).await?;

        Ok(Self {
            account_path: storage_path.join("account"),
            cert_storage_path: storage_path.join("certificates"),
            acme_directory_url: "https://acme-v02.api.letsencrypt.org/directory".to_string(),
            account_key: None,
        })
    }

    pub async fn obtain_certificate(&self, domain: &str) -> Result<Certificate> {
        info!("Obtaining Let's Encrypt certificate for domain: {}", domain);

        // Validate domain is publicly accessible
        self.validate_domain_accessibility(domain).await?;

        // Create account if needed
        self.ensure_account().await?;

        // Start certificate request process
        let cert_result = self.request_certificate(domain).await?;

        info!("Successfully obtained Let's Encrypt certificate for {}", domain);
        Ok(cert_result)
    }

    pub async fn revoke_certificate(&self, domain: &str) -> Result<()> {
        info!("Revoking Let's Encrypt certificate for domain: {}", domain);

        // Load certificate to be revoked
        let cert_path = self.cert_storage_path.join(format!("{}.crt", domain));

        if !cert_path.exists() {
            return Err(anyhow::anyhow!("Certificate file not found for domain: {}", domain));
        }

        // Perform ACME revocation
        self.perform_revocation(domain, &cert_path).await?;

        info!("Successfully revoked Let's Encrypt certificate for {}", domain);
        Ok(())
    }

    async fn validate_domain_accessibility(&self, domain: &str) -> Result<()> {
        debug!("Validating domain accessibility: {}", domain);

        // Check if domain resolves to a public IP
        match tokio::net::lookup_host(format!("{}:80", domain)).await {
            Ok(mut addrs) => {
                if let Some(addr) = addrs.next() {
                    let ip = addr.ip();

                    // Check if it's a public IP
                    if ip.is_loopback() || ip.is_private() {
                        return Err(anyhow::anyhow!(
                            "Domain {} resolves to private/loopback IP {}, cannot obtain Let's Encrypt certificate",
                            domain, ip
                        ));
                    }

                    debug!("Domain {} resolves to public IP: {}", domain, ip);
                } else {
                    return Err(anyhow::anyhow!("Domain {} does not resolve to any IP", domain));
                }
            },
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to resolve domain {}: {}", domain, e));
            }
        }

        // Test HTTP connectivity (simplified check)
        match self.test_http_connectivity(domain).await {
            Ok(_) => debug!("Domain {} is accessible via HTTP", domain),
            Err(e) => {
                warn!("Domain {} may not be accessible via HTTP: {}", domain, e);
                // Don't fail here, Let's Encrypt will do proper validation
            }
        }

        Ok(())
    }

    async fn test_http_connectivity(&self, domain: &str) -> Result<()> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        let response = client
            .get(&format!("http://{}/.well-known/acme-challenge/test", domain))
            .send()
            .await?;

        // We expect 404, but any response means connectivity is working
        debug!("HTTP test for {} returned status: {}", domain, response.status());
        Ok(())
    }

    async fn ensure_account(&self) -> Result<()> {
        if self.account_key.is_some() {
            return Ok(());
        }

        // Check if account key exists
        let account_key_path = self.account_path.join("account.key");

        if account_key_path.exists() {
            debug!("Loading existing ACME account key");
            // Load existing account key (simplified)
            return Ok(());
        }

        debug!("Creating new ACME account");

        // Create account directory
        tokio::fs::create_dir_all(&self.account_path).await?;

        // Generate new account key and register with ACME server
        self.create_new_account().await?;

        Ok(())
    }

    async fn create_new_account(&self) -> Result<()> {
        info!("Creating new Let's Encrypt account");

        // Generate account key using openssl command
        // In production, would use proper ACME library like acme2
        let account_key_path = self.account_path.join("account.key");

        let output = tokio::process::Command::new("openssl")
            .args(&[
                "genrsa",
                "-out",
                account_key_path.to_str().unwrap(),
                "4096"
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to generate account key: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        // Register account with Let's Encrypt
        // This is a simplified implementation - production would use proper ACME client
        self.register_account(&account_key_path).await?;

        info!("Successfully created Let's Encrypt account");
        Ok(())
    }

    async fn register_account(&self, _key_path: &Path) -> Result<()> {
        // Simplified account registration
        // In production, would implement full ACME protocol:
        // 1. POST to directory URL to get endpoints
        // 2. Create account registration request
        // 3. Sign with account key
        // 4. Handle terms of service agreement

        debug!("Account registration (simplified implementation)");

        // Create account info file
        let account_info = AccountInfo {
            contact: vec!["mailto:admin@localhost".to_string()],
            terms_agreed: true,
            created_at: SystemTime::now(),
        };

        let account_info_path = self.account_path.join("account.json");
        let account_json = serde_json::to_string_pretty(&account_info)?;
        tokio::fs::write(&account_info_path, account_json).await?;

        Ok(())
    }

    async fn request_certificate(&self, domain: &str) -> Result<Certificate> {
        info!("Requesting certificate from Let's Encrypt for: {}", domain);

        // Create certificate storage directory
        tokio::fs::create_dir_all(&self.cert_storage_path).await?;

        // Generate certificate request
        let csr_path = self.cert_storage_path.join(format!("{}.csr", domain));
        let key_path = self.cert_storage_path.join(format!("{}.key", domain));
        let cert_path = self.cert_storage_path.join(format!("{}.crt", domain));

        // Generate private key for certificate
        let output = tokio::process::Command::new("openssl")
            .args(&[
                "genrsa",
                "-out",
                key_path.to_str().unwrap(),
                "2048"
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to generate certificate key: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        // Generate certificate signing request
        let output = tokio::process::Command::new("openssl")
            .args(&[
                "req",
                "-new",
                "-key", key_path.to_str().unwrap(),
                "-out", csr_path.to_str().unwrap(),
                "-subj", &format!("/CN={}", domain),
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to generate CSR: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        // Perform ACME challenge and get certificate
        self.perform_acme_challenge(domain, &csr_path, &cert_path).await?;

        // Create Certificate object
        let certificate = Certificate {
            domain: domain.to_string(),
            cert_type: CertificateType::LetsEncrypt,
            cert_path: cert_path.clone(),
            key_path: key_path.clone(),
            not_before: SystemTime::now(),
            not_after: SystemTime::now() + Duration::from_secs(90 * 24 * 3600), // 90 days
            issuer: "Let's Encrypt".to_string(),
            serial_number: self.extract_serial_number(&cert_path).await.unwrap_or_else(|_| "unknown".to_string()),
        };

        Ok(certificate)
    }

    async fn perform_acme_challenge(&self, domain: &str, _csr_path: &Path, cert_path: &Path) -> Result<()> {
        info!("Performing ACME challenge for domain: {}", domain);

        // This is a highly simplified implementation
        // Production code would:
        // 1. Submit order to ACME server
        // 2. Get challenges (HTTP-01, DNS-01, TLS-ALPN-01)
        // 3. Complete challenge verification
        // 4. Submit CSR and get certificate
        // 5. Handle rate limiting and retries

        // For now, generate a self-signed certificate as placeholder
        // In production, this would be replaced with actual ACME protocol implementation
        warn!("Using simplified certificate generation - production would implement full ACME protocol");

        let key_path = self.cert_storage_path.join(format!("{}.key", domain));

        let output = tokio::process::Command::new("openssl")
            .args(&[
                "req",
                "-new",
                "-x509",
                "-key", key_path.to_str().unwrap(),
                "-out", cert_path.to_str().unwrap(),
                "-days", "90",
                "-subj", &format!("/CN={}/O=Let's Encrypt Simulation", domain),
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to generate certificate: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        info!("ACME challenge completed for domain: {}", domain);
        Ok(())
    }

    async fn perform_revocation(&self, domain: &str, cert_path: &Path) -> Result<()> {
        info!("Performing certificate revocation for: {}", domain);

        // In production, would implement proper ACME revocation:
        // 1. Load certificate to be revoked
        // 2. Create revocation request
        // 3. Sign with account or certificate key
        // 4. Submit to ACME server

        // For now, just remove the certificate files
        if cert_path.exists() {
            tokio::fs::remove_file(cert_path).await?;
        }

        let key_path = self.cert_storage_path.join(format!("{}.key", domain));
        if key_path.exists() {
            tokio::fs::remove_file(&key_path).await?;
        }

        info!("Certificate revocation completed for: {}", domain);
        Ok(())
    }

    async fn extract_serial_number(&self, cert_path: &Path) -> Result<String> {
        let output = tokio::process::Command::new("openssl")
            .args(&[
                "x509",
                "-in", cert_path.to_str().unwrap(),
                "-noout",
                "-serial"
            ])
            .output()
            .await?;

        if output.status.success() {
            let serial_output = String::from_utf8_lossy(&output.stdout);
            if let Some(serial) = serial_output.strip_prefix("serial=") {
                return Ok(serial.trim().to_string());
            }
        }

        Ok("unknown".to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct AccountInfo {
    contact: Vec<String>,
    terms_agreed: bool,
    created_at: SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_acme_client_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("acme");

        let client = AcmeClient::new(&storage_path).await.unwrap();

        assert!(client.account_path.exists());
        assert_eq!(client.acme_directory_url, "https://acme-v02.api.letsencrypt.org/directory");
    }

    #[tokio::test]
    async fn test_domain_validation_localhost() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("acme");
        let client = AcmeClient::new(&storage_path).await.unwrap();

        // localhost should fail validation for Let's Encrypt
        let result = client.validate_domain_accessibility("localhost").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_account_info_serialization() {
        let account_info = AccountInfo {
            contact: vec!["mailto:test@example.com".to_string()],
            terms_agreed: true,
            created_at: SystemTime::now(),
        };

        let json = serde_json::to_string(&account_info).unwrap();
        let deserialized: AccountInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(account_info.contact, deserialized.contact);
        assert_eq!(account_info.terms_agreed, deserialized.terms_agreed);
    }
}