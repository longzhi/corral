//! Network handlers

use anyhow::{anyhow, Result};
use corral_core::PolicyEngine;
use serde_json::Value;

/// Handle network operations
pub async fn handle(method: &str, params: &Value, policy: &PolicyEngine) -> Result<Value> {
    match method {
        "http" => http(params, policy).await,
        "download" => download(params, policy).await,
        _ => Err(anyhow!("Unknown network method: {}", method)),
    }
}

async fn http(params: &Value, policy: &PolicyEngine) -> Result<Value> {
    let method = params
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET");

    let url = params
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'url' parameter"))?;

    // Parse URL to check host and port
    let parsed_url = url::Url::parse(url)?;
    let host = parsed_url
        .host_str()
        .ok_or_else(|| anyhow!("Invalid URL: no host"))?;
    let port = parsed_url
        .port_or_known_default()
        .ok_or_else(|| anyhow!("Invalid URL: no port"))?;

    // Check policy
    policy.check_network_result(host, port)?;

    // Build HTTP client
    let client = reqwest::Client::new();
    let mut request = match method.to_uppercase().as_str() {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "DELETE" => client.delete(url),
        "PATCH" => client.patch(url),
        _ => return Err(anyhow!("Unsupported HTTP method: {}", method)),
    };

    // Add headers if provided
    if let Some(headers) = params.get("headers").and_then(|v| v.as_object()) {
        for (key, value) in headers {
            if let Some(value_str) = value.as_str() {
                request = request.header(key, value_str);
            }
        }
    }

    // Add body if provided
    if let Some(body) = params.get("body") {
        if body.is_string() {
            request = request.body(body.as_str().unwrap().to_string());
        } else {
            request = request.json(body);
        }
    }

    // Execute request
    let response = request.send().await?;
    let status = response.status().as_u16();
    let headers: std::collections::HashMap<String, String> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let body = response.text().await?;

    Ok(serde_json::json!({
        "status": status,
        "headers": headers,
        "body": body,
    }))
}

async fn download(params: &Value, policy: &PolicyEngine) -> Result<Value> {
    let url = params
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'url' parameter"))?;

    let save_to = params
        .get("saveTo")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'saveTo' parameter"))?;

    // Parse URL and check network policy
    let parsed_url = url::Url::parse(url)?;
    let host = parsed_url
        .host_str()
        .ok_or_else(|| anyhow!("Invalid URL: no host"))?;
    let port = parsed_url
        .port_or_known_default()
        .ok_or_else(|| anyhow!("Invalid URL: no port"))?;

    policy.check_network_result(host, port)?;

    // Check file write policy
    policy.check_file_write(save_to)?;

    // Download file
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;
    let bytes = response.bytes().await?;

    // Write to file
    tokio::fs::write(save_to, bytes).await?;

    Ok(serde_json::json!({
        "success": true,
        "path": save_to,
    }))
}
