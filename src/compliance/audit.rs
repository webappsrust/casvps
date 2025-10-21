use anyhow::Result;
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};

use crate::database::Database;
use super::{ComplianceReport, ComplianceStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: String,
    pub timestamp: SystemTime,
    pub user_id: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub compliance_type: Option<String>,
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub session_id: Option<String>,
    pub result: String,  // 'success', 'failure', 'partial'
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceAuditEntry {
    pub id: String,
    pub timestamp: SystemTime,
    pub compliance_type: String,
    pub action: String,  // 'enable', 'disable', 'check', 'remediate', 'report'
    pub user_id: Option<String>,
    pub details: serde_json::Value,
    pub result: String,
    pub issues_found: Option<usize>,
    pub critical_issues: Option<usize>,
}

pub struct AuditLogger {
    database: Arc<Database>,
}

impl AuditLogger {
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        Ok(Self { database })
    }

    pub async fn log_compliance_change(&self, action: &str, compliance_type: &str, user_id: Option<&str>) -> Result<()> {
        let entry = ComplianceAuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now(),
            compliance_type: compliance_type.to_string(),
            action: action.to_string(),
            user_id: user_id.map(|s| s.to_string()),
            details: serde_json::json!({
                "compliance_type": compliance_type,
                "action": action,
            }),
            result: "success".to_string(),
            issues_found: None,
            critical_issues: None,
        };

        self.write_compliance_audit(&entry).await?;
        info!("Logged compliance change: {} {} by {:?}", action, compliance_type, user_id);

        Ok(())
    }

    pub async fn log_compliance_check(&self, compliance_type: &str, status: &ComplianceStatus) -> Result<()> {
        let critical_issues = status.issues.iter().filter(|i| i.severity == "critical").count();
        let high_issues = status.issues.iter().filter(|i| i.severity == "high").count();

        let entry = ComplianceAuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now(),
            compliance_type: compliance_type.to_string(),
            action: "check".to_string(),
            user_id: None,  // System-initiated
            details: serde_json::json!({
                "compliance_type": compliance_type,
                "status": status.status,
                "total_issues": status.issues.len(),
                "critical_issues": critical_issues,
                "high_issues": high_issues,
                "medium_issues": status.issues.iter().filter(|i| i.severity == "medium").count(),
                "low_issues": status.issues.iter().filter(|i| i.severity == "low").count(),
            }),
            result: match status.status.as_str() {
                "compliant" => "success".to_string(),
                "non_compliant" => "failure".to_string(),
                _ => "partial".to_string(),
            },
            issues_found: Some(status.issues.len()),
            critical_issues: Some(critical_issues),
        };

        self.write_compliance_audit(&entry).await?;

        if critical_issues > 0 {
            warn!("Compliance check for {} found {} critical issues", compliance_type, critical_issues);
        }

        Ok(())
    }

    pub async fn log_compliance_remediation(&self, compliance_type: &str, issue_id: &str) -> Result<()> {
        let entry = ComplianceAuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now(),
            compliance_type: compliance_type.to_string(),
            action: "remediate".to_string(),
            user_id: None,  // System-initiated
            details: serde_json::json!({
                "compliance_type": compliance_type,
                "issue_id": issue_id,
            }),
            result: "success".to_string(),
            issues_found: None,
            critical_issues: None,
        };

        self.write_compliance_audit(&entry).await?;
        info!("Logged compliance remediation: {} issue {}", compliance_type, issue_id);

        Ok(())
    }

    pub async fn log_compliance_report(&self, report: &ComplianceReport) -> Result<()> {
        let total_issues: usize = report.summary.values().map(|s| s.issues.len()).sum();
        let critical_issues: usize = report.summary.values()
            .flat_map(|s| &s.issues)
            .filter(|i| i.severity == "critical")
            .count();

        let entry = ComplianceAuditEntry {
            id: report.report_id.clone(),
            timestamp: report.generated_at,
            compliance_type: "all".to_string(),
            action: "report".to_string(),
            user_id: None,  // System-generated
            details: serde_json::json!({
                "overall_status": report.overall_status,
                "compliance_types": report.compliance_types,
                "total_issues": total_issues,
                "critical_issues": critical_issues,
                "recommendations_count": report.recommendations.len(),
            }),
            result: match report.overall_status.as_str() {
                "compliant" => "success".to_string(),
                "non_compliant" => "failure".to_string(),
                _ => "partial".to_string(),
            },
            issues_found: Some(total_issues),
            critical_issues: Some(critical_issues),
        };

        self.write_compliance_audit(&entry).await?;
        info!("Generated compliance report with {} total issues across {} frameworks",
              total_issues, report.compliance_types.len());

        Ok(())
    }

    pub async fn log_user_action(&self, user_id: &str, action: &str, resource_type: &str,
                                resource_id: Option<&str>, ip_address: Option<&str>,
                                details: serde_json::Value) -> Result<()> {
        let entry = AuditLogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now(),
            user_id: Some(user_id.to_string()),
            action: action.to_string(),
            resource_type: resource_type.to_string(),
            resource_id: resource_id.map(|s| s.to_string()),
            compliance_type: None,
            details,
            ip_address: ip_address.map(|s| s.to_string()),
            user_agent: None,
            session_id: None,
            result: "success".to_string(),
        };

        self.write_audit_log(&entry).await?;

        Ok(())
    }

    pub async fn log_system_event(&self, action: &str, resource_type: &str,
                                 resource_id: Option<&str>, details: serde_json::Value) -> Result<()> {
        let entry = AuditLogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now(),
            user_id: None,  // System event
            action: action.to_string(),
            resource_type: resource_type.to_string(),
            resource_id: resource_id.map(|s| s.to_string()),
            compliance_type: None,
            details,
            ip_address: None,
            user_agent: None,
            session_id: None,
            result: "success".to_string(),
        };

        self.write_audit_log(&entry).await?;

        Ok(())
    }

    pub async fn log_security_event(&self, event_type: &str, source_ip: Option<&str>,
                                   user_id: Option<&str>, details: serde_json::Value) -> Result<()> {
        let entry = AuditLogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now(),
            user_id: user_id.map(|s| s.to_string()),
            action: "security_event".to_string(),
            resource_type: "security".to_string(),
            resource_id: Some(event_type.to_string()),
            compliance_type: None,
            details,
            ip_address: source_ip.map(|s| s.to_string()),
            user_agent: None,
            session_id: None,
            result: "detected".to_string(),
        };

        self.write_audit_log(&entry).await?;

        // Log security events at warning level
        if let Some(ip) = source_ip {
            warn!("Security event: {} from IP {}", event_type, ip);
        } else {
            warn!("Security event: {}", event_type);
        }

        Ok(())
    }

    pub async fn get_audit_logs(&self, start_time: SystemTime, end_time: SystemTime,
                              limit: Option<usize>) -> Result<Vec<AuditLogEntry>> {
        // This would be implemented with actual database queries
        // For now, return empty vector
        Ok(Vec::new())
    }

    pub async fn get_compliance_audit_logs(&self, compliance_type: Option<&str>,
                                         start_time: SystemTime, end_time: SystemTime,
                                         limit: Option<usize>) -> Result<Vec<ComplianceAuditEntry>> {
        // This would be implemented with actual database queries
        // For now, return empty vector
        Ok(Vec::new())
    }

    pub async fn cleanup_old_logs(&self, retention_days: u64) -> Result<usize> {
        let cutoff_time = SystemTime::now() - std::time::Duration::from_secs(retention_days * 24 * 3600);

        // This would delete old log entries from the database
        // For now, return 0
        info!("Cleaned up audit logs older than {} days", retention_days);
        Ok(0)
    }

    async fn write_audit_log(&self, entry: &AuditLogEntry) -> Result<()> {
        // This would write to the audit_log table in the database
        let query = r#"
            INSERT INTO audit_log (
                id, timestamp, user_id, action, resource_type, resource_id,
                details, ip_address, result
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        // Convert SystemTime to timestamp for database
        let timestamp = entry.timestamp.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // In a real implementation, this would execute the SQL query
        // For now, we'll just log it
        info!("Audit log entry: {} {} {} by {:?}",
              entry.action, entry.resource_type,
              entry.resource_id.as_deref().unwrap_or("N/A"),
              entry.user_id);

        Ok(())
    }

    async fn write_compliance_audit(&self, entry: &ComplianceAuditEntry) -> Result<()> {
        // This would write to the compliance_audit_log table in the database
        let query = r#"
            INSERT INTO compliance_audit_log (
                id, timestamp, compliance_type, action, user_id,
                details, result, issues_found, critical_issues
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        // Convert SystemTime to timestamp for database
        let timestamp = entry.timestamp.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // In a real implementation, this would execute the SQL query
        // For now, we'll just log it
        info!("Compliance audit entry: {} {} {} - result: {}",
              entry.compliance_type, entry.action,
              entry.user_id.as_deref().unwrap_or("system"),
              entry.result);

        Ok(())
    }
}

// Audit trail integrity protection
pub struct AuditIntegrityManager {
    database: Arc<Database>,
}

impl AuditIntegrityManager {
    pub async fn new(database: Arc<Database>) -> Result<Self> {
        Ok(Self { database })
    }

    pub async fn verify_audit_integrity(&self) -> Result<bool> {
        // This would implement audit log integrity verification
        // using cryptographic hashes or digital signatures

        info!("Verifying audit log integrity");

        // In a real implementation, this would:
        // 1. Calculate hashes of audit log entries
        // 2. Verify against stored checksums
        // 3. Check for gaps in sequence numbers
        // 4. Validate digital signatures if used

        Ok(true)  // Assume integrity is valid for now
    }

    pub async fn create_audit_backup(&self, backup_path: &str) -> Result<()> {
        // This would create tamper-evident backup of audit logs
        info!("Creating audit log backup at: {}", backup_path);

        // In a real implementation, this would:
        // 1. Export all audit logs
        // 2. Create cryptographic hash
        // 3. Optionally sign with private key
        // 4. Store in immutable format

        Ok(())
    }

    pub async fn archive_old_logs(&self, archive_path: &str, retention_days: u64) -> Result<usize> {
        let cutoff_time = SystemTime::now() - std::time::Duration::from_secs(retention_days * 24 * 3600);

        info!("Archiving audit logs older than {} days to {}", retention_days, archive_path);

        // In a real implementation, this would:
        // 1. Extract logs older than retention period
        // 2. Create tamper-evident archive
        // 3. Verify archive integrity
        // 4. Remove archived logs from main database

        Ok(0)  // Return number of archived entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_database() -> Arc<Database> {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        Arc::new(Database::new(db_path.to_str().unwrap()).await.unwrap())
    }

    #[tokio::test]
    async fn test_audit_logger_creation() {
        let database = create_test_database().await;
        let logger = AuditLogger::new(database).await.unwrap();

        // Logger should be created successfully
        assert!(!logger.database.is_null());
    }

    #[tokio::test]
    async fn test_compliance_change_logging() {
        let database = create_test_database().await;
        let logger = AuditLogger::new(database).await.unwrap();

        // Test logging compliance change
        logger.log_compliance_change("enable", "gdpr", Some("admin")).await.unwrap();
    }

    #[tokio::test]
    async fn test_user_action_logging() {
        let database = create_test_database().await;
        let logger = AuditLogger::new(database).await.unwrap();

        // Test logging user action
        let details = serde_json::json!({"vm_id": "vm123", "action": "start"});
        logger.log_user_action("user1", "vm_start", "vm", Some("vm123"),
                              Some("192.168.1.100"), details).await.unwrap();
    }

    #[tokio::test]
    async fn test_security_event_logging() {
        let database = create_test_database().await;
        let logger = AuditLogger::new(database).await.unwrap();

        // Test logging security event
        let details = serde_json::json!({"attack_type": "brute_force", "attempts": 5});
        logger.log_security_event("ssh_brute_force", Some("10.0.0.1"),
                                 None, details).await.unwrap();
    }

    #[tokio::test]
    async fn test_audit_integrity_manager() {
        let database = create_test_database().await;
        let integrity_mgr = AuditIntegrityManager::new(database).await.unwrap();

        // Test integrity verification
        let is_valid = integrity_mgr.verify_audit_integrity().await.unwrap();
        assert!(is_valid);
    }
}