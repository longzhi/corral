//! Filesystem handlers

use anyhow::{anyhow, Result};
use corral_core::PolicyEngine;
use serde_json::Value;
use std::path::Path;

/// Handle filesystem operations
pub async fn handle(method: &str, params: &Value, policy: &PolicyEngine) -> Result<Value> {
    match method {
        "read" => read(params, policy).await,
        "write" => write(params, policy).await,
        "list" => list(params, policy).await,
        "stat" => stat(params, policy).await,
        "delete" => delete(params, policy).await,
        _ => Err(anyhow!("Unknown fs method: {}", method)),
    }
}

async fn read(params: &Value, policy: &PolicyEngine) -> Result<Value> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;

    // Check policy
    policy.check_file_read(path)?;

    // Read file
    let content = tokio::fs::read_to_string(path).await?;

    Ok(serde_json::json!({
        "content": content
    }))
}

async fn write(params: &Value, policy: &PolicyEngine) -> Result<Value> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;

    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'content' parameter"))?;

    // Check policy
    policy.check_file_write(path)?;

    // Ensure parent directory exists
    if let Some(parent) = Path::new(path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Write file
    tokio::fs::write(path, content).await?;

    Ok(serde_json::json!({
        "success": true
    }))
}

async fn list(params: &Value, policy: &PolicyEngine) -> Result<Value> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;

    // Check policy
    policy.check_file_read(path)?;

    // List directory
    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(path).await?;

    while let Some(entry) = read_dir.next_entry().await? {
        let metadata = entry.metadata().await?;
        entries.push(serde_json::json!({
            "name": entry.file_name().to_string_lossy(),
            "is_dir": metadata.is_dir(),
            "is_file": metadata.is_file(),
            "size": metadata.len(),
        }));
    }

    Ok(serde_json::json!({
        "entries": entries
    }))
}

async fn stat(params: &Value, policy: &PolicyEngine) -> Result<Value> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;

    // Check policy
    policy.check_file_read(path)?;

    // Get metadata
    let metadata = tokio::fs::metadata(path).await?;

    Ok(serde_json::json!({
        "is_dir": metadata.is_dir(),
        "is_file": metadata.is_file(),
        "size": metadata.len(),
        "readonly": metadata.permissions().readonly(),
    }))
}

async fn delete(params: &Value, policy: &PolicyEngine) -> Result<Value> {
    let path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;

    // Check policy
    policy.check_file_write(path)?;

    // Delete file or directory
    let metadata = tokio::fs::metadata(path).await?;
    if metadata.is_dir() {
        tokio::fs::remove_dir_all(path).await?;
    } else {
        tokio::fs::remove_file(path).await?;
    }

    Ok(serde_json::json!({
        "success": true
    }))
}
