use anyhow::Result;
use std::collections::HashMap;
use std::time::SystemTime;
use async_trait::async_trait;
use tracing::{debug, warn};

use crate::database::Database;
use super::{ComplianceChecker, ComplianceStatus, ComplianceIssue, ComplianceRequirement};

pub struct GDPRChecker;

impl GDPRChecker {
    pub fn new() -> Self {
        Self
    }

    async fn check_data_retention_policies(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check if data retention policies are configured
        let retention_config = database.get_config("gdpr.data_retention")
            .await
            .map(|v| v.as_object().cloned())
            .unwrap_or_default();

        if retention_config.is_none() {
            issues.push(ComplianceIssue {
                id: "gdpr_retention_001".to_string(),
                severity: "high".to_string(),
                title: "No Data Retention Policy Configured".to_string(),
                description: "GDPR requires explicit data retention policies for personal data".to_string(),
                remediation: "Configure data retention policies in compliance settings".to_string(),
                detected_at: SystemTime::now(),
            });
        } else {
            let retention = retention_config.unwrap();

            // Check for excessive retention periods
            if let Some(default_retention) = retention.get("default_retention_days") {
                if let Some(days) = default_retention.as_u64() {
                    if days > 2555 {  // ~7 years
                        issues.push(ComplianceIssue {
                            id: "gdpr_retention_002".to_string(),
                            severity: "medium".to_string(),
                            title: "Excessive Data Retention Period".to_string(),
                            description: format!("Default retention of {} days may exceed GDPR requirements", days),
                            remediation: "Review and reduce retention periods to necessary minimum".to_string(),
                            detected_at: SystemTime::now(),
                        });
                    }
                }
            }
        }

        Ok(issues)
    }

    async fn check_data_minimization(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check if data minimization practices are in place
        let data_collection_config = database.get_config("gdpr.data_collection")
            .await
            .map(|v| v.as_object().cloned())
            .unwrap_or_default();

        if data_collection_config.is_none() {
            issues.push(ComplianceIssue {
                id: "gdpr_minimization_001".to_string(),
                severity: "medium".to_string(),
                title: "Data Minimization Policy Missing".to_string(),
                description: "GDPR requires data collection to be limited to what is necessary".to_string(),
                remediation: "Configure data collection policies to minimize personal data collection".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for PII in logs
        let log_settings = database.get_config("logging.gdpr_compliant")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !log_settings {
            issues.push(ComplianceIssue {
                id: "gdpr_minimization_002".to_string(),
                severity: "high".to_string(),
                title: "Potential PII in Logs".to_string(),
                description: "Logs may contain personal identifiable information".to_string(),
                remediation: "Enable GDPR-compliant logging to anonymize PII in logs".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_consent_management(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check if consent management is configured
        let consent_config = database.get_config("gdpr.consent_management")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !consent_config {
            issues.push(ComplianceIssue {
                id: "gdpr_consent_001".to_string(),
                severity: "critical".to_string(),
                title: "No Consent Management System".to_string(),
                description: "GDPR requires explicit consent for personal data processing".to_string(),
                remediation: "Implement consent management system for user data processing".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_right_to_erasure(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check if right to erasure is implemented
        let erasure_enabled = database.get_config("gdpr.right_to_erasure")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !erasure_enabled {
            issues.push(ComplianceIssue {
                id: "gdpr_erasure_001".to_string(),
                severity: "critical".to_string(),
                title: "Right to Erasure Not Implemented".to_string(),
                description: "GDPR requires ability for users to request data deletion".to_string(),
                remediation: "Implement right to erasure functionality in user management".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for data anonymization after erasure
        let anonymization_enabled = database.get_config("gdpr.anonymization_after_erasure")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !anonymization_enabled {
            issues.push(ComplianceIssue {
                id: "gdpr_erasure_002".to_string(),
                severity: "medium".to_string(),
                title: "Data Not Anonymized After Erasure".to_string(),
                description: "Data should be anonymized rather than just marked as deleted".to_string(),
                remediation: "Enable data anonymization after user data erasure requests".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_data_portability(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check if data portability is supported
        let portability_enabled = database.get_config("gdpr.data_portability")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !portability_enabled {
            issues.push(ComplianceIssue {
                id: "gdpr_portability_001".to_string(),
                severity: "medium".to_string(),
                title: "Data Portability Not Supported".to_string(),
                description: "GDPR grants users right to receive their personal data in portable format".to_string(),
                remediation: "Implement data export functionality for user data portability".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_data_protection_by_design(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check encryption at rest
        let encryption_at_rest = database.get_config("security.encryption_at_rest")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !encryption_at_rest {
            issues.push(ComplianceIssue {
                id: "gdpr_protection_001".to_string(),
                severity: "high".to_string(),
                title: "No Encryption at Rest".to_string(),
                description: "Personal data should be encrypted when stored".to_string(),
                remediation: "Enable database and file system encryption".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check encryption in transit
        let encryption_in_transit = database.get_config("security.encryption_in_transit")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(true);  // Assume HTTPS by default

        if !encryption_in_transit {
            issues.push(ComplianceIssue {
                id: "gdpr_protection_002".to_string(),
                severity: "critical".to_string(),
                title: "No Encryption in Transit".to_string(),
                description: "Personal data transmission must be encrypted".to_string(),
                remediation: "Enable HTTPS/TLS for all data transmissions".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_breach_notification(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check if breach notification system is configured
        let breach_notification = database.get_config("gdpr.breach_notification")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !breach_notification {
            issues.push(ComplianceIssue {
                id: "gdpr_breach_001".to_string(),
                severity: "high".to_string(),
                title: "No Breach Notification System".to_string(),
                description: "GDPR requires data breach notification within 72 hours".to_string(),
                remediation: "Configure automated breach detection and notification system".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }
}

#[async_trait]
impl ComplianceChecker for GDPRChecker {
    fn compliance_type(&self) -> &'static str {
        "gdpr"
    }

    async fn check_compliance(&self, database: &Database) -> Result<ComplianceStatus> {
        debug!("Running GDPR compliance check");

        let mut all_issues = Vec::new();

        // Run all GDPR compliance checks
        all_issues.extend(self.check_data_retention_policies(database).await?);
        all_issues.extend(self.check_data_minimization(database).await?);
        all_issues.extend(self.check_consent_management(database).await?);
        all_issues.extend(self.check_right_to_erasure(database).await?);
        all_issues.extend(self.check_data_portability(database).await?);
        all_issues.extend(self.check_data_protection_by_design(database).await?);
        all_issues.extend(self.check_breach_notification(database).await?);

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
            warn!("GDPR compliance issues found: {}", all_issues.len());
        }

        Ok(ComplianceStatus {
            compliance_type: "gdpr".to_string(),
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
                id: "gdpr_req_001".to_string(),
                title: "Lawful Basis for Processing".to_string(),
                description: "Personal data processing must have a lawful basis under GDPR Article 6".to_string(),
                mandatory: true,
                category: "legal_basis".to_string(),
            },
            ComplianceRequirement {
                id: "gdpr_req_002".to_string(),
                title: "Data Subject Rights".to_string(),
                description: "Implement all data subject rights (access, rectification, erasure, portability, etc.)".to_string(),
                mandatory: true,
                category: "data_subject_rights".to_string(),
            },
            ComplianceRequirement {
                id: "gdpr_req_003".to_string(),
                title: "Data Protection by Design and by Default".to_string(),
                description: "Implement appropriate technical and organizational measures".to_string(),
                mandatory: true,
                category: "data_protection".to_string(),
            },
            ComplianceRequirement {
                id: "gdpr_req_004".to_string(),
                title: "Data Retention Policies".to_string(),
                description: "Personal data must not be kept longer than necessary".to_string(),
                mandatory: true,
                category: "data_retention".to_string(),
            },
            ComplianceRequirement {
                id: "gdpr_req_005".to_string(),
                title: "Breach Notification".to_string(),
                description: "Report personal data breaches within 72 hours".to_string(),
                mandatory: true,
                category: "breach_notification".to_string(),
            },
            ComplianceRequirement {
                id: "gdpr_req_006".to_string(),
                title: "Data Minimization".to_string(),
                description: "Collect only personal data that is necessary for the purpose".to_string(),
                mandatory: true,
                category: "data_minimization".to_string(),
            },
        ]
    }

    async fn remediate_issue(&self, issue_id: &str, database: &Database) -> Result<()> {
        debug!("Attempting to remediate GDPR issue: {}", issue_id);

        match issue_id {
            "gdpr_retention_001" => {
                // Set default retention policy
                let retention_config = serde_json::json!({
                    "default_retention_days": 365,
                    "access_logs_retention_days": 90,
                    "audit_logs_retention_days": 2555,  // 7 years for audit
                    "user_data_retention_days": 730,  // 2 years
                });
                database.set_config("gdpr.data_retention", retention_config).await?;
            },
            "gdpr_minimization_002" => {
                // Enable GDPR-compliant logging
                database.set_config("logging.gdpr_compliant", serde_json::Value::Bool(true)).await?;
                database.set_config("logging.anonymize_pii", serde_json::Value::Bool(true)).await?;
            },
            "gdpr_consent_001" => {
                // Enable consent management
                database.set_config("gdpr.consent_management", serde_json::Value::Bool(true)).await?;
            },
            "gdpr_erasure_001" => {
                // Enable right to erasure
                database.set_config("gdpr.right_to_erasure", serde_json::Value::Bool(true)).await?;
                database.set_config("gdpr.anonymization_after_erasure", serde_json::Value::Bool(true)).await?;
            },
            "gdpr_portability_001" => {
                // Enable data portability
                database.set_config("gdpr.data_portability", serde_json::Value::Bool(true)).await?;
            },
            "gdpr_protection_001" => {
                // Enable encryption at rest
                database.set_config("security.encryption_at_rest", serde_json::Value::Bool(true)).await?;
            },
            "gdpr_protection_002" => {
                // Force HTTPS
                database.set_config("security.force_https", serde_json::Value::Bool(true)).await?;
                database.set_config("security.encryption_in_transit", serde_json::Value::Bool(true)).await?;
            },
            "gdpr_breach_001" => {
                // Enable breach notification
                database.set_config("gdpr.breach_notification", serde_json::Value::Bool(true)).await?;
                database.set_config("gdpr.breach_notification_hours", serde_json::Value::Number(72.into())).await?;
            },
            _ => {
                return Err(anyhow::anyhow!("Unknown GDPR issue ID: {}", issue_id));
            }
        }

        debug!("Successfully remediated GDPR issue: {}", issue_id);
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
    async fn test_gdpr_compliance_check() {
        let database = create_test_database().await;
        let checker = GDPRChecker::new();

        let status = checker.check_compliance(&database).await.unwrap();
        assert_eq!(status.compliance_type, "gdpr");

        // Should have issues since nothing is configured
        assert!(!status.issues.is_empty());
        assert_eq!(status.status, "non_compliant");
    }

    #[tokio::test]
    async fn test_gdpr_requirements() {
        let checker = GDPRChecker::new();
        let requirements = checker.get_requirements().await;

        assert!(!requirements.is_empty());
        assert!(requirements.iter().all(|r| r.mandatory));
    }

    #[tokio::test]
    async fn test_gdpr_remediation() {
        let database = create_test_database().await;
        let checker = GDPRChecker::new();

        // Test remediation
        checker.remediate_issue("gdpr_consent_001", &database).await.unwrap();

        // Verify configuration was set
        let consent_enabled = database.get_config("gdpr.consent_management").await.unwrap();
        assert!(consent_enabled.as_bool().unwrap());
    }
}