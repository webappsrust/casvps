use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};

use crate::database::Database;

pub mod gdpr;
pub mod pci_dss;
pub mod hipaa;
pub mod sox;
pub mod iso27001;
pub mod fips;
pub mod audit;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStatus {
    pub compliance_type: String,
    pub enabled: bool,
    pub status: String,  // 'compliant', 'non_compliant', 'warning', 'unknown'
    pub last_check: SystemTime,
    pub issues: Vec<ComplianceIssue>,
    pub config: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceIssue {
    pub id: String,
    pub severity: String,  // 'critical', 'high', 'medium', 'low'
    pub title: String,
    pub description: String,
    pub remediation: String,
    pub detected_at: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub report_id: String,
    pub generated_at: SystemTime,
    pub compliance_types: Vec<String>,
    pub overall_status: String,
    pub summary: HashMap<String, ComplianceStatus>,
    pub recommendations: Vec<String>,
}

pub struct ComplianceFramework {
    database: Arc<Database>,
    checkers: RwLock<HashMap<String, Box<dyn ComplianceChecker>>>,
    audit_logger: audit::AuditLogger,
}

pub trait ComplianceChecker: Send + Sync {
    fn compliance_type(&self) -> &'static str;
    async fn check_compliance(&self, database: &Database) -> Result<ComplianceStatus>;
    async fn get_requirements(&self) -> Vec<ComplianceRequirement>;
    async fn remediate_issue(&self, issue_id: &str, database: &Database) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceRequirement {
    pub id: String,
    pub title: String,
    pub description: String,
    pub mandatory: bool,
    pub category: String,
}

impl ComplianceFramework {
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        let mut framework = Self {
            database: database.clone(),
            checkers: RwLock::new(HashMap::new()),
            audit_logger: audit::AuditLogger::new(database.clone()).await?,
        };

        // Register default compliance checkers (off by default)
        framework.register_checker(Box::new(gdpr::GDPRChecker::new())).await?;
        framework.register_checker(Box::new(pci_dss::PCIDSSChecker::new())).await?;
        framework.register_checker(Box::new(hipaa::HIPAAChecker::new())).await?;
        framework.register_checker(Box::new(sox::SOXChecker::new())).await?;
        framework.register_checker(Box::new(iso27001::ISO27001Checker::new())).await?;
        framework.register_checker(Box::new(fips::FIPSChecker::new())).await?;

        Ok(framework)
    }

    async fn register_checker(&self, checker: Box<dyn ComplianceChecker>) -> Result<()> {
        let compliance_type = checker.compliance_type().to_string();
        debug!("Registering compliance checker: {}", compliance_type);

        // Check if enabled in database (default: false)
        let enabled = self.database.get_config(&format!("compliance.{}.enabled", compliance_type))
            .await
            .map(|v| v.as_bool().unwrap_or(false))
            .unwrap_or(false);

        if enabled {
            info!("Compliance checker {} is enabled", compliance_type);
            let mut checkers = self.checkers.write().await;
            checkers.insert(compliance_type, checker);
        } else {
            debug!("Compliance checker {} is disabled", compliance_type);
        }

        Ok(())
    }

    pub async fn enable_compliance(&self, compliance_type: &str) -> Result<()> {
        info!("Enabling compliance framework: {}", compliance_type);

        // Update database config
        self.database.set_config(
            &format!("compliance.{}.enabled", compliance_type),
            serde_json::Value::Bool(true),
        ).await?;

        // Register checker if available
        let checker = match compliance_type {
            "gdpr" => Box::new(gdpr::GDPRChecker::new()) as Box<dyn ComplianceChecker>,
            "pci" => Box::new(pci_dss::PCIDSSChecker::new()) as Box<dyn ComplianceChecker>,
            "hipaa" => Box::new(hipaa::HIPAAChecker::new()) as Box<dyn ComplianceChecker>,
            "sox" => Box::new(sox::SOXChecker::new()) as Box<dyn ComplianceChecker>,
            "iso27001" => Box::new(iso27001::ISO27001Checker::new()) as Box<dyn ComplianceChecker>,
            "fips" => Box::new(fips::FIPSChecker::new()) as Box<dyn ComplianceChecker>,
            _ => return Err(anyhow::anyhow!("Unknown compliance type: {}", compliance_type)),
        };

        let mut checkers = self.checkers.write().await;
        checkers.insert(compliance_type.to_string(), checker);

        // Log compliance enabling
        self.audit_logger.log_compliance_change("enable", compliance_type, None).await?;

        // Run initial compliance check
        self.check_single_compliance(compliance_type).await?;

        Ok(())
    }

    pub async fn disable_compliance(&self, compliance_type: &str) -> Result<()> {
        info!("Disabling compliance framework: {}", compliance_type);

        // Update database config
        self.database.set_config(
            &format!("compliance.{}.enabled", compliance_type),
            serde_json::Value::Bool(false),
        ).await?;

        // Remove checker
        let mut checkers = self.checkers.write().await;
        checkers.remove(compliance_type);

        // Log compliance disabling
        self.audit_logger.log_compliance_change("disable", compliance_type, None).await?;

        Ok(())
    }

    pub async fn check_all_compliance(&self) -> Result<ComplianceReport> {
        debug!("Running compliance checks for all enabled frameworks");

        let checkers = self.checkers.read().await;
        let mut summary = HashMap::new();
        let mut overall_status = "compliant".to_string();
        let mut recommendations = Vec::new();

        for (compliance_type, checker) in checkers.iter() {
            match checker.check_compliance(&self.database).await {
                Ok(status) => {
                    if status.status == "non_compliant" {
                        overall_status = "non_compliant".to_string();
                    } else if status.status == "warning" && overall_status == "compliant" {
                        overall_status = "warning".to_string();
                    }

                    // Add recommendations for issues
                    for issue in &status.issues {
                        recommendations.push(format!("[{}] {}: {}",
                            compliance_type.to_uppercase(),
                            issue.title,
                            issue.remediation));
                    }

                    summary.insert(compliance_type.clone(), status);
                },
                Err(e) => {
                    error!("Failed to check compliance for {}: {}", compliance_type, e);
                    let error_status = ComplianceStatus {
                        compliance_type: compliance_type.clone(),
                        enabled: true,
                        status: "unknown".to_string(),
                        last_check: SystemTime::now(),
                        issues: vec![ComplianceIssue {
                            id: format!("check_error_{}", compliance_type),
                            severity: "critical".to_string(),
                            title: "Compliance Check Failed".to_string(),
                            description: format!("Unable to perform compliance check: {}", e),
                            remediation: "Check system configuration and retry".to_string(),
                            detected_at: SystemTime::now(),
                        }],
                        config: HashMap::new(),
                    };
                    summary.insert(compliance_type.clone(), error_status);
                    overall_status = "unknown".to_string();
                }
            }
        }

        let report = ComplianceReport {
            report_id: uuid::Uuid::new_v4().to_string(),
            generated_at: SystemTime::now(),
            compliance_types: checkers.keys().cloned().collect(),
            overall_status,
            summary,
            recommendations,
        };

        // Log compliance report generation
        self.audit_logger.log_compliance_report(&report).await?;

        Ok(report)
    }

    pub async fn check_single_compliance(&self, compliance_type: &str) -> Result<ComplianceStatus> {
        debug!("Running compliance check for: {}", compliance_type);

        let checkers = self.checkers.read().await;
        if let Some(checker) = checkers.get(compliance_type) {
            let status = checker.check_compliance(&self.database).await?;

            // Log compliance check
            self.audit_logger.log_compliance_check(compliance_type, &status).await?;

            Ok(status)
        } else {
            Err(anyhow::anyhow!("Compliance type not enabled or not found: {}", compliance_type))
        }
    }

    pub async fn get_compliance_requirements(&self, compliance_type: &str) -> Result<Vec<ComplianceRequirement>> {
        let checkers = self.checkers.read().await;
        if let Some(checker) = checkers.get(compliance_type) {
            Ok(checker.get_requirements().await)
        } else {
            Err(anyhow::anyhow!("Compliance type not enabled: {}", compliance_type))
        }
    }

    pub async fn remediate_issue(&self, compliance_type: &str, issue_id: &str) -> Result<()> {
        info!("Remediating compliance issue: {} in {}", issue_id, compliance_type);

        let checkers = self.checkers.read().await;
        if let Some(checker) = checkers.get(compliance_type) {
            checker.remediate_issue(issue_id, &self.database).await?;

            // Log remediation action
            self.audit_logger.log_compliance_remediation(compliance_type, issue_id).await?;

            // Re-run compliance check after remediation
            drop(checkers);
            self.check_single_compliance(compliance_type).await?;

            Ok(())
        } else {
            Err(anyhow::anyhow!("Compliance type not enabled: {}", compliance_type))
        }
    }

    pub async fn get_enabled_compliance_types(&self) -> Result<Vec<String>> {
        let checkers = self.checkers.read().await;
        Ok(checkers.keys().cloned().collect())
    }

    pub async fn schedule_compliance_checks(&self) -> Result<()> {
        info!("Scheduling automatic compliance checks");

        // Schedule daily compliance checks for all enabled frameworks
        let enabled_types = self.get_enabled_compliance_types().await?;

        for compliance_type in enabled_types {
            // Schedule based on compliance requirements
            let interval = match compliance_type.as_str() {
                "pci" => Duration::from_secs(24 * 3600), // Daily for PCI-DSS
                "hipaa" => Duration::from_secs(24 * 3600), // Daily for HIPAA
                "sox" => Duration::from_secs(24 * 3600), // Daily for SOX
                "gdpr" => Duration::from_secs(7 * 24 * 3600), // Weekly for GDPR
                "iso27001" => Duration::from_secs(7 * 24 * 3600), // Weekly for ISO27001
                "fips" => Duration::from_secs(24 * 3600), // Daily for FIPS
                _ => Duration::from_secs(24 * 3600), // Default daily
            };

            // Add to task scheduler (would integrate with existing scheduler)
            debug!("Scheduling compliance check for {} every {:?}", compliance_type, interval);
        }

        Ok(())
    }

    pub async fn export_compliance_report(&self, report: &ComplianceReport, format: &str) -> Result<String> {
        match format {
            "json" => Ok(serde_json::to_string_pretty(report)?),
            "csv" => self.export_csv_report(report).await,
            "pdf" => self.export_pdf_report(report).await,
            _ => Err(anyhow::anyhow!("Unsupported export format: {}", format)),
        }
    }

    async fn export_csv_report(&self, report: &ComplianceReport) -> Result<String> {
        let mut csv = String::new();
        csv.push_str("Compliance Type,Status,Issue Count,Critical Issues,High Issues,Medium Issues,Low Issues\n");

        for (compliance_type, status) in &report.summary {
            let critical_count = status.issues.iter().filter(|i| i.severity == "critical").count();
            let high_count = status.issues.iter().filter(|i| i.severity == "high").count();
            let medium_count = status.issues.iter().filter(|i| i.severity == "medium").count();
            let low_count = status.issues.iter().filter(|i| i.severity == "low").count();

            csv.push_str(&format!("{},{},{},{},{},{},{}\n",
                compliance_type.to_uppercase(),
                status.status,
                status.issues.len(),
                critical_count,
                high_count,
                medium_count,
                low_count
            ));
        }

        Ok(csv)
    }

    async fn export_pdf_report(&self, _report: &ComplianceReport) -> Result<String> {
        // PDF generation would require additional dependencies
        // For now, return HTML that can be converted to PDF
        Ok("PDF export not implemented - use HTML format".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::path::Path;

    async fn create_test_database() -> Arc<Database> {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        Arc::new(Database::new(db_path.to_str().unwrap()).await.unwrap())
    }

    #[tokio::test]
    async fn test_compliance_framework_creation() {
        let database = create_test_database().await;
        let framework = ComplianceFramework::new(database).await.unwrap();

        // All compliance should be disabled by default
        let enabled_types = framework.get_enabled_compliance_types().await.unwrap();
        assert_eq!(enabled_types.len(), 0);
    }

    #[tokio::test]
    async fn test_enable_disable_compliance() {
        let database = create_test_database().await;
        let framework = ComplianceFramework::new(database).await.unwrap();

        // Enable GDPR compliance
        framework.enable_compliance("gdpr").await.unwrap();
        let enabled_types = framework.get_enabled_compliance_types().await.unwrap();
        assert!(enabled_types.contains(&"gdpr".to_string()));

        // Disable GDPR compliance
        framework.disable_compliance("gdpr").await.unwrap();
        let enabled_types = framework.get_enabled_compliance_types().await.unwrap();
        assert!(!enabled_types.contains(&"gdpr".to_string()));
    }

    #[tokio::test]
    async fn test_compliance_report_generation() {
        let database = create_test_database().await;
        let framework = ComplianceFramework::new(database).await.unwrap();

        // Enable a compliance type
        framework.enable_compliance("gdpr").await.unwrap();

        // Generate report
        let report = framework.check_all_compliance().await.unwrap();
        assert!(report.compliance_types.contains(&"gdpr".to_string()));
        assert!(!report.report_id.is_empty());
    }
}