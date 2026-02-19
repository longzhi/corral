//! Request router - dispatches JSON-RPC methods to handlers

use super::handlers;
use super::jsonrpc::Request;
use crate::policy::PolicyEngine;
use anyhow::{anyhow, Result};
use serde_json::Value;

/// Route a JSON-RPC request to the appropriate handler
pub async fn route_request(request: &Request, policy: &PolicyEngine) -> Result<Value> {
    let parts: Vec<&str> = request.method.split('.').collect();

    if parts.len() != 2 {
        return Err(anyhow!("Invalid method format: {}", request.method));
    }

    let namespace = parts[0];
    let method = parts[1];
    let params = request.params.as_ref().unwrap_or(&Value::Null);

    match namespace {
        "fs" => handlers::fs::handle(method, params, policy).await,
        "network" => handlers::network::handle(method, params, policy).await,
        "reminders" => handlers::services::handle_reminders(method, params, policy).await,
        "calendar" => handlers::services::handle_calendar(method, params, policy).await,
        "browser" => handlers::services::handle_browser(method, params, policy).await,
        "notifications" => handlers::services::handle_notifications(method, params, policy).await,
        "clipboard" => handlers::services::handle_clipboard(method, params, policy).await,
        "exec" => handlers::exec::handle(method, params, policy).await,
        "env" => handlers::env::handle(method, params, policy).await,
        _ => Err(anyhow!("Unknown namespace: {}", namespace)),
    }
}
