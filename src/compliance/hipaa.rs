use anyhow::Result;
use std::collections::HashMap;
use std::time::SystemTime;
use async_trait::async_trait;
use tracing::{debug, warn};

use crate::database::Database;
use super::{ComplianceChecker, ComplianceStatus, ComplianceIssue, ComplianceRequirement};

pub struct HIPAAChecker;

impl HIPAAChecker {
    pub fn new() -> Self {
        Self
    }

    async fn check_administrative_safeguards(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for assigned security responsibility
        let security_officer_assigned = database.get_config("hipaa.security_officer_assigned")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !security_officer_assigned {
            issues.push(ComplianceIssue {
                id: "hipaa_admin_001".to_string(),
                severity: "critical".to_string(),
                title: "No Assigned Security Officer".to_string(),
                description: "HIPAA requires a designated security officer for covered entities".to_string(),
                remediation: "Assign a security officer responsible for HIPAA compliance".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for workforce training
        let workforce_training = database.get_config("hipaa.workforce_training")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !workforce_training {
            issues.push(ComplianceIssue {
                id: "hipaa_admin_002".to_string(),
                severity: "high".to_string(),
                title: "No HIPAA Workforce Training".to_string(),
                description: "HIPAA requires security awareness training for workforce members".to_string(),
                remediation: "Implement HIPAA security awareness training program".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for access management procedures
        let access_management = database.get_config("hipaa.access_management_procedures")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !access_management {
            issues.push(ComplianceIssue {
                id: "hipaa_admin_003".to_string(),
                severity: "high".to_string(),
                title: "No Access Management Procedures".to_string(),
                description: "HIPAA requires procedures for granting access to PHI".to_string(),
                remediation: "Implement access management procedures and documentation".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for business associate agreements
        let baa_in_place = database.get_config("hipaa.business_associate_agreements")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !baa_in_place {
            issues.push(ComplianceIssue {
                id: "hipaa_admin_004".to_string(),
                severity: "critical".to_string(),
                title: "No Business Associate Agreements".to_string(),
                description: "HIPAA requires BAAs with vendors who handle PHI".to_string(),
                remediation: "Execute business associate agreements with all relevant vendors".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_physical_safeguards(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for facility access controls
        let facility_access_controls = database.get_config("hipaa.facility_access_controls")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !facility_access_controls {
            issues.push(ComplianceIssue {
                id: "hipaa_physical_001".to_string(),
                severity: "high".to_string(),
                title: "No Facility Access Controls".to_string(),
                description: "HIPAA requires physical controls to limit access to facilities containing PHI".to_string(),
                remediation: "Implement facility access controls and monitoring".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for workstation use controls
        let workstation_controls = database.get_config("hipaa.workstation_use_controls")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !workstation_controls {
            issues.push(ComplianceIssue {
                id: "hipaa_physical_002".to_string(),
                severity: "medium".to_string(),
                title: "No Workstation Use Controls".to_string(),
                description: "HIPAA requires controls on workstation use and access to PHI".to_string(),
                remediation: "Implement workstation use policies and technical controls".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for device and media controls
        let device_media_controls = database.get_config("hipaa.device_media_controls")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !device_media_controls {
            issues.push(ComplianceIssue {
                id: "hipaa_physical_003".to_string(),
                severity: "medium".to_string(),
                title: "No Device and Media Controls".to_string(),
                description: "HIPAA requires controls for receipt/removal of hardware and electronic media".to_string(),
                remediation: "Implement device and media control procedures".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_technical_safeguards(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for access control to PHI
        let access_control_enabled = database.get_config("auth.rbac_enabled")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !access_control_enabled {
            issues.push(ComplianceIssue {
                id: "hipaa_technical_001".to_string(),
                severity: "critical".to_string(),
                title: "No Access Control System".to_string(),
                description: "HIPAA requires unique user identification and access control to PHI".to_string(),
                remediation: "Implement role-based access control system".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for audit controls
        let audit_controls = database.get_config("monitoring.audit_logging_enabled")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !audit_controls {
            issues.push(ComplianceIssue {
                id: "hipaa_technical_002".to_string(),
                severity: "critical".to_string(),
                title: "No Audit Controls".to_string(),
                description: "HIPAA requires audit logs for systems containing PHI".to_string(),
                remediation: "Enable comprehensive audit logging for all PHI access".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for data integrity controls
        let data_integrity = database.get_config("hipaa.data_integrity_controls")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !data_integrity {
            issues.push(ComplianceIssue {
                id: "hipaa_technical_003".to_string(),
                severity: "high".to_string(),
                title: "No Data Integrity Controls".to_string(),
                description: "HIPAA requires protection of PHI from improper alteration or destruction".to_string(),
                remediation: "Implement data integrity controls and checksums".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for transmission security
        let transmission_security = database.get_config("security.encryption_in_transit")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !transmission_security {
            issues.push(ComplianceIssue {
                id: "hipaa_technical_004".to_string(),
                severity: "critical".to_string(),
                title: "No Transmission Security".to_string(),
                description: "HIPAA requires encryption of PHI during transmission".to_string(),
                remediation: "Enable end-to-end encryption for all PHI transmissions".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for encryption at rest
        let encryption_at_rest = database.get_config("security.encryption_at_rest")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !encryption_at_rest {
            issues.push(ComplianceIssue {
                id: "hipaa_technical_005".to_string(),
                severity: "critical".to_string(),
                title: "No Encryption at Rest".to_string(),
                description: "HIPAA requires encryption of stored PHI".to_string(),
                remediation: "Enable encryption for all stored PHI".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for automatic logoff
        let automatic_logoff = database.get_config("auth.session_timeout_minutes")
            .await
            .map(|v| v.as_u64())
            .unwrap_or_default()
            .unwrap_or(0);

        if automatic_logoff == 0 || automatic_logoff > 30 {
            issues.push(ComplianceIssue {
                id: "hipaa_technical_006".to_string(),
                severity: "medium".to_string(),
                title: "No Automatic Logoff".to_string(),
                description: "HIPAA requires automatic logoff from systems containing PHI".to_string(),
                remediation: "Configure automatic session timeout of 30 minutes or less".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_breach_notification(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check breach notification procedures
        let breach_notification = database.get_config("hipaa.breach_notification_procedures")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !breach_notification {
            issues.push(ComplianceIssue {
                id: "hipaa_breach_001".to_string(),
                severity: "critical".to_string(),
                title: "No Breach Notification Procedures".to_string(),
                description: "HIPAA requires breach notification procedures and 60-day notification requirement".to_string(),
                remediation: "Implement breach notification procedures and automated monitoring".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for breach detection capabilities
        let breach_detection = database.get_config("security.intrusion_detection")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !breach_detection {
            issues.push(ComplianceIssue {
                id: "hipaa_breach_002".to_string(),
                severity: "high".to_string(),
                title: "No Breach Detection System".to_string(),
                description: "Systems containing PHI should have breach detection capabilities".to_string(),
                remediation: "Implement intrusion detection and monitoring systems".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_minimum_necessary(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check minimum necessary policies
        let minimum_necessary = database.get_config("hipaa.minimum_necessary_policies")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !minimum_necessary {
            issues.push(ComplianceIssue {
                id: "hipaa_minimum_001".to_string(),
                severity: "high".to_string(),
                title: "No Minimum Necessary Policies".to_string(),
                description: "HIPAA requires minimum necessary standard for PHI use and disclosure".to_string(),
                remediation: "Implement minimum necessary policies and access controls".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }
}

#[async_trait]
impl ComplianceChecker for HIPAAChecker {
    fn compliance_type(&self) -> &'static str {
        "hipaa"
    }

    async fn check_compliance(&self, database: &Database) -> Result<ComplianceStatus> {
        debug!("Running HIPAA compliance check");

        let mut all_issues = Vec::new();

        // Run all HIPAA compliance checks
        all_issues.extend(self.check_administrative_safeguards(database).await?);
        all_issues.extend(self.check_physical_safeguards(database).await?);
        all_issues.extend(self.check_technical_safeguards(database).await?);
        all_issues.extend(self.check_breach_notification(database).await?);
        all_issues.extend(self.check_minimum_necessary(database).await?);

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
            warn!("HIPAA compliance issues found: {}", all_issues.len());
        }

        Ok(ComplianceStatus {
            compliance_type: "hipaa".to_string(),
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
                id: "hipaa_req_001".to_string(),
                title: "Administrative Safeguards".to_string(),
                description: "Assign security responsibility and implement access management".to_string(),
                mandatory: true,
                category: "administrative".to_string(),
            },
            ComplianceRequirement {
                id: "hipaa_req_002".to_string(),
                title: "Physical Safeguards".to_string(),
                description: "Control physical access to facilities and workstations".to_string(),
                mandatory: true,
                category: "physical".to_string(),
            },
            ComplianceRequirement {
                id: "hipaa_req_003".to_string(),
                title: "Technical Safeguards".to_string(),
                description: "Implement access control, audit controls, integrity, and transmission security".to_string(),
                mandatory: true,
                category: "technical".to_string(),
            },
            ComplianceRequirement {
                id: "hipaa_req_004".to_string(),
                title: "Breach Notification Rule".to_string(),
                description: "Notify individuals, HHS, and media of breaches within 60 days".to_string(),
                mandatory: true,
                category: "breach_notification".to_string(),
            },
            ComplianceRequirement {
                id: "hipaa_req_005".to_string(),
                title: "Minimum Necessary Standard".to_string(),
                description: "Limit PHI use and disclosure to minimum necessary".to_string(),
                mandatory: true,
                category: "minimum_necessary".to_string(),
            },
            ComplianceRequirement {
                id: "hipaa_req_006".to_string(),
                title: "Business Associate Agreements".to_string(),
                description: "Execute BAAs with vendors handling PHI".to_string(),
                mandatory: true,
                category: "business_associates".to_string(),
            },
        ]
    }

    async fn remediate_issue(&self, issue_id: &str, database: &Database) -> Result<()> {
        debug!("Attempting to remediate HIPAA issue: {}", issue_id);

        match issue_id {
            "hipaa_admin_001" => {
                database.set_config("hipaa.security_officer_assigned", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_admin_002" => {
                database.set_config("hipaa.workforce_training", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_admin_003" => {
                database.set_config("hipaa.access_management_procedures", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_admin_004" => {
                database.set_config("hipaa.business_associate_agreements", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_physical_001" => {
                database.set_config("hipaa.facility_access_controls", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_physical_002" => {
                database.set_config("hipaa.workstation_use_controls", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_physical_003" => {
                database.set_config("hipaa.device_media_controls", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_technical_001" => {
                database.set_config("auth.rbac_enabled", serde_json::Value::Bool(true)).await?;
                database.set_config("auth.unique_user_ids", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_technical_002" => {
                database.set_config("monitoring.audit_logging_enabled", serde_json::Value::Bool(true)).await?;
                database.set_config("monitoring.comprehensive_audit_logging", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_technical_003" => {
                database.set_config("hipaa.data_integrity_controls", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_technical_004" => {
                database.set_config("security.encryption_in_transit", serde_json::Value::Bool(true)).await?;
                database.set_config("security.force_https", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_technical_005" => {
                database.set_config("security.encryption_at_rest", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_technical_006" => {
                database.set_config("auth.session_timeout_minutes", serde_json::Value::Number(30.into())).await?;
                database.set_config("auth.automatic_logoff_enabled", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_breach_001" => {
                database.set_config("hipaa.breach_notification_procedures", serde_json::Value::Bool(true)).await?;
                database.set_config("hipaa.breach_notification_days", serde_json::Value::Number(60.into())).await?;
            },
            "hipaa_breach_002" => {
                database.set_config("security.intrusion_detection", serde_json::Value::Bool(true)).await?;
            },
            "hipaa_minimum_001" => {
                database.set_config("hipaa.minimum_necessary_policies", serde_json::Value::Bool(true)).await?;
            },
            _ => {
                return Err(anyhow::anyhow!("Unknown HIPAA issue ID: {}", issue_id));
            }
        }

        debug!("Successfully remediated HIPAA issue: {}", issue_id);
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
    async fn test_hipaa_compliance_check() {
        let database = create_test_database().await;
        let checker = HIPAAChecker::new();

        let status = checker.check_compliance(&database).await.unwrap();
        assert_eq!(status.compliance_type, "hipaa");

        // Should have issues since nothing is configured
        assert!(!status.issues.is_empty());
        assert_eq!(status.status, "non_compliant");
    }

    #[tokio::test]
    async fn test_hipaa_requirements() {
        let checker = HIPAAChecker::new();
        let requirements = checker.get_requirements().await;

        assert!(!requirements.is_empty());
        assert!(requirements.iter().all(|r| r.mandatory));
    }

    #[tokio::test]
    async fn test_hipaa_remediation() {
        let database = create_test_database().await;
        let checker = HIPAAChecker::new();

        // Test remediation
        checker.remediate_issue("hipaa_admin_001", &database).await.unwrap();

        // Verify configuration was set
        let security_officer = database.get_config("hipaa.security_officer_assigned").await.unwrap();
        assert!(security_officer.as_bool().unwrap());
    }
}