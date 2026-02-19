//! Service adapter stubs

use crate::policy::PolicyEngine;
use anyhow::{anyhow, Result};
use serde_json::Value;

/// Handle reminders service calls
pub async fn handle_reminders(
    method: &str,
    params: &Value,
    policy: &PolicyEngine,
) -> Result<Value> {
    // Check policy
    policy.check_service("reminders", method, params)?;

    // Return stub - service unavailable
    Err(anyhow!(
        "Service unavailable: reminders.{} not implemented (stub)",
        method
    ))
}

/// Handle calendar service calls
pub async fn handle_calendar(method: &str, params: &Value, policy: &PolicyEngine) -> Result<Value> {
    // Check policy
    policy.check_service("calendar", method, params)?;

    // Return stub - service unavailable
    Err(anyhow!(
        "Service unavailable: calendar.{} not implemented (stub)",
        method
    ))
}

/// Handle browser service calls
pub async fn handle_browser(method: &str, params: &Value, policy: &PolicyEngine) -> Result<Value> {
    // Check policy
    policy.check_service("browser", method, params)?;

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
    policy.check_service("notifications", method, params)?;

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
    policy.check_service("clipboard", method, params)?;

    // Return stub - service unavailable
    Err(anyhow!(
        "Service unavailable: clipboard.{} not implemented (stub)",
        method
    ))
}
