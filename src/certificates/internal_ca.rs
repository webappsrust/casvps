use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::{info, warn, debug};
use serde::{Deserialize, Serialize};
use super::{Certificate, CertificateType};

/// Internal Certificate Authority for local domains
pub struct InternalCA {
    ca_path: PathBuf,
    cert_storage_path: PathBuf,
    ca_initialized: bool,
}

impl InternalCA {
    pub async fn new(storage_path: &Path) -> Result<Self> {
        let storage_path = storage_path.to_path_buf();

        // Create CA storage directories
        tokio::fs::create_dir_all(&storage_path).await?;
        tokio::fs::create_dir_all(&storage_path.join("certificates")).await?;

        let mut ca = Self {
            ca_path: storage_path,
            cert_storage_path: storage_path.join("certificates"),
            ca_initialized: false,
        };

        // Initialize CA if needed
        ca.ensure_ca_initialized().await?;

        Ok(ca)
    }

    pub async fn issue_certificate(&self, domain: &str) -> Result<Certificate> {
        info!("Issuing internal CA certificate for domain: {}", domain);

        if !self.ca_initialized {
            return Err(anyhow::anyhow!("Internal CA not initialized"));
        }

        // Generate certificate using internal CA
        let cert_result = self.generate_certificate(domain).await?;

        info!("Successfully issued internal CA certificate for {}", domain);
        Ok(cert_result)
    }

    pub async fn get_ca_certificate(&self) -> Result<String> {
        let ca_cert_path = self.ca_path.join("ca.crt");

        if !ca_cert_path.exists() {
            return Err(anyhow::anyhow!("CA certificate not found"));
        }

        let ca_cert_content = tokio::fs::read_to_string(&ca_cert_path).await?;
        Ok(ca_cert_content)
    }

    pub async fn get_ca_info(&self) -> Result<CAInfo> {
        let ca_cert_path = self.ca_path.join("ca.crt");

        if !ca_cert_path.exists() {
            return Err(anyhow::anyhow!("CA certificate not found"));
        }

        // Extract CA information using openssl
        let subject = self.extract_ca_subject().await?;
        let not_after = self.extract_ca_expiry().await?;
        let serial_number = self.extract_ca_serial().await?;

        let issued_certificates = self.count_issued_certificates().await?;

        Ok(CAInfo {
            subject,
            not_after,
            serial_number,
            issued_certificates,
            ca_cert_path: ca_cert_path.to_string_lossy().to_string(),
        })
    }

    async fn ensure_ca_initialized(&mut self) -> Result<()> {
        let ca_cert_path = self.ca_path.join("ca.crt");
        let ca_key_path = self.ca_path.join("ca.key");

        if ca_cert_path.exists() && ca_key_path.exists() {
            debug!("Internal CA already exists");
            self.ca_initialized = true;
            return Ok(());
        }

        info!("Initializing internal Certificate Authority");

        // Generate CA private key
        let output = tokio::process::Command::new("openssl")
            .args(&[
                "genrsa",
                "-out",
                ca_key_path.to_str().unwrap(),
                "4096"
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to generate CA private key: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        // Generate CA certificate
        let ca_subject = "/CN=CasVPS Internal CA/O=CasVPS/OU=Certificate Authority";

        let output = tokio::process::Command::new("openssl")
            .args(&[
                "req",
                "-new",
                "-x509",
                "-key", ca_key_path.to_str().unwrap(),
                "-out", ca_cert_path.to_str().unwrap(),
                "-days", "3650", // 10 years
                "-subj", ca_subject,
                "-extensions", "v3_ca",
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to generate CA certificate: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        // Set secure permissions on CA key
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&ca_key_path, permissions)?;
        }

        // Create CA database files for tracking issued certificates
        self.initialize_ca_database().await?;

        self.ca_initialized = true;
        info!("Internal CA initialized successfully");

        Ok(())
    }

    async fn initialize_ca_database(&self) -> Result<()> {
        // Create OpenSSL CA database files
        let ca_db_path = self.ca_path.join("index.txt");
        let ca_serial_path = self.ca_path.join("serial");

        // Initialize empty index file
        tokio::fs::write(&ca_db_path, "").await?;

        // Initialize serial number
        tokio::fs::write(&ca_serial_path, "01\n").await?;

        // Create CA config file
        let ca_config = self.generate_ca_config().await?;
        let ca_config_path = self.ca_path.join("ca.conf");
        tokio::fs::write(&ca_config_path, ca_config).await?;

        Ok(())
    }

    async fn generate_ca_config(&self) -> Result<String> {
        let ca_config = format!(r#"
[ ca ]
default_ca = CA_default

[ CA_default ]
dir = {}
certs = $dir/certificates
crl_dir = $dir/crl
database = $dir/index.txt
new_certs_dir = $dir/certificates
certificate = $dir/ca.crt
serial = $dir/serial
crlnumber = $dir/crlnumber
crl = $dir/crl.pem
private_key = $dir/ca.key
RANDFILE = $dir/.rand

x509_extensions = usr_cert
name_opt = ca_default
cert_opt = ca_default
default_days = 365
default_crl_days = 30
default_md = sha256
preserve = no
policy = policy_match

[ policy_match ]
countryName = optional
stateOrProvinceName = optional
organizationName = optional
organizationalUnitName = optional
commonName = supplied
emailAddress = optional

[ usr_cert ]
basicConstraints = CA:FALSE
nsComment = "CasVPS Internal CA Generated Certificate"
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid,issuer

[ server_cert ]
basicConstraints = CA:FALSE
nsCertType = server
nsComment = "CasVPS Internal CA Generated Server Certificate"
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid,issuer:always
keyUsage = critical, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
"#, self.ca_path.display());

        Ok(ca_config)
    }

    async fn generate_certificate(&self, domain: &str) -> Result<Certificate> {
        let key_path = self.cert_storage_path.join(format!("{}.key", domain));
        let csr_path = self.cert_storage_path.join(format!("{}.csr", domain));
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
        let subject = format!("/CN={}/O=CasVPS/OU=Internal", domain);

        let output = tokio::process::Command::new("openssl")
            .args(&[
                "req",
                "-new",
                "-key", key_path.to_str().unwrap(),
                "-out", csr_path.to_str().unwrap(),
                "-subj", &subject,
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to generate CSR: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        // Sign certificate with CA
        let ca_config_path = self.ca_path.join("ca.conf");
        let ca_cert_path = self.ca_path.join("ca.crt");
        let ca_key_path = self.ca_path.join("ca.key");

        let output = tokio::process::Command::new("openssl")
            .args(&[
                "ca",
                "-config", ca_config_path.to_str().unwrap(),
                "-in", csr_path.to_str().unwrap(),
                "-out", cert_path.to_str().unwrap(),
                "-cert", ca_cert_path.to_str().unwrap(),
                "-keyfile", ca_key_path.to_str().unwrap(),
                "-days", "365",
                "-batch", // Don't prompt for confirmation
                "-extensions", "server_cert",
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to sign certificate: {}",
                String::from_utf8_lossy(&output.stderr)));
        }

        // Clean up CSR file
        if csr_path.exists() {
            let _ = tokio::fs::remove_file(&csr_path).await;
        }

        // Create Certificate object
        let serial_number = self.extract_serial_number(&cert_path).await
            .unwrap_or_else(|_| "unknown".to_string());

        let certificate = Certificate {
            domain: domain.to_string(),
            cert_type: CertificateType::InternalCA,
            cert_path: cert_path.clone(),
            key_path: key_path.clone(),
            not_before: SystemTime::now(),
            not_after: SystemTime::now() + Duration::from_secs(365 * 24 * 3600), // 1 year
            issuer: "CasVPS Internal CA".to_string(),
            serial_number,
        };

        Ok(certificate)
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

    async fn extract_ca_subject(&self) -> Result<String> {
        let ca_cert_path = self.ca_path.join("ca.crt");

        let output = tokio::process::Command::new("openssl")
            .args(&[
                "x509",
                "-in", ca_cert_path.to_str().unwrap(),
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

    async fn extract_ca_expiry(&self) -> Result<SystemTime> {
        let ca_cert_path = self.ca_path.join("ca.crt");

        let output = tokio::process::Command::new("openssl")
            .args(&[
                "x509",
                "-in", ca_cert_path.to_str().unwrap(),
                "-noout",
                "-enddate"
            ])
            .output()
            .await?;

        if output.status.success() {
            let enddate_output = String::from_utf8_lossy(&output.stdout);
            if let Some(enddate) = enddate_output.strip_prefix("notAfter=") {
                // Parse date - this is simplified, production would use proper date parsing
                debug!("CA expires: {}", enddate.trim());
                // Return a default expiry for now
                return Ok(SystemTime::now() + Duration::from_secs(10 * 365 * 24 * 3600)); // 10 years
            }
        }

        Ok(SystemTime::now() + Duration::from_secs(365 * 24 * 3600)) // Default 1 year
    }

    async fn extract_ca_serial(&self) -> Result<String> {
        let ca_cert_path = self.ca_path.join("ca.crt");

        let output = tokio::process::Command::new("openssl")
            .args(&[
                "x509",
                "-in", ca_cert_path.to_str().unwrap(),
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

    async fn count_issued_certificates(&self) -> Result<usize> {
        let index_path = self.ca_path.join("index.txt");

        if !index_path.exists() {
            return Ok(0);
        }

        let index_content = tokio::fs::read_to_string(&index_path).await?;
        let count = index_content.lines()
            .filter(|line| !line.is_empty())
            .count();

        Ok(count)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CAInfo {
    pub subject: String,
    pub not_after: SystemTime,
    pub serial_number: String,
    pub issued_certificates: usize,
    pub ca_cert_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_internal_ca_creation() {
        let temp_dir = TempDir::new().unwrap();
        let ca_path = temp_dir.path().join("ca");

        let ca = InternalCA::new(&ca_path).await.unwrap();

        assert!(ca.ca_initialized);
        assert!(ca_path.join("ca.crt").exists());
        assert!(ca_path.join("ca.key").exists());
    }

    #[tokio::test]
    async fn test_ca_certificate_generation() {
        let temp_dir = TempDir::new().unwrap();
        let ca_path = temp_dir.path().join("ca");

        let ca = InternalCA::new(&ca_path).await.unwrap();

        let certificate = ca.issue_certificate("test.local").await.unwrap();

        assert_eq!(certificate.domain, "test.local");
        assert_eq!(certificate.cert_type, CertificateType::InternalCA);
        assert_eq!(certificate.issuer, "CasVPS Internal CA");
        assert!(certificate.cert_path.exists());
        assert!(certificate.key_path.exists());
    }

    #[tokio::test]
    async fn test_ca_info_extraction() {
        let temp_dir = TempDir::new().unwrap();
        let ca_path = temp_dir.path().join("ca");

        let ca = InternalCA::new(&ca_path).await.unwrap();

        let ca_info = ca.get_ca_info().await.unwrap();

        assert!(!ca_info.subject.is_empty());
        assert!(!ca_info.serial_number.is_empty());
        assert_eq!(ca_info.issued_certificates, 0);
    }
}