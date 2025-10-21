use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::{info, debug};
use super::{Certificate, CertificateType};

/// Self-signed certificate generator (last resort)
pub struct SelfSignedGenerator;

impl SelfSignedGenerator {
    pub async fn generate_certificate(domain: &str, storage_path: &Path) -> Result<Certificate> {
        info!("Generating self-signed certificate for domain: {}", domain);

        let cert_storage_path = storage_path.join("active");
        tokio::fs::create_dir_all(&cert_storage_path).await?;

        let key_path = cert_storage_path.join(format!("{}.key", domain));
        let cert_path = cert_storage_path.join(format!("{}.crt", domain));

        // Generate private key
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
            return Err(anyhow::anyhow!("Failed to generate private key: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        // Generate self-signed certificate
        let subject = format!("/CN={}/O=CasVPS/OU=Self-Signed", domain);

        let output = tokio::process::Command::new("openssl")
            .args(&[
                "req",
                "-new",
                "-x509",
                "-key", key_path.to_str().unwrap(),
                "-out", cert_path.to_str().unwrap(),
                "-days", "365",
                "-subj", &subject,
                "-extensions", "v3_req",
                "-config", "/dev/stdin"
            ])
            .stdin(std::process::Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to generate self-signed certificate: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        // Extract certificate information
        let serial_number = Self::extract_serial_number(&cert_path).await
            .unwrap_or_else(|_| "unknown".to_string());

        let certificate = Certificate {
            domain: domain.to_string(),
            cert_type: CertificateType::SelfSigned,
            cert_path: cert_path.clone(),
            key_path: key_path.clone(),
            not_before: SystemTime::now(),
            not_after: SystemTime::now() + Duration::from_secs(365 * 24 * 3600), // 1 year
            issuer: format!("{} (Self-Signed)", domain),
            serial_number,
        };

        info!("Successfully generated self-signed certificate for {}", domain);
        Ok(certificate)
    }

    pub async fn generate_with_san(domain: &str, san_domains: &[String], storage_path: &Path) -> Result<Certificate> {
        info!("Generating self-signed certificate with SAN for domain: {} (SAN: {:?})", domain, san_domains);

        let cert_storage_path = storage_path.join("active");
        tokio::fs::create_dir_all(&cert_storage_path).await?;

        let key_path = cert_storage_path.join(format!("{}.key", domain));
        let cert_path = cert_storage_path.join(format!("{}.crt", domain));
        let config_path = cert_storage_path.join(format!("{}.conf", domain));

        // Generate private key
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
            return Err(anyhow::anyhow!("Failed to generate private key: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        // Create OpenSSL config with SAN
        let config_content = Self::generate_openssl_config(domain, san_domains)?;
        tokio::fs::write(&config_path, config_content).await?;

        // Generate self-signed certificate with SAN
        let subject = format!("/CN={}/O=CasVPS/OU=Self-Signed", domain);

        let output = tokio::process::Command::new("openssl")
            .args(&[
                "req",
                "-new",
                "-x509",
                "-key", key_path.to_str().unwrap(),
                "-out", cert_path.to_str().unwrap(),
                "-days", "365",
                "-subj", &subject,
                "-config", config_path.to_str().unwrap(),
                "-extensions", "v3_req"
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to generate self-signed certificate with SAN: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        // Clean up config file
        let _ = tokio::fs::remove_file(&config_path).await;

        // Extract certificate information
        let serial_number = Self::extract_serial_number(&cert_path).await
            .unwrap_or_else(|_| "unknown".to_string());

        let certificate = Certificate {
            domain: domain.to_string(),
            cert_type: CertificateType::SelfSigned,
            cert_path: cert_path.clone(),
            key_path: key_path.clone(),
            not_before: SystemTime::now(),
            not_after: SystemTime::now() + Duration::from_secs(365 * 24 * 3600), // 1 year
            issuer: format!("{} (Self-Signed with SAN)", domain),
            serial_number,
        };

        info!("Successfully generated self-signed certificate with SAN for {}", domain);
        Ok(certificate)
    }

    pub async fn generate_wildcard(domain: &str, storage_path: &Path) -> Result<Certificate> {
        info!("Generating wildcard self-signed certificate for domain: *.{}", domain);

        let wildcard_domain = format!("*.{}", domain);
        let cert_storage_path = storage_path.join("active");
        tokio::fs::create_dir_all(&cert_storage_path).await?;

        let key_path = cert_storage_path.join(format!("wildcard-{}.key", domain));
        let cert_path = cert_storage_path.join(format!("wildcard-{}.crt", domain));

        // Generate private key
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
            return Err(anyhow::anyhow!("Failed to generate private key: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        // Generate wildcard certificate
        let subject = format!("/CN={}/O=CasVPS/OU=Self-Signed-Wildcard", wildcard_domain);

        let output = tokio::process::Command::new("openssl")
            .args(&[
                "req",
                "-new",
                "-x509",
                "-key", key_path.to_str().unwrap(),
                "-out", cert_path.to_str().unwrap(),
                "-days", "365",
                "-subj", &subject,
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to generate wildcard certificate: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        // Extract certificate information
        let serial_number = Self::extract_serial_number(&cert_path).await
            .unwrap_or_else(|_| "unknown".to_string());

        let certificate = Certificate {
            domain: wildcard_domain.clone(),
            cert_type: CertificateType::SelfSigned,
            cert_path: cert_path.clone(),
            key_path: key_path.clone(),
            not_before: SystemTime::now(),
            not_after: SystemTime::now() + Duration::from_secs(365 * 24 * 3600), // 1 year
            issuer: format!("{} (Self-Signed Wildcard)", wildcard_domain),
            serial_number,
        };

        info!("Successfully generated wildcard self-signed certificate for *.{}", domain);
        Ok(certificate)
    }

    fn generate_openssl_config(domain: &str, san_domains: &[String]) -> Result<String> {
        let mut config = format!(r#"
[req]
distinguished_name = req_distinguished_name
req_extensions = v3_req
prompt = no

[req_distinguished_name]
CN = {}

[v3_req]
keyUsage = keyEncipherment, dataEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = {}
"#, domain, domain);

        // Add SAN domains
        for (i, san_domain) in san_domains.iter().enumerate() {
            config.push_str(&format!("DNS.{} = {}\n", i + 2, san_domain));
        }

        Ok(config)
    }

    async fn extract_serial_number(cert_path: &Path) -> Result<String> {
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

    pub async fn verify_certificate(cert_path: &Path) -> Result<CertificateInfo> {
        debug!("Verifying certificate: {}", cert_path.display());

        // Extract certificate information
        let subject = Self::extract_subject(cert_path).await?;
        let issuer = Self::extract_issuer(cert_path).await?;
        let serial_number = Self::extract_serial_number(cert_path).await?;
        let not_before = Self::extract_not_before(cert_path).await?;
        let not_after = Self::extract_not_after(cert_path).await?;
        let san_domains = Self::extract_san_domains(cert_path).await?;

        Ok(CertificateInfo {
            subject,
            issuer,
            serial_number,
            not_before,
            not_after,
            san_domains,
        })
    }

    async fn extract_subject(cert_path: &Path) -> Result<String> {
        let output = tokio::process::Command::new("openssl")
            .args(&[
                "x509",
                "-in", cert_path.to_str().unwrap(),
                "-noout",
                "-subject"
            ])
            .output()
            .await?;

        if output.status.success() {
            let subject_output = String::from_utf8_lossy(&output.stdout);
            if let Some(subject) = subject_output.strip_prefix("subject=") {
                return Ok(subject.trim().to_string());
            }
        }

        Ok("Unknown".to_string())
    }

    async fn extract_issuer(cert_path: &Path) -> Result<String> {
        let output = tokio::process::Command::new("openssl")
            .args(&[
                "x509",
                "-in", cert_path.to_str().unwrap(),
                "-noout",
                "-issuer"
            ])
            .output()
            .await?;

        if output.status.success() {
            let issuer_output = String::from_utf8_lossy(&output.stdout);
            if let Some(issuer) = issuer_output.strip_prefix("issuer=") {
                return Ok(issuer.trim().to_string());
            }
        }

        Ok("Unknown".to_string())
    }

    async fn extract_not_before(cert_path: &Path) -> Result<SystemTime> {
        let output = tokio::process::Command::new("openssl")
            .args(&[
                "x509",
                "-in", cert_path.to_str().unwrap(),
                "-noout",
                "-startdate"
            ])
            .output()
            .await?;

        if output.status.success() {
            let startdate_output = String::from_utf8_lossy(&output.stdout);
            if let Some(_startdate) = startdate_output.strip_prefix("notBefore=") {
                // Simplified - would parse actual date in production
                return Ok(SystemTime::now());
            }
        }

        Ok(SystemTime::now())
    }

    async fn extract_not_after(cert_path: &Path) -> Result<SystemTime> {
        let output = tokio::process::Command::new("openssl")
            .args(&[
                "x509",
                "-in", cert_path.to_str().unwrap(),
                "-noout",
                "-enddate"
            ])
            .output()
            .await?;

        if output.status.success() {
            let enddate_output = String::from_utf8_lossy(&output.stdout);
            if let Some(_enddate) = enddate_output.strip_prefix("notAfter=") {
                // Simplified - would parse actual date in production
                return Ok(SystemTime::now() + Duration::from_secs(365 * 24 * 3600));
            }
        }

        Ok(SystemTime::now() + Duration::from_secs(365 * 24 * 3600))
    }

    async fn extract_san_domains(cert_path: &Path) -> Result<Vec<String>> {
        let output = tokio::process::Command::new("openssl")
            .args(&[
                "x509",
                "-in", cert_path.to_str().unwrap(),
                "-noout",
                "-ext", "subjectAltName"
            ])
            .output()
            .await?;

        let mut san_domains = Vec::new();

        if output.status.success() {
            let san_output = String::from_utf8_lossy(&output.stdout);

            for line in san_output.lines() {
                if line.contains("DNS:") {
                    // Parse DNS entries from SAN
                    for part in line.split(',') {
                        if let Some(dns_name) = part.trim().strip_prefix("DNS:") {
                            san_domains.push(dns_name.trim().to_string());
                        }
                    }
                }
            }
        }

        Ok(san_domains)
    }
}

#[derive(Debug, Clone)]
pub struct CertificateInfo {
    pub subject: String,
    pub issuer: String,
    pub serial_number: String,
    pub not_before: SystemTime,
    pub not_after: SystemTime,
    pub san_domains: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_generate_self_signed_certificate() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path();

        let certificate = SelfSignedGenerator::generate_certificate("test.local", storage_path).await.unwrap();

        assert_eq!(certificate.domain, "test.local");
        assert_eq!(certificate.cert_type, CertificateType::SelfSigned);
        assert!(certificate.cert_path.exists());
        assert!(certificate.key_path.exists());
    }

    #[tokio::test]
    async fn test_generate_certificate_with_san() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path();

        let san_domains = vec![
            "api.test.local".to_string(),
            "www.test.local".to_string(),
        ];

        let certificate = SelfSignedGenerator::generate_with_san("test.local", &san_domains, storage_path).await.unwrap();

        assert_eq!(certificate.domain, "test.local");
        assert_eq!(certificate.cert_type, CertificateType::SelfSigned);
        assert!(certificate.issuer.contains("Self-Signed with SAN"));
        assert!(certificate.cert_path.exists());
        assert!(certificate.key_path.exists());
    }

    #[tokio::test]
    async fn test_generate_wildcard_certificate() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path();

        let certificate = SelfSignedGenerator::generate_wildcard("test.local", storage_path).await.unwrap();

        assert_eq!(certificate.domain, "*.test.local");
        assert_eq!(certificate.cert_type, CertificateType::SelfSigned);
        assert!(certificate.issuer.contains("Wildcard"));
        assert!(certificate.cert_path.exists());
        assert!(certificate.key_path.exists());
    }

    #[test]
    fn test_openssl_config_generation() {
        let domain = "test.local";
        let san_domains = vec![
            "api.test.local".to_string(),
            "www.test.local".to_string(),
        ];

        let config = SelfSignedGenerator::generate_openssl_config(domain, &san_domains).unwrap();

        assert!(config.contains("CN = test.local"));
        assert!(config.contains("DNS.1 = test.local"));
        assert!(config.contains("DNS.2 = api.test.local"));
        assert!(config.contains("DNS.3 = www.test.local"));
    }
}