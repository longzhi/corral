//! Exec handler

use crate::policy::PolicyEngine;
use anyhow::{anyhow, Result};
use serde_json::Value;

/// Handle exec operations
pub async fn handle(method: &str, params: &Value, policy: &PolicyEngine) -> Result<Value> {
    match method {
        "run" => run(params, policy).await,
        _ => Err(anyhow!("Unknown exec method: {}", method)),
    }
}

async fn run(params: &Value, policy: &PolicyEngine) -> Result<Value> {
    let command = params
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'command' parameter"))?;

    // Check policy
    policy.check_exec(command)?;

    let args = params
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Execute command
    let mut cmd = tokio::process::Command::new(command);
    cmd.args(&args);

    if let Some(cwd) = params.get("cwd").and_then(|v| v.as_str()) {
        cmd.current_dir(cwd);
    }

    let output = cmd.output().await?;

    Ok(serde_json::json!({
        "exit_code": output.status.code().unwrap_or(-1),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
    }))
}
