//! sandbox-call - SDK CLI for calling broker services from sandboxed scripts

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[derive(Parser)]
#[command(name = "sandbox-call")]
#[command(about = "Call sandbox broker services", long_about = None)]
struct Cli {
    /// Method to call (e.g., fs.read, network.http)
    method: String,

    /// JSON parameters (as key=value pairs or --json for raw JSON)
    #[arg(trailing_var_arg = true)]
    params: Vec<String>,

    /// Provide parameters as raw JSON
    #[arg(long)]
    json: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Get socket path from environment
    let socket_path =
        std::env::var("SANDBOX_SOCKET").context("SANDBOX_SOCKET environment variable not set")?;

    // Parse parameters
    let params = if cli.json {
        if cli.params.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&cli.params.join(" "))?
        }
    } else {
        parse_params(&cli.params)?
    };

    // Call broker
    let result = call_broker(&socket_path, &cli.method, params).await?;

    // Print result
    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}

/// Parse key=value parameters into JSON object
fn parse_params(params: &[String]) -> Result<Value> {
    if params.is_empty() {
        return Ok(Value::Null);
    }

    let mut map = serde_json::Map::new();

    for param in params {
        let parts: Vec<&str> = param.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(anyhow!(
                "Invalid parameter format: {}. Expected key=value",
                param
            ));
        }

        let key = parts[0].to_string();
        let value = parts[1];

        // Try to parse as JSON value, fallback to string
        let json_value =
            serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_string()));

        map.insert(key, json_value);
    }

    Ok(Value::Object(map))
}

/// Call the broker via Unix socket
async fn call_broker(socket_path: &str, method: &str, params: Value) -> Result<Value> {
    // Connect to broker
    let stream = UnixStream::connect(socket_path)
        .await
        .context("Failed to connect to broker socket")?;

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Build JSON-RPC request
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });

    // Send request
    let request_str = serde_json::to_string(&request)?;
    writer.write_all(request_str.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    // Read response
    let mut response_line = String::new();
    reader.read_line(&mut response_line).await?;

    // Parse response
    let response: serde_json::Value = serde_json::from_str(&response_line)?;

    // Check for error
    if let Some(error) = response.get("error") {
        let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(anyhow!("Broker error ({}): {}", code, message));
    }

    // Return result
    response
        .get("result")
        .cloned()
        .ok_or_else(|| anyhow!("No result in response"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_params() {
        let params = vec![
            "path=/tmp/test.txt".to_string(),
            "count=5".to_string(),
            "enabled=true".to_string(),
        ];

        let result = parse_params(&params).unwrap();
        assert_eq!(result["path"], "/tmp/test.txt");
        assert_eq!(result["count"], 5);
        assert_eq!(result["enabled"], true);
    }

    #[test]
    fn test_parse_empty_params() {
        let params = vec![];
        let result = parse_params(&params).unwrap();
        assert_eq!(result, Value::Null);
    }
}
