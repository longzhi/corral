//! Environment variable handler

use crate::policy::PolicyEngine;
use anyhow::{anyhow, Result};
use serde_json::Value;

/// Handle env operations
pub async fn handle(method: &str, params: &Value, policy: &PolicyEngine) -> Result<Value> {
    match method {
        "get" => get(params, policy).await,
        _ => Err(anyhow!("Unknown env method: {}", method)),
    }
}

async fn get(params: &Value, policy: &PolicyEngine) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'name' parameter"))?;

    // Check policy
    policy.check_env(name)?;

    // Get environment variable
    let value = std::env::var(name).ok();

    Ok(serde_json::json!({
        "name": name,
        "value": value,
    }))
}
