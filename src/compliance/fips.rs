use anyhow::Result;
use std::collections::HashMap;
use std::time::SystemTime;
use async_trait::async_trait;
use tracing::{debug, warn};

use crate::database::Database;
use super::{ComplianceChecker, ComplianceStatus, ComplianceIssue, ComplianceRequirement};

pub struct FIPSChecker;

impl FIPSChecker {
    pub fn new() -> Self {
        Self
    }

    async fn check_cryptographic_modules(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for FIPS 140-2 validated modules
        let fips_modules = database.get_config("fips.validated_crypto_modules")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !fips_modules {
            issues.push(ComplianceIssue {
                id: "fips_crypto_001".to_string(),
                severity: "critical".to_string(),
                title: "No FIPS 140-2 Validated Modules".to_string(),
                description: "FIPS 140-2 requires use of validated cryptographic modules".to_string(),
                remediation: "Configure system to use FIPS 140-2 validated cryptographic modules".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for approved algorithms
        let approved_algorithms = database.get_config("fips.approved_algorithms_only")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !approved_algorithms {
            issues.push(ComplianceIssue {
                id: "fips_crypto_002".to_string(),
                severity: "critical".to_string(),
                title: "Non-Approved Algorithms in Use".to_string(),
                description: "FIPS 140-2 requires use of only approved cryptographic algorithms".to_string(),
                remediation: "Configure system to use only FIPS-approved algorithms".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_key_management(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for FIPS-compliant key generation
        let key_generation = database.get_config("fips.compliant_key_generation")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !key_generation {
            issues.push(ComplianceIssue {
                id: "fips_keys_001".to_string(),
                severity: "critical".to_string(),
                title: "Non-FIPS Key Generation".to_string(),
                description: "FIPS 140-2 requires compliant key generation methods".to_string(),
                remediation: "Use FIPS-approved key generation mechanisms".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for key storage protection
        let key_storage = database.get_config("fips.secure_key_storage")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !key_storage {
            issues.push(ComplianceIssue {
                id: "fips_keys_002".to_string(),
                severity: "critical".to_string(),
                title: "Insecure Key Storage".to_string(),
                description: "FIPS 140-2 requires secure protection of cryptographic keys".to_string(),
                remediation: "Implement secure key storage with appropriate protection levels".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for key destruction
        let key_destruction = database.get_config("fips.secure_key_destruction")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !key_destruction {
            issues.push(ComplianceIssue {
                id: "fips_keys_003".to_string(),
                severity: "high".to_string(),
                title: "Insecure Key Destruction".to_string(),
                description: "FIPS 140-2 requires secure destruction of cryptographic keys".to_string(),
                remediation: "Implement secure key destruction procedures".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_authentication(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for FIPS-approved authentication
        let fips_auth = database.get_config("fips.approved_authentication")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !fips_auth {
            issues.push(ComplianceIssue {
                id: "fips_auth_001".to_string(),
                severity: "critical".to_string(),
                title: "Non-FIPS Authentication Methods".to_string(),
                description: "FIPS 140-2 requires approved authentication mechanisms".to_string(),
                remediation: "Configure FIPS-approved authentication methods".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for role-based authentication
        let role_based_auth = database.get_config("fips.role_based_authentication")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !role_based_auth {
            issues.push(ComplianceIssue {
                id: "fips_auth_002".to_string(),
                severity: "high".to_string(),
                title: "No Role-Based Authentication".to_string(),
                description: "FIPS 140-2 requires role-based authentication and authorization".to_string(),
                remediation: "Implement role-based authentication system".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_self_tests(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for power-on self tests
        let power_on_tests = database.get_config("fips.power_on_self_tests")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !power_on_tests {
            issues.push(ComplianceIssue {
                id: "fips_tests_001".to_string(),
                severity: "critical".to_string(),
                title: "No Power-On Self Tests".to_string(),
                description: "FIPS 140-2 requires power-on self tests for cryptographic modules".to_string(),
                remediation: "Enable power-on self tests for all cryptographic functions".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for conditional self tests
        let conditional_tests = database.get_config("fips.conditional_self_tests")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !conditional_tests {
            issues.push(ComplianceIssue {
                id: "fips_tests_002".to_string(),
                severity: "high".to_string(),
                title: "No Conditional Self Tests".to_string(),
                description: "FIPS 140-2 requires conditional self tests during operation".to_string(),
                remediation: "Enable conditional self tests for cryptographic operations".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_physical_security(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for tamper evidence (Level 2+)
        let tamper_evidence = database.get_config("fips.tamper_evidence")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        let security_level = database.get_config("fips.security_level")
            .await
            .map(|v| v.as_u64())
            .unwrap_or_default()
            .unwrap_or(1);

        if security_level >= 2 && !tamper_evidence {
            issues.push(ComplianceIssue {
                id: "fips_physical_001".to_string(),
                severity: "high".to_string(),
                title: "No Tamper Evidence".to_string(),
                description: "FIPS 140-2 Level 2+ requires tamper evidence mechanisms".to_string(),
                remediation: "Implement tamper evidence controls for Level 2+ compliance".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for tamper response (Level 3+)
        let tamper_response = database.get_config("fips.tamper_response")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if security_level >= 3 && !tamper_response {
            issues.push(ComplianceIssue {
                id: "fips_physical_002".to_string(),
                severity: "critical".to_string(),
                title: "No Tamper Response".to_string(),
                description: "FIPS 140-2 Level 3+ requires tamper response mechanisms".to_string(),
                remediation: "Implement tamper response controls for Level 3+ compliance".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }

    async fn check_design_assurance(&self, database: &Database) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for configuration management
        let config_mgmt = database.get_config("fips.configuration_management")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !config_mgmt {
            issues.push(ComplianceIssue {
                id: "fips_design_001".to_string(),
                severity: "high".to_string(),
                title: "No Configuration Management".to_string(),
                description: "FIPS 140-2 requires configuration management for cryptographic modules".to_string(),
                remediation: "Implement configuration management system".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        // Check for delivery and operation
        let delivery_operation = database.get_config("fips.delivery_and_operation")
            .await
            .map(|v| v.as_bool())
            .unwrap_or_default()
            .unwrap_or(false);

        if !delivery_operation {
            issues.push(ComplianceIssue {
                id: "fips_design_002".to_string(),
                severity: "medium".to_string(),
                title: "No Delivery and Operation Controls".to_string(),
                description: "FIPS 140-2 requires controls for delivery and operation".to_string(),
                remediation: "Implement delivery and operation security controls".to_string(),
                detected_at: SystemTime::now(),
            });
        }

        Ok(issues)
    }
}

#[async_trait]
impl ComplianceChecker for FIPSChecker {
    fn compliance_type(&self) -> &'static str {
        "fips"
    }

    async fn check_compliance(&self, database: &Database) -> Result<ComplianceStatus> {
        debug!("Running FIPS 140-2 compliance check");

        let mut all_issues = Vec::new();

        // Run all FIPS 140-2 compliance checks
        all_issues.extend(self.check_cryptographic_modules(database).await?);
        all_issues.extend(self.check_key_management(database).await?);
        all_issues.extend(self.check_authentication(database).await?);
        all_issues.extend(self.check_self_tests(database).await?);
        all_issues.extend(self.check_physical_security(database).await?);
        all_issues.extend(self.check_design_assurance(database).await?);

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
            warn!("FIPS 140-2 compliance issues found: {}", all_issues.len());
        }

        Ok(ComplianceStatus {
            compliance_type: "fips".to_string(),
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
                id: "fips_req_001".to_string(),
                title: "Cryptographic Module Specification".to_string(),
                description: "Use FIPS 140-2 validated cryptographic modules".to_string(),
                mandatory: true,
                category: "cryptographic_modules".to_string(),
            },
            ComplianceRequirement {
                id: "fips_req_002".to_string(),
                title: "Cryptographic Module Ports and Interfaces".to_string(),
                description: "Define and control all ports and interfaces".to_string(),
                mandatory: true,
                category: "ports_interfaces".to_string(),
            },
            ComplianceRequirement {
                id: "fips_req_003".to_string(),
                title: "Roles, Services, and Authentication".to_string(),
                description: "Implement role-based authentication and authorization".to_string(),
                mandatory: true,
                category: "authentication".to_string(),
            },
            ComplianceRequirement {
                id: "fips_req_004".to_string(),
                title: "Finite State Model".to_string(),
                description: "Define finite state model for cryptographic module".to_string(),
                mandatory: true,
                category: "finite_state".to_string(),
            },
            ComplianceRequirement {
                id: "fips_req_005".to_string(),
                title: "Physical Security".to_string(),
                description: "Implement appropriate physical security controls".to_string(),
                mandatory: true,
                category: "physical_security".to_string(),
            },
            ComplianceRequirement {
                id: "fips_req_006".to_string(),
                title: "Operational Environment".to_string(),
                description: "Control and secure the operational environment".to_string(),
                mandatory: true,
                category: "operational_environment".to_string(),
            },
            ComplianceRequirement {
                id: "fips_req_007".to_string(),
                title: "Cryptographic Key Management".to_string(),
                description: "Implement secure key generation, storage, and destruction".to_string(),
                mandatory: true,
                category: "key_management".to_string(),
            },
            ComplianceRequirement {
                id: "fips_req_008".to_string(),
                title: "Electromagnetic Interference/Compatibility".to_string(),
                description: "Meet EMI/EMC requirements for cryptographic modules".to_string(),
                mandatory: true,
                category: "emi_emc".to_string(),
            },
            ComplianceRequirement {
                id: "fips_req_009".to_string(),
                title: "Self-Tests".to_string(),
                description: "Implement power-on and conditional self-tests".to_string(),
                mandatory: true,
                category: "self_tests".to_string(),
            },
            ComplianceRequirement {
                id: "fips_req_010".to_string(),
                title: "Design Assurance".to_string(),
                description: "Provide design assurance and configuration management".to_string(),
                mandatory: true,
                category: "design_assurance".to_string(),
            },
            ComplianceRequirement {
                id: "fips_req_011".to_string(),
                title: "Mitigation of Other Attacks".to_string(),
                description: "Implement mitigation for other attacks as required".to_string(),
                mandatory: true,
                category: "attack_mitigation".to_string(),
            },
        ]
    }

    async fn remediate_issue(&self, issue_id: &str, database: &Database) -> Result<()> {
        debug!("Attempting to remediate FIPS 140-2 issue: {}", issue_id);

        match issue_id {
            "fips_crypto_001" => {
                database.set_config("fips.validated_crypto_modules", serde_json::Value::Bool(true)).await?;
            },
            "fips_crypto_002" => {
                database.set_config("fips.approved_algorithms_only", serde_json::Value::Bool(true)).await?;
            },
            "fips_keys_001" => {
                database.set_config("fips.compliant_key_generation", serde_json::Value::Bool(true)).await?;
            },
            "fips_keys_002" => {
                database.set_config("fips.secure_key_storage", serde_json::Value::Bool(true)).await?;
            },
            "fips_keys_003" => {
                database.set_config("fips.secure_key_destruction", serde_json::Value::Bool(true)).await?;
            },
            "fips_auth_001" => {
                database.set_config("fips.approved_authentication", serde_json::Value::Bool(true)).await?;
            },
            "fips_auth_002" => {
                database.set_config("fips.role_based_authentication", serde_json::Value::Bool(true)).await?;
                database.set_config("auth.rbac_enabled", serde_json::Value::Bool(true)).await?;
            },
            "fips_tests_001" => {
                database.set_config("fips.power_on_self_tests", serde_json::Value::Bool(true)).await?;
            },
            "fips_tests_002" => {
                database.set_config("fips.conditional_self_tests", serde_json::Value::Bool(true)).await?;
            },
            "fips_physical_001" => {
                database.set_config("fips.tamper_evidence", serde_json::Value::Bool(true)).await?;
            },
            "fips_physical_002" => {
                database.set_config("fips.tamper_response", serde_json::Value::Bool(true)).await?;
            },
            "fips_design_001" => {
                database.set_config("fips.configuration_management", serde_json::Value::Bool(true)).await?;
            },
            "fips_design_002" => {
                database.set_config("fips.delivery_and_operation", serde_json::Value::Bool(true)).await?;
            },
            _ => {
                return Err(anyhow::anyhow!("Unknown FIPS 140-2 issue ID: {}", issue_id));
            }
        }

        debug!("Successfully remediated FIPS 140-2 issue: {}", issue_id);
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
    async fn test_fips_compliance_check() {
        let database = create_test_database().await;
        let checker = FIPSChecker::new();

        let status = checker.check_compliance(&database).await.unwrap();
        assert_eq!(status.compliance_type, "fips");

        // Should have issues since nothing is configured
        assert!(!status.issues.is_empty());
        assert_eq!(status.status, "non_compliant");
    }

    #[tokio::test]
    async fn test_fips_requirements() {
        let checker = FIPSChecker::new();
        let requirements = checker.get_requirements().await;

        // Should have 11 FIPS 140-2 requirements
        assert_eq!(requirements.len(), 11);
        assert!(requirements.iter().all(|r| r.mandatory));
    }

    #[tokio::test]
    async fn test_fips_remediation() {
        let database = create_test_database().await;
        let checker = FIPSChecker::new();

        // Test remediation
        checker.remediate_issue("fips_crypto_001", &database).await.unwrap();

        // Verify configuration was set
        let validated_modules = database.get_config("fips.validated_crypto_modules").await.unwrap();
        assert!(validated_modules.as_bool().unwrap());
    }
}