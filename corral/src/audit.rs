//! Audit logging for execution

#[cfg(feature = "broker")]
use crate::broker::BrokerHandle;
use crate::manifest::Manifest;
use anyhow::Result;
use serde_json::json;
use std::path::PathBuf;

/// Log execution results to audit file (with broker stats)
#[cfg(feature = "broker")]
pub async fn log_execution(
    manifest: &Manifest,
    broker: &BrokerHandle,
    exit_code: i32,
) -> Result<()> {
    let log_dir = get_audit_log_dir()?;
    std::fs::create_dir_all(&log_dir)?;

    let log_file = log_dir.join(format!(
        "audit-{}.jsonl",
        chrono::Utc::now().format("%Y%m%d")
    ));

    let stats = broker.stats.read().await;

    let entry = json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "skill": manifest.name,
        "version": manifest.version,
        "exit_code": exit_code,
        "broker_stats": {
            "total_calls": stats.total_calls,
            "allowed_calls": stats.allowed_calls,
            "denied_calls": stats.denied_calls,
            "calls_by_method": stats.calls_by_method,
        }
    });

    // Append to JSONL file
    let entry_str = serde_json::to_string(&entry)?;
    tokio::fs::write(&log_file, format!("{}\n", entry_str)).await?;

    Ok(())
}

/// Log execution results to audit file (without broker)
#[cfg(not(feature = "broker"))]
pub async fn log_execution_simple(manifest: &Manifest, exit_code: i32) -> Result<()> {
    let log_dir = get_audit_log_dir()?;
    std::fs::create_dir_all(&log_dir)?;

    let log_file = log_dir.join(format!(
        "audit-{}.jsonl",
        chrono::Utc::now().format("%Y%m%d")
    ));

    let entry = json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "skill": manifest.name,
        "version": manifest.version,
        "exit_code": exit_code,
    });

    // Append to JSONL file
    let entry_str = serde_json::to_string(&entry)?;
    tokio::fs::write(&log_file, format!("{}\n", entry_str)).await?;

    Ok(())
}

/// Get audit log directory
fn get_audit_log_dir() -> Result<PathBuf> {
    let dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("corral")
        .join("audit");
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_log_dir() {
        let dir = get_audit_log_dir().unwrap();
        assert!(dir.to_string_lossy().contains("corral"));
        assert!(dir.to_string_lossy().contains("audit"));
    }
}
