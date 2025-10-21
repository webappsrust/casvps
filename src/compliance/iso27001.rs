use anyhow::Result;
use std::collections::HashMap;
use std::time::SystemTime;
use async_trait::async_trait;
use tracing::{debug, warn};

use crate::database::Database;
use super::{ComplianceChecker, ComplianceStatus, ComplianceIssue, ComplianceRequirement};

pub struct ISO27001Checker;

impl ISO27001Checker {
    pub fn new() -> Self {
        Self
    }

    async fn check_information_security_policy(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for documented security policy
        let security_policy_documented = database.get_config("iso27001.security_policy_documented")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !security_policy_documented {
            issues.push(ComplianceIssue {
                id: "iso27001_policy_001".to_string(),
                severity: "critical".to_string(),
                title: "No Documented Security Policy".to_string(),
                description: "ISO 27001 requires a documented information security policy".to_string(),
                remediation: "Create and document comprehensive information security policy".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for management approval
        let management_approval = database.get_config("iso27001.management_approval")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !management_approval {
            issues.push(ComplianceIssue {
                id: "iso27001_policy_002".to_string(),
                severity: "high".to_string(),
                title: "No Management Approval".to_string(),
                description: "ISO 27001 requires management approval of security policy".to_string(),
                remediation: "Obtain management approval and commitment for security policy".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_risk_management(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for risk assessment process
        let risk_assessment = database.get_config("iso27001.risk_assessment_process")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !risk_assessment {
            issues.push(ComplianceIssue {
                id: "iso27001_risk_001".to_string(),
                severity: "critical".to_string(),
                title: "No Risk Assessment Process".to_string(),
                description: "ISO 27001 requires systematic risk assessment".to_string(),
                remediation: "Implement risk assessment methodology and process".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for risk treatment plan
        let risk_treatment = database.get_config("iso27001.risk_treatment_plan")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !risk_treatment {
            issues.push(ComplianceIssue {
                id: "iso27001_risk_002".to_string(),
                severity: "high".to_string(),
                title: "No Risk Treatment Plan".to_string(),
                description: "ISO 27001 requires risk treatment plan for identified risks".to_string(),
                remediation: "Develop and implement risk treatment plan".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for regular risk reviews
        let risk_reviews = database.get_config("iso27001.regular_risk_reviews")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !risk_reviews {
            issues.push(ComplianceIssue {
                id: "iso27001_risk_003".to_string(),
                severity: "medium".to_string(),
                title: "No Regular Risk Reviews".to_string(),
                description: "ISO 27001 requires periodic review of risks".to_string(),
                remediation: "Schedule and conduct regular risk assessment reviews".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_access_control(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for access control policy
        let access_control_policy = database.get_config("iso27001.access_control_policy")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !access_control_policy {
            issues.push(ComplianceIssue {
                id: "iso27001_access_001".to_string(),
                severity: "critical".to_string(),
                title: "No Access Control Policy".to_string(),
                description: "ISO 27001 requires documented access control policy".to_string(),
                remediation: "Create comprehensive access control policy".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for user access management
        let user_access_mgmt = database.get_config("auth.rbac_enabled")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !user_access_mgmt {
            issues.push(ComplianceIssue {
                id: "iso27001_access_002".to_string(),
                severity: "high".to_string(),
                title: "No User Access Management".to_string(),
                description: "ISO 27001 requires formal user access management process".to_string(),
                remediation: "Implement role-based access control and user management".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for privileged access management
        let privileged_access = database.get_config("iso27001.privileged_access_management")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !privileged_access {
            issues.push(ComplianceIssue {
                id: "iso27001_access_003".to_string(),
                severity: "high".to_string(),
                title: "No Privileged Access Management".to_string(),
                description: "ISO 27001 requires special controls for privileged access".to_string(),
                remediation: "Implement privileged access management controls".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for access reviews
        let access_reviews = database.get_config("iso27001.access_reviews")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !access_reviews {
            issues.push(ComplianceIssue {
                id: "iso27001_access_004".to_string(),
                severity: "medium".to_string(),
                title: "No Regular Access Reviews".to_string(),
                description: "ISO 27001 requires periodic review of access rights".to_string(),
                remediation: "Implement periodic access rights review process".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_cryptography(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for cryptographic policy
        let crypto_policy = database.get_config("iso27001.cryptographic_policy")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !crypto_policy {
            issues.push(ComplianceIssue {
                id: "iso27001_crypto_001".to_string(),
                severity: "high".to_string(),
                title: "No Cryptographic Policy".to_string(),
                description: "ISO 27001 requires policy on the use of cryptographic controls".to_string(),
                remediation: "Develop cryptographic policy and implementation guidelines".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check encryption in transit
        let encryption_transit = database.get_config("security.encryption_in_transit")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !encryption_transit {
            issues.push(ComplianceIssue {
                id: "iso27001_crypto_002".to_string(),
                severity: "high".to_string(),
                title: "No Encryption in Transit".to_string(),
                description: "ISO 27001 requires protection of information in transit".to_string(),
                remediation: "Implement encryption for all data transmissions".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check encryption at rest
        let encryption_rest = database.get_config("security.encryption_at_rest")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !encryption_rest {
            issues.push(ComplianceIssue {
                id: "iso27001_crypto_003".to_string(),
                severity: "high".to_string(),
                title: "No Encryption at Rest".to_string(),
                description: "ISO 27001 requires protection of stored information".to_string(),
                remediation: "Implement encryption for all stored data".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check key management
        let key_management = database.get_config("iso27001.key_management")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !key_management {
            issues.push(ComplianceIssue {
                id: "iso27001_crypto_004".to_string(),
                severity: "high".to_string(),
                title: "No Key Management".to_string(),
                description: "ISO 27001 requires management of cryptographic keys".to_string(),
                remediation: "Implement cryptographic key management system".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_incident_management(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for incident response procedures
        let incident_procedures = database.get_config("iso27001.incident_response_procedures")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !incident_procedures {
            issues.push(ComplianceIssue {
                id: "iso27001_incident_001".to_string(),
                severity: "critical".to_string(),
                title: "No Incident Response Procedures".to_string(),
                description: "ISO 27001 requires documented incident response procedures".to_string(),
                remediation: "Develop and implement incident response procedures".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for incident reporting
        let incident_reporting = database.get_config("iso27001.incident_reporting")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !incident_reporting {
            issues.push(ComplianceIssue {
                id: "iso27001_incident_002".to_string(),
                severity: "high".to_string(),
                title: "No Incident Reporting System".to_string(),
                description: "ISO 27001 requires system for reporting security incidents".to_string(),
                remediation: "Implement incident reporting and tracking system".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_monitoring(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for monitoring procedures
        let monitoring_procedures = database.get_config("monitoring.security_monitoring")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !monitoring_procedures {
            issues.push(ComplianceIssue {
                id: "iso27001_monitoring_001".to_string(),
                severity: "critical".to_string(),
                title: "No Security Monitoring".to_string(),
                description: "ISO 27001 requires monitoring of information security".to_string(),
                remediation: "Implement security monitoring and logging procedures".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for performance measurement
        let performance_measurement = database.get_config("iso27001.performance_measurement")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !performance_measurement {
            issues.push(ComplianceIssue {
                id: "iso27001_monitoring_002".to_string(),
                severity: "medium".to_string(),
                title: "No Performance Measurement".to_string(),
                description: "ISO 27001 requires measurement of security performance".to_string(),
                remediation: "Implement security metrics and performance measurement".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }
}

#[async_trait]
impl ComplianceChecker for ISO27001Checker {
    fn compliance_type(&self) -> &'static str {
        "iso27001"
    }

    async fn check_compliance(&self, database: &Database) -> Result<ComplianceStatus> {
        debug!("Running ISO 27001 compliance check");

        let mut all_issues = Vec::new();

        // Run all ISO 27001 compliance checks
        all_issues.extend(self.check_information_security_policy(database).await?);
        all_issues.extend(self.check_risk_management(database).await?);
        all_issues.extend(self.check_access_control(database).await?);
        all_issues.extend(self.check_cryptography(database).await?);
        all_issues.extend(self.check_incident_management(database).await?);
        all_issues.extend(self.check_monitoring(database).await?);

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
            warn!("ISO 27001 compliance issues found: {}", all_issues.len());
        }

        Ok(ComplianceStatus {
            compliance_type: "iso27001".to_string(),
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
                id: "iso27001_req_001".to_string(),
                title: "Information Security Policy".to_string(),
                description: "Establish and maintain information security policy".to_string(),
                mandatory: true,
                category: "policy".to_string(),
            },
            ComplianceRequirement {
                id: "iso27001_req_002".to_string(),
                title: "Risk Management".to_string(),
                description: "Implement systematic risk assessment and treatment".to_string(),
                mandatory: true,
                category: "risk_management".to_string(),
            },
            ComplianceRequirement {
                id: "iso27001_req_003".to_string(),
                title: "Access Control".to_string(),
                description: "Control access to information and information processing facilities".to_string(),
                mandatory: true,
                category: "access_control".to_string(),
            },
            ComplianceRequirement {
                id: "iso27001_req_004".to_string(),
                title: "Cryptography".to_string(),
                description: "Use cryptography to protect information confidentiality and integrity".to_string(),
                mandatory: true,
                category: "cryptography".to_string(),
            },
            ComplianceRequirement {
                id: "iso27001_req_005".to_string(),
                title: "Incident Management".to_string(),
                description: "Manage information security incidents effectively".to_string(),
                mandatory: true,
                category: "incident_management".to_string(),
            },
            ComplianceRequirement {
                id: "iso27001_req_006".to_string(),
                title: "Monitoring and Evaluation".to_string(),
                description: "Monitor, measure and evaluate information security performance".to_string(),
                mandatory: true,
                category: "monitoring".to_string(),
            },
        ]
    }

    async fn remediate_issue(&self, issue_id: &str, database: &Database) -> Result<()> {
        debug!("Attempting to remediate ISO 27001 issue: {}", issue_id);

        match issue_id {
            "iso27001_policy_001" => {
                database.set_config("iso27001.security_policy_documented", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_policy_002" => {
                database.set_config("iso27001.management_approval", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_risk_001" => {
                database.set_config("iso27001.risk_assessment_process", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_risk_002" => {
                database.set_config("iso27001.risk_treatment_plan", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_risk_003" => {
                database.set_config("iso27001.regular_risk_reviews", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_access_001" => {
                database.set_config("iso27001.access_control_policy", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_access_002" => {
                database.set_config("auth.rbac_enabled", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_access_003" => {
                database.set_config("iso27001.privileged_access_management", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_access_004" => {
                database.set_config("iso27001.access_reviews", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_crypto_001" => {
                database.set_config("iso27001.cryptographic_policy", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_crypto_002" => {
                database.set_config("security.encryption_in_transit", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_crypto_003" => {
                database.set_config("security.encryption_at_rest", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_crypto_004" => {
                database.set_config("iso27001.key_management", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_incident_001" => {
                database.set_config("iso27001.incident_response_procedures", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_incident_002" => {
                database.set_config("iso27001.incident_reporting", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_monitoring_001" => {
                database.set_config("monitoring.security_monitoring", serde_json::Value::Bool(true)).await?;
            },
            "iso27001_monitoring_002" => {
                database.set_config("iso27001.performance_measurement", serde_json::Value::Bool(true)).await?;
            },
            _ => {
                return Err(anyhow::anyhow!("Unknown ISO 27001 issue ID: {}", issue_id));
            }
        }

        debug!("Successfully remediated ISO 27001 issue: {}", issue_id);
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
    async fn test_iso27001_compliance_check() {
        let database = create_test_database().await;
        let checker = ISO27001Checker::new();

        let status = checker.check_compliance(&database).await.unwrap();
        assert_eq!(status.compliance_type, "iso27001");

        // Should have issues since nothing is configured
        assert!(!status.issues.is_empty());
        assert_eq!(status.status, "non_compliant");
    }

    #[tokio::test]
    async fn test_iso27001_requirements() {
        let checker = ISO27001Checker::new();
        let requirements = checker.get_requirements().await;

        assert!(!requirements.is_empty());
        assert!(requirements.iter().all(|r| r.mandatory));
    }

    #[tokio::test]
    async fn test_iso27001_remediation() {
        let database = create_test_database().await;
        let checker = ISO27001Checker::new();

        // Test remediation
        checker.remediate_issue("iso27001_policy_001", &database).await.unwrap();

        // Verify configuration was set
        let policy_documented = database.get_config("iso27001.security_policy_documented").await.unwrap();
        assert!(policy_documented.as_bool().unwrap());
    }
}