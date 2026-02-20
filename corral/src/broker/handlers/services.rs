//! Service adapter handlers

use crate::adapters::reminders;
use anyhow::{anyhow, Result};
use corral_core::PolicyEngine;
use serde_json::{json, Value};

/// Handle reminders service calls
pub async fn handle_reminders(
    method: &str,
    params: &Value,
    policy: &PolicyEngine,
) -> Result<Value> {
    // Check policy - determines if service is allowed
    policy.check_service_result("reminders", method, params)?;

    // Create adapter
    let adapter = reminders::create_adapter();

    // Check if available on this platform
    if !adapter.is_available() {
        return Err(anyhow!("Reminders service not available on this platform"));
    }

    // Route to appropriate method
    match method {
        "list" => {
            let list_params: reminders::ListParams = serde_json::from_value(params.clone())
                .map_err(|e| anyhow!("Invalid parameters for reminders.list: {}", e))?;

            // Check scope if defined
            if let Some(list_name) = &list_params.list {
                policy.check_reminders_scope_result(list_name)?;
            }

            let reminders = adapter.list(list_params).await?;
            Ok(json!({ "reminders": reminders }))
        }
        "add" => {
            let add_params: reminders::AddParams = serde_json::from_value(params.clone())
                .map_err(|e| anyhow!("Invalid parameters for reminders.add: {}", e))?;

            // Check scope
            policy.check_reminders_scope_result(&add_params.list)?;

            let reminder = adapter.add(add_params).await?;
            Ok(json!({ "reminder": reminder }))
        }
        "update" => {
            let update_params: reminders::UpdateParams = serde_json::from_value(params.clone())
                .map_err(|e| anyhow!("Invalid parameters for reminders.update: {}", e))?;

            let reminder = adapter.update(update_params).await?;
            Ok(json!({ "reminder": reminder }))
        }
        "complete" => {
            let id_params: reminders::IdParams = serde_json::from_value(params.clone())
                .map_err(|e| anyhow!("Invalid parameters for reminders.complete: {}", e))?;

            let reminder = adapter.complete(id_params).await?;
            Ok(json!({ "reminder": reminder }))
        }
        "delete" => {
            let id_params: reminders::IdParams = serde_json::from_value(params.clone())
                .map_err(|e| anyhow!("Invalid parameters for reminders.delete: {}", e))?;

            adapter.delete(id_params).await?;
            Ok(json!({ "success": true }))
        }
        _ => Err(anyhow!("Unknown reminders method: {}", method)),
    }
}

/// Handle calendar service calls
pub async fn handle_calendar(method: &str, params: &Value, policy: &PolicyEngine) -> Result<Value> {
    // Check policy
    policy.check_service_result("calendar", method, params)?;

    // Return stub - service unavailable
    Err(anyhow!(
        "Service unavailable: calendar.{} not implemented (stub)",
        method
    ))
}

/// Handle browser service calls
pub async fn handle_browser(method: &str, params: &Value, policy: &PolicyEngine) -> Result<Value> {
    // Check policy
    policy.check_service_result("browser", method, params)?;

    // Return stub - service unavailable
    Err(anyhow!(
        "Service unavailable: browser.{} not implemented (stub)",
        method
    ))
}

/// Handle notifications service calls
pub async fn handle_notifications(
    method: &str,
    params: &Value,
    policy: &PolicyEngine,
) -> Result<Value> {
    // Check policy
    policy.check_service_result("notifications", method, params)?;

    // Return stub - service unavailable
    Err(anyhow!(
        "Service unavailable: notifications.{} not implemented (stub)",
        method
    ))
}

/// Handle clipboard service calls
pub async fn handle_clipboard(
    method: &str,
    params: &Value,
    policy: &PolicyEngine,
) -> Result<Value> {
    // Check policy
    policy.check_service_result("clipboard", method, params)?;

    // Return stub - service unavailable
    Err(anyhow!(
        "Service unavailable: clipboard.{} not implemented (stub)",
        method
    ))
}
