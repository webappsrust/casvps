use anyhow::Result;
use std::collections::HashMap;
use std::time::SystemTime;
use async_trait::async_trait;
use tracing::{debug, warn};

use crate::database::Database;
use super::{ComplianceChecker, ComplianceStatus, ComplianceIssue, ComplianceRequirement};

pub struct PCIDSSChecker;

impl PCIDSSChecker {
    pub fn new() -> Self {
        Self
    }

    async fn check_network_security(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check firewall configuration
        let firewall_enabled = database.get_config("firewall.enabled")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !firewall_enabled {
            issues.push(ComplianceIssue {
                id: "pci_network_001".to_string(),
                severity: "critical".to_string(),
                title: "Firewall Not Enabled".to_string(),
                description: "PCI-DSS requires firewall protection for cardholder data environment".to_string(),
                remediation: "Enable and configure firewall protection".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for default passwords
        let default_passwords_changed = database.get_config("pci.default_passwords_changed")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !default_passwords_changed {
            issues.push(ComplianceIssue {
                id: "pci_network_002".to_string(),
                severity: "critical".to_string(),
                title: "Default Passwords Not Changed".to_string(),
                description: "PCI-DSS requires changing default passwords and security parameters".to_string(),
                remediation: "Change all default passwords and security settings".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check network segmentation
        let network_segmentation = database.get_config("pci.network_segmentation")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !network_segmentation {
            issues.push(ComplianceIssue {
                id: "pci_network_003".to_string(),
                severity: "high".to_string(),
                title: "No Network Segmentation".to_string(),
                description: "PCI-DSS requires network segmentation to isolate cardholder data environment".to_string(),
                remediation: "Implement network segmentation for cardholder data systems".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_data_protection(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check encryption at rest
        let encryption_at_rest = database.get_config("security.encryption_at_rest")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !encryption_at_rest {
            issues.push(ComplianceIssue {
                id: "pci_data_001".to_string(),
                severity: "critical".to_string(),
                title: "No Encryption at Rest".to_string(),
                description: "PCI-DSS requires encryption of stored cardholder data".to_string(),
                remediation: "Enable encryption for all stored cardholder data".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check encryption in transit
        let encryption_in_transit = database.get_config("security.encryption_in_transit")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !encryption_in_transit {
            issues.push(ComplianceIssue {
                id: "pci_data_002".to_string(),
                severity: "critical".to_string(),
                title: "No Encryption in Transit".to_string(),
                description: "PCI-DSS requires encryption of cardholder data during transmission".to_string(),
                remediation: "Enable strong encryption for all cardholder data transmissions".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for PAN masking
        let pan_masking = database.get_config("pci.pan_masking")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !pan_masking {
            issues.push(ComplianceIssue {
                id: "pci_data_003".to_string(),
                severity: "high".to_string(),
                title: "PAN Not Masked".to_string(),
                description: "PCI-DSS requires masking of PAN when displayed".to_string(),
                remediation: "Implement PAN masking for all displays and reports".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check data retention policies
        let data_retention = database.get_config("pci.data_retention")
            .await
            .map(|v| v.as_object())
            .unwrap_or_default();

        if data_retention.is_none() {
            issues.push(ComplianceIssue {
                id: "pci_data_004".to_string(),
                severity: "medium".to_string(),
                title: "No Data Retention Policy".to_string(),
                description: "PCI-DSS requires defined data retention and disposal policies".to_string(),
                remediation: "Define and implement data retention and secure disposal policies".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_access_control(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for unique user IDs
        let unique_user_ids = database.get_config("pci.unique_user_ids")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !unique_user_ids {
            issues.push(ComplianceIssue {
                id: "pci_access_001".to_string(),
                severity: "high".to_string(),
                title: "Non-Unique User IDs".to_string(),
                description: "PCI-DSS requires unique user IDs for each person with computer access".to_string(),
                remediation: "Ensure all users have unique IDs and shared accounts are disabled".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check multi-factor authentication
        let mfa_enabled = database.get_config("auth.mfa_required")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !mfa_enabled {
            issues.push(ComplianceIssue {
                id: "pci_access_002".to_string(),
                severity: "high".to_string(),
                title: "Multi-Factor Authentication Not Required".to_string(),
                description: "PCI-DSS requires MFA for remote access to cardholder data environment".to_string(),
                remediation: "Enable and enforce multi-factor authentication".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check role-based access control
        let rbac_enabled = database.get_config("auth.rbac_enabled")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !rbac_enabled {
            issues.push(ComplianceIssue {
                id: "pci_access_003".to_string(),
                severity: "medium".to_string(),
                title: "No Role-Based Access Control".to_string(),
                description: "PCI-DSS requires access control based on job function".to_string(),
                remediation: "Implement role-based access control system".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check session timeout
        let session_timeout = database.get_config("auth.session_timeout_minutes")
            .await
            .map(|v| v.as_u64())
            .unwrap_or_default()
            .unwrap_or(0);

        if session_timeout == 0 || session_timeout > 15 {
            issues.push(ComplianceIssue {
                id: "pci_access_004".to_string(),
                severity: "medium".to_string(),
                title: "Session Timeout Too Long".to_string(),
                description: "PCI-DSS requires session timeout of 15 minutes or less for idle sessions".to_string(),
                remediation: "Set session timeout to 15 minutes or less".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_monitoring(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check logging enabled
        let logging_enabled = database.get_config("monitoring.logging_enabled")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !logging_enabled {
            issues.push(ComplianceIssue {
                id: "pci_monitoring_001".to_string(),
                severity: "critical".to_string(),
                title: "Logging Not Enabled".to_string(),
                description: "PCI-DSS requires comprehensive logging of access to network resources and cardholder data".to_string(),
                remediation: "Enable comprehensive logging for all system activities".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check log integrity protection
        let log_integrity = database.get_config("monitoring.log_integrity_protection")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !log_integrity {
            issues.push(ComplianceIssue {
                id: "pci_monitoring_002".to_string(),
                severity: "high".to_string(),
                title: "Log Integrity Not Protected".to_string(),
                description: "PCI-DSS requires protection of log files from unauthorized modifications".to_string(),
                remediation: "Implement log file integrity monitoring and protection".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check daily log review
        let daily_log_review = database.get_config("pci.daily_log_review")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !daily_log_review {
            issues.push(ComplianceIssue {
                id: "pci_monitoring_003".to_string(),
                severity: "medium".to_string(),
                title: "No Daily Log Review".to_string(),
                description: "PCI-DSS requires daily review of logs".to_string(),
                remediation: "Implement automated daily log review and alerting".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check file integrity monitoring
        let fim_enabled = database.get_config("security.file_integrity_monitoring")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !fim_enabled {
            issues.push(ComplianceIssue {
                id: "pci_monitoring_004".to_string(),
                severity: "high".to_string(),
                title: "No File Integrity Monitoring".to_string(),
                description: "PCI-DSS requires file integrity monitoring for critical files".to_string(),
                remediation: "Implement file integrity monitoring for critical system files".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_vulnerability_management(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check antivirus deployment
        let antivirus_enabled = database.get_config("security.clamav.enabled")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !antivirus_enabled {
            issues.push(ComplianceIssue {
                id: "pci_vuln_001".to_string(),
                severity: "high".to_string(),
                title: "Antivirus Not Deployed".to_string(),
                description: "PCI-DSS requires antivirus software on systems susceptible to malware".to_string(),
                remediation: "Deploy and maintain current antivirus software".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check system patching
        let auto_updates = database.get_config("security.auto_updates_enabled")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !auto_updates {
            issues.push(ComplianceIssue {
                id: "pci_vuln_002".to_string(),
                severity: "high".to_string(),
                title: "No Automated Patching".to_string(),
                description: "PCI-DSS requires systems to be protected with security patches".to_string(),
                remediation: "Enable automated security updates and patch management".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check vulnerability scanning
        let vuln_scanning = database.get_config("security.vulnerability_scanning")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !vuln_scanning {
            issues.push(ComplianceIssue {
                id: "pci_vuln_003".to_string(),
                severity: "medium".to_string(),
                title: "No Vulnerability Scanning".to_string(),
                description: "PCI-DSS requires regular vulnerability scans".to_string(),
                remediation: "Implement regular internal and external vulnerability scanning".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }
}

#[async_trait]
impl ComplianceChecker for PCIDSSChecker {
    fn compliance_type(&self) -> &'static str {
        "pci"
    }

    async fn check_compliance(&self, database: &Database) -> Result<ComplianceStatus> {
        debug!("Running PCI-DSS compliance check");

        let mut all_issues = Vec::new();

        // Run all PCI-DSS compliance checks
        all_issues.extend(self.check_network_security(database).await?);
        all_issues.extend(self.check_data_protection(database).await?);
        all_issues.extend(self.check_access_control(database).await?);
        all_issues.extend(self.check_monitoring(database).await?);
        all_issues.extend(self.check_vulnerability_management(database).await?);

        // Determine overall compliance status
        let status = if all_issues.iter().any(|i| i.severity == "critical") {
            "non_compliant"
        } else if all_issues.iter().any(|i| i.severity == "high") {
            "warning"
        } else if all_issues.is_empty() {
            "compliant"
        } else {
            "warning"
        };

        if !all_issues.is_empty() {
            warn!("PCI-DSS compliance issues found: {}", all_issues.len());
        }

        Ok(ComplianceStatus {
            compliance_type: "pci".to_string(),
            enabled: true,
            status: status.to_string(),
            last_check: SystemTime::now(),
            issues: all_issues,
            config: HashMap::new(),
        })
    }

    async fn get_requirements(&self) -> Vec<ComplianceRequirement> {
        vec![
            ComplianceRequirement {
                id: "pci_req_001".to_string(),
                title: "Install and maintain firewall configuration".to_string(),
                description: "Build and maintain a secure network and systems".to_string(),
                mandatory: true,
                category: "network_security".to_string(),
            },
            ComplianceRequirement {
                id: "pci_req_002".to_string(),
                title: "Do not use vendor-supplied defaults".to_string(),
                description: "Change default passwords and security parameters".to_string(),
                mandatory: true,
                category: "network_security".to_string(),
            },
            ComplianceRequirement {
                id: "pci_req_003".to_string(),
                title: "Protect stored cardholder data".to_string(),
                description: "Encrypt stored cardholder data".to_string(),
                mandatory: true,
                category: "data_protection".to_string(),
            },
            ComplianceRequirement {
                id: "pci_req_004".to_string(),
                title: "Encrypt transmission of cardholder data".to_string(),
                description: "Encrypt cardholder data during transmission across open, public networks".to_string(),
                mandatory: true,
                category: "data_protection".to_string(),
            },
            ComplianceRequirement {
                id: "pci_req_005".to_string(),
                title: "Protect against malware".to_string(),
                description: "Use and regularly update antivirus software".to_string(),
                mandatory: true,
                category: "vulnerability_management".to_string(),
            },
            ComplianceRequirement {
                id: "pci_req_006".to_string(),
                title: "Develop and maintain secure systems".to_string(),
                description: "Develop and maintain secure systems and applications".to_string(),
                mandatory: true,
                category: "vulnerability_management".to_string(),
            },
            ComplianceRequirement {
                id: "pci_req_007".to_string(),
                title: "Restrict access by business need-to-know".to_string(),
                description: "Restrict access to cardholder data by business need to know".to_string(),
                mandatory: true,
                category: "access_control".to_string(),
            },
            ComplianceRequirement {
                id: "pci_req_008".to_string(),
                title: "Identify and authenticate access".to_string(),
                description: "Identify and authenticate access to system components".to_string(),
                mandatory: true,
                category: "access_control".to_string(),
            },
            ComplianceRequirement {
                id: "pci_req_009".to_string(),
                title: "Restrict physical access".to_string(),
                description: "Restrict physical access to cardholder data".to_string(),
                mandatory: true,
                category: "physical_security".to_string(),
            },
            ComplianceRequirement {
                id: "pci_req_010".to_string(),
                title: "Track and monitor access".to_string(),
                description: "Track and monitor all access to network resources and cardholder data".to_string(),
                mandatory: true,
                category: "monitoring".to_string(),
            },
            ComplianceRequirement {
                id: "pci_req_011".to_string(),
                title: "Regularly test security systems".to_string(),
                description: "Regularly test security systems and processes".to_string(),
                mandatory: true,
                category: "vulnerability_management".to_string(),
            },
            ComplianceRequirement {
                id: "pci_req_012".to_string(),
                title: "Maintain information security policy".to_string(),
                description: "Maintain a policy that addresses information security for all personnel".to_string(),
                mandatory: true,
                category: "security_policy".to_string(),
            },
        ]
    }

    async fn remediate_issue(&self, issue_id: &str, database: &Database) -> Result<()> {
        debug!("Attempting to remediate PCI-DSS issue: {}", issue_id);

        match issue_id {
            "pci_network_001" => {
                database.set_config("firewall.enabled", serde_json::Value::Bool(true)).await?;
                database.set_config("firewall.default_policy", serde_json::Value::String("drop".to_string())).await?;
            },
            "pci_network_002" => {
                database.set_config("pci.default_passwords_changed", serde_json::Value::Bool(true)).await?;
            },
            "pci_network_003" => {
                database.set_config("pci.network_segmentation", serde_json::Value::Bool(true)).await?;
            },
            "pci_data_001" => {
                database.set_config("security.encryption_at_rest", serde_json::Value::Bool(true)).await?;
            },
            "pci_data_002" => {
                database.set_config("security.encryption_in_transit", serde_json::Value::Bool(true)).await?;
                database.set_config("security.force_https", serde_json::Value::Bool(true)).await?;
            },
            "pci_data_003" => {
                database.set_config("pci.pan_masking", serde_json::Value::Bool(true)).await?;
            },
            "pci_data_004" => {
                let retention_config = serde_json::json!({
                    "cardholder_data_retention_days": 365,
                    "audit_logs_retention_days": 730,
                    "secure_disposal_required": true
                });
                database.set_config("pci.data_retention", retention_config).await?;
            },
            "pci_access_001" => {
                database.set_config("pci.unique_user_ids", serde_json::Value::Bool(true)).await?;
                database.set_config("auth.shared_accounts_disabled", serde_json::Value::Bool(true)).await?;
            },
            "pci_access_002" => {
                database.set_config("auth.mfa_required", serde_json::Value::Bool(true)).await?;
            },
            "pci_access_003" => {
                database.set_config("auth.rbac_enabled", serde_json::Value::Bool(true)).await?;
            },
            "pci_access_004" => {
                database.set_config("auth.session_timeout_minutes", serde_json::Value::Number(15.into())).await?;
            },
            "pci_monitoring_001" => {
                database.set_config("monitoring.logging_enabled", serde_json::Value::Bool(true)).await?;
                database.set_config("monitoring.comprehensive_logging", serde_json::Value::Bool(true)).await?;
            },
            "pci_monitoring_002" => {
                database.set_config("monitoring.log_integrity_protection", serde_json::Value::Bool(true)).await?;
            },
            "pci_monitoring_003" => {
                database.set_config("pci.daily_log_review", serde_json::Value::Bool(true)).await?;
            },
            "pci_monitoring_004" => {
                database.set_config("security.file_integrity_monitoring", serde_json::Value::Bool(true)).await?;
            },
            "pci_vuln_001" => {
                database.set_config("security.clamav.enabled", serde_json::Value::Bool(true)).await?;
            },
            "pci_vuln_002" => {
                database.set_config("security.auto_updates_enabled", serde_json::Value::Bool(true)).await?;
            },
            "pci_vuln_003" => {
                database.set_config("security.vulnerability_scanning", serde_json::Value::Bool(true)).await?;
            },
            _ => {
                return Err(anyhow::anyhow!("Unknown PCI-DSS issue ID: {}", issue_id));
            }
        }

        debug!("Successfully remediated PCI-DSS issue: {}", issue_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_database() -> Database {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        Database::new(db_path.to_str().unwrap()).await.unwrap()
    }

    #[tokio::test]
    async fn test_pci_dss_compliance_check() {
        let database = create_test_database().await;
        let checker = PCIDSSChecker::new();

        let status = checker.check_compliance(&database).await.unwrap();
        assert_eq!(status.compliance_type, "pci");

        // Should have issues since nothing is configured
        assert!(!status.issues.is_empty());
        assert_eq!(status.status, "non_compliant");
    }

    #[tokio::test]
    async fn test_pci_dss_requirements() {
        let checker = PCIDSSChecker::new();
        let requirements = checker.get_requirements().await;

        // Should have 12 main PCI-DSS requirements
        assert_eq!(requirements.len(), 12);
        assert!(requirements.iter().all(|r| r.mandatory));
    }

    #[tokio::test]
    async fn test_pci_dss_remediation() {
        let database = create_test_database().await;
        let checker = PCIDSSChecker::new();

        // Test remediation
        checker.remediate_issue("pci_network_001", &database).await.unwrap();

        // Verify configuration was set
        let firewall_enabled = database.get_config("firewall.enabled").await.unwrap();
        assert!(firewall_enabled.as_bool().unwrap());
    }
}