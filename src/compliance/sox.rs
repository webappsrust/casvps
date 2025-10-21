use anyhow::Result;
use std::collections::HashMap;
use std::time::SystemTime;
use async_trait::async_trait;
use tracing::{debug, warn};

use crate::database::Database;
use super::{ComplianceChecker, ComplianceStatus, ComplianceIssue, ComplianceRequirement};

pub struct SOXChecker;

impl SOXChecker {
    pub fn new() -> Self {
        Self
    }

    async fn check_financial_reporting_controls(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for financial data identification
        let financial_data_classified = database.get_config("sox.financial_data_classified")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !financial_data_classified {
            issues.push(ComplianceIssue {
                id: "sox_reporting_001".to_string(),
                severity: "critical".to_string(),
                title: "Financial Data Not Classified".to_string(),
                description: "SOX requires identification and classification of financial data".to_string(),
                remediation: "Implement financial data classification and labeling".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for change management controls
        let change_management = database.get_config("sox.change_management_controls")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !change_management {
            issues.push(ComplianceIssue {
                id: "sox_reporting_002".to_string(),
                severity: "high".to_string(),
                title: "No Change Management Controls".to_string(),
                description: "SOX requires controls over changes to financial systems".to_string(),
                remediation: "Implement change management procedures for financial systems".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for segregation of duties
        let segregation_of_duties = database.get_config("sox.segregation_of_duties")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !segregation_of_duties {
            issues.push(ComplianceIssue {
                id: "sox_reporting_003".to_string(),
                severity: "high".to_string(),
                title: "No Segregation of Duties".to_string(),
                description: "SOX requires separation of incompatible duties in financial processes".to_string(),
                remediation: "Implement segregation of duties controls".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_audit_trail_requirements(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check comprehensive audit logging
        let comprehensive_logging = database.get_config("monitoring.comprehensive_audit_logging")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !comprehensive_logging {
            issues.push(ComplianceIssue {
                id: "sox_audit_001".to_string(),
                severity: "critical".to_string(),
                title: "Insufficient Audit Logging".to_string(),
                description: "SOX requires comprehensive audit trails for all financial data access".to_string(),
                remediation: "Enable comprehensive audit logging for all financial systems".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check audit log retention (7 years requirement)
        let log_retention_days = database.get_config("sox.audit_log_retention_days")
            .await
            .map(|v| v.as_u64())
            .unwrap_or_default()
            .unwrap_or(0);

        if log_retention_days < 2555 {  // 7 years = ~2555 days
            issues.push(ComplianceIssue {
                id: "sox_audit_002".to_string(),
                severity: "critical".to_string(),
                title: "Insufficient Audit Log Retention".to_string(),
                description: "SOX requires audit log retention for 7 years".to_string(),
                remediation: "Configure audit log retention for minimum 7 years (2555 days)".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check audit log integrity protection
        let log_integrity = database.get_config("monitoring.log_integrity_protection")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !log_integrity {
            issues.push(ComplianceIssue {
                id: "sox_audit_003".to_string(),
                severity: "high".to_string(),
                title: "Audit Logs Not Protected".to_string(),
                description: "SOX requires protection of audit logs from tampering".to_string(),
                remediation: "Implement audit log integrity protection and immutability".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for audit trail completeness
        let audit_trail_complete = database.get_config("sox.audit_trail_completeness")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !audit_trail_complete {
            issues.push(ComplianceIssue {
                id: "sox_audit_004".to_string(),
                severity: "high".to_string(),
                title: "Incomplete Audit Trails".to_string(),
                description: "SOX requires complete audit trails for all financial transactions".to_string(),
                remediation: "Ensure all financial data access and changes are logged".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_access_controls(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check role-based access control
        let rbac_enabled = database.get_config("auth.rbac_enabled")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !rbac_enabled {
            issues.push(ComplianceIssue {
                id: "sox_access_001".to_string(),
                severity: "critical".to_string(),
                title: "No Role-Based Access Control".to_string(),
                description: "SOX requires proper access controls for financial systems".to_string(),
                remediation: "Implement role-based access control for all financial data".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for periodic access reviews
        let access_reviews = database.get_config("sox.periodic_access_reviews")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !access_reviews {
            issues.push(ComplianceIssue {
                id: "sox_access_002".to_string(),
                severity: "high".to_string(),
                title: "No Periodic Access Reviews".to_string(),
                description: "SOX requires regular review of user access to financial systems".to_string(),
                remediation: "Implement periodic access review procedures".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for privileged access management
        let privileged_access_mgmt = database.get_config("sox.privileged_access_management")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !privileged_access_mgmt {
            issues.push(ComplianceIssue {
                id: "sox_access_003".to_string(),
                severity: "high".to_string(),
                title: "No Privileged Access Management".to_string(),
                description: "SOX requires controls over privileged access to financial systems".to_string(),
                remediation: "Implement privileged access management controls".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_data_retention(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check financial data retention policy
        let retention_policy = database.get_config("sox.data_retention_policy")
            .await
            .map(|v| v.as_object())
            .unwrap_or_default();

        if retention_policy.is_none() {
            issues.push(ComplianceIssue {
                id: "sox_retention_001".to_string(),
                severity: "critical".to_string(),
                title: "No Data Retention Policy".to_string(),
                description: "SOX requires documented data retention policies for financial records".to_string(),
                remediation: "Implement comprehensive data retention policy for financial data".to_string(),
                detected_at: SystemTime::now(),
            });
        } else {
            let policy = retention_policy.unwrap();

            // Check for 7-year retention requirement
            if let Some(financial_retention) = policy.get("financial_records_retention_days") {
                if let Some(days) = financial_retention.as_u64() {
                    if days < 2555 {  // 7 years
                        issues.push(ComplianceIssue {
                            id: "sox_retention_002".to_string(),
                            severity: "critical".to_string(),
                            title: "Insufficient Financial Data Retention".to_string(),
                            description: "SOX requires financial records retention for 7 years".to_string(),
                            remediation: "Increase financial data retention to minimum 7 years".to_string(),
                            detected_at: SystemTime::now(),
                        });
                    }
                }
            }
        }

        // Check secure disposal procedures
        let secure_disposal = database.get_config("sox.secure_disposal_procedures")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !secure_disposal {
            issues.push(ComplianceIssue {
                id: "sox_retention_003".to_string(),
                severity: "medium".to_string(),
                title: "No Secure Disposal Procedures".to_string(),
                description: "SOX requires secure disposal of financial data after retention period".to_string(),
                remediation: "Implement secure data disposal procedures".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_backup_and_recovery(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check backup procedures for financial data
        let financial_backup = database.get_config("sox.financial_data_backup")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !financial_backup {
            issues.push(ComplianceIssue {
                id: "sox_backup_001".to_string(),
                severity: "high".to_string(),
                title: "No Financial Data Backup".to_string(),
                description: "SOX requires backup procedures for financial data".to_string(),
                remediation: "Implement comprehensive backup procedures for financial systems".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check disaster recovery procedures
        let disaster_recovery = database.get_config("sox.disaster_recovery_procedures")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !disaster_recovery {
            issues.push(ComplianceIssue {
                id: "sox_backup_002".to_string(),
                severity: "high".to_string(),
                title: "No Disaster Recovery Procedures".to_string(),
                description: "SOX requires disaster recovery plans for financial systems".to_string(),
                remediation: "Develop and test disaster recovery procedures".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }
}

#[async_trait]
impl ComplianceChecker for SOXChecker {
    fn compliance_type(&self) -> &'static str {
        "sox"
    }

    async fn check_compliance(&self, database: &Database) -> Result<ComplianceStatus> {
        debug!("Running SOX compliance check");

        let mut all_issues = Vec::new();

        // Run all SOX compliance checks
        all_issues.extend(self.check_financial_reporting_controls(database).await?);
        all_issues.extend(self.check_audit_trail_requirements(database).await?);
        all_issues.extend(self.check_access_controls(database).await?);
        all_issues.extend(self.check_data_retention(database).await?);
        all_issues.extend(self.check_backup_and_recovery(database).await?);

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
            warn!("SOX compliance issues found: {}", all_issues.len());
        }

        Ok(ComplianceStatus {
            compliance_type: "sox".to_string(),
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
                id: "sox_req_001".to_string(),
                title: "Internal Controls Over Financial Reporting".to_string(),
                description: "Establish and maintain adequate internal control over financial reporting".to_string(),
                mandatory: true,
                category: "internal_controls".to_string(),
            },
            ComplianceRequirement {
                id: "sox_req_002".to_string(),
                title: "Management Assessment".to_string(),
                description: "Management must assess effectiveness of internal controls".to_string(),
                mandatory: true,
                category: "management_assessment".to_string(),
            },
            ComplianceRequirement {
                id: "sox_req_003".to_string(),
                title: "Auditor Attestation".to_string(),
                description: "External auditor must attest to management's assessment".to_string(),
                mandatory: true,
                category: "auditor_attestation".to_string(),
            },
            ComplianceRequirement {
                id: "sox_req_004".to_string(),
                title: "Financial Record Retention".to_string(),
                description: "Retain financial records for 7 years".to_string(),
                mandatory: true,
                category: "record_retention".to_string(),
            },
            ComplianceRequirement {
                id: "sox_req_005".to_string(),
                title: "Audit Trail Integrity".to_string(),
                description: "Maintain complete and tamper-proof audit trails".to_string(),
                mandatory: true,
                category: "audit_trails".to_string(),
            },
            ComplianceRequirement {
                id: "sox_req_006".to_string(),
                title: "Access Controls".to_string(),
                description: "Implement appropriate access controls for financial systems".to_string(),
                mandatory: true,
                category: "access_controls".to_string(),
            },
        ]
    }

    async fn remediate_issue(&self, issue_id: &str, database: &Database) -> Result<()> {
        debug!("Attempting to remediate SOX issue: {}", issue_id);

        match issue_id {
            "sox_reporting_001" => {
                database.set_config("sox.financial_data_classified", serde_json::Value::Bool(true)).await?;
            },
            "sox_reporting_002" => {
                database.set_config("sox.change_management_controls", serde_json::Value::Bool(true)).await?;
            },
            "sox_reporting_003" => {
                database.set_config("sox.segregation_of_duties", serde_json::Value::Bool(true)).await?;
            },
            "sox_audit_001" => {
                database.set_config("monitoring.comprehensive_audit_logging", serde_json::Value::Bool(true)).await?;
            },
            "sox_audit_002" => {
                database.set_config("sox.audit_log_retention_days", serde_json::Value::Number(2555.into())).await?;
            },
            "sox_audit_003" => {
                database.set_config("monitoring.log_integrity_protection", serde_json::Value::Bool(true)).await?;
            },
            "sox_audit_004" => {
                database.set_config("sox.audit_trail_completeness", serde_json::Value::Bool(true)).await?;
            },
            "sox_access_001" => {
                database.set_config("auth.rbac_enabled", serde_json::Value::Bool(true)).await?;
            },
            "sox_access_002" => {
                database.set_config("sox.periodic_access_reviews", serde_json::Value::Bool(true)).await?;
            },
            "sox_access_003" => {
                database.set_config("sox.privileged_access_management", serde_json::Value::Bool(true)).await?;
            },
            "sox_retention_001" => {
                let retention_policy = serde_json::json!({
                    "financial_records_retention_days": 2555,  // 7 years
                    "audit_logs_retention_days": 2555,
                    "supporting_documents_retention_days": 2555,
                });
                database.set_config("sox.data_retention_policy", retention_policy).await?;
            },
            "sox_retention_002" => {
                let retention_policy = serde_json::json!({
                    "financial_records_retention_days": 2555,  // 7 years minimum
                });
                database.set_config("sox.data_retention_policy", retention_policy).await?;
            },
            "sox_retention_003" => {
                database.set_config("sox.secure_disposal_procedures", serde_json::Value::Bool(true)).await?;
            },
            "sox_backup_001" => {
                database.set_config("sox.financial_data_backup", serde_json::Value::Bool(true)).await?;
            },
            "sox_backup_002" => {
                database.set_config("sox.disaster_recovery_procedures", serde_json::Value::Bool(true)).await?;
            },
            _ => {
                return Err(anyhow::anyhow!("Unknown SOX issue ID: {}", issue_id));
            }
        }

        debug!("Successfully remediated SOX issue: {}", issue_id);
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
    async fn test_sox_compliance_check() {
        let database = create_test_database().await;
        let checker = SOXChecker::new();

        let status = checker.check_compliance(&database).await.unwrap();
        assert_eq!(status.compliance_type, "sox");

        // Should have issues since nothing is configured
        assert!(!status.issues.is_empty());
        assert_eq!(status.status, "non_compliant");
    }

    #[tokio::test]
    async fn test_sox_requirements() {
        let checker = SOXChecker::new();
        let requirements = checker.get_requirements().await;

        assert!(!requirements.is_empty());
        assert!(requirements.iter().all(|r| r.mandatory));
    }

    #[tokio::test]
    async fn test_sox_remediation() {
        let database = create_test_database().await;
        let checker = SOXChecker::new();

        // Test remediation
        checker.remediate_issue("sox_reporting_001", &database).await.unwrap();

        // Verify configuration was set
        let financial_classified = database.get_config("sox.financial_data_classified").await.unwrap();
        assert!(financial_classified.as_bool().unwrap());
    }
}