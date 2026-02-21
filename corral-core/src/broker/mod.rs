//! Broker module - JSON-RPC service broker for sandboxed processes
//!
//! The broker listens on a Unix socket and handles service calls from sandboxed processes.
//! It enforces policy checks before dispatching to service handlers.

pub mod jsonrpc;

use crate::PolicyEngine;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;

/// Broker handle returned after starting the broker
#[derive(Clone)]
pub struct BrokerHandle {
    pub socket_path: PathBuf,
    pub stats: Arc<RwLock<BrokerStats>>,
}

/// Broker call statistics
#[derive(Default, Clone, Debug)]
pub struct BrokerStats {
    pub total_calls: usize,
    pub allowed_calls: usize,
    pub denied_calls: usize,
    pub calls_by_method: HashMap<String, usize>,
}

/// Service handler trait - implement this to add custom services
#[async_trait]
pub trait ServiceHandler: Send + Sync {
    /// Handle a service method call
    async fn handle(
        &self,
        method: &str,
        params: &Value,
        policy: &PolicyEngine,
    ) -> Result<Value>;

    /// Check if this handler supports the given namespace
    fn namespace(&self) -> &str;
}

/// Broker server configuration
pub struct BrokerConfig {
    pub policy: PolicyEngine,
    pub handlers: HashMap<String, Arc<dyn ServiceHandler>>,
}

impl BrokerConfig {
    pub fn new(policy: PolicyEngine) -> Self {
        Self {
            policy,
            handlers: HashMap::new(),
        }
    }

    /// Register a service handler
    pub fn register_handler(&mut self, handler: Arc<dyn ServiceHandler>) {
        self.handlers.insert(handler.namespace().to_string(), handler);
    }
}

/// Start the broker server
pub async fn start_broker(config: BrokerConfig) -> Result<BrokerHandle> {
    let socket_path = create_socket_path()?;

    // Remove old socket if exists
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path)?;
    // Broker listening on socket_path

    let stats = Arc::new(RwLock::new(BrokerStats::default()));
    let handle = BrokerHandle {
        socket_path: socket_path.clone(),
        stats: stats.clone(),
    };

    let policy = Arc::new(config.policy);
    let handlers = Arc::new(config.handlers);

    // Spawn broker task
    tokio::spawn(async move {
        if let Err(e) = broker_loop(listener, policy, handlers, stats).await {
            eprintln!("Broker error: {}", e);
        }
    });

    Ok(handle)
}

/// Main broker loop
async fn broker_loop(
    listener: UnixListener,
    policy: Arc<PolicyEngine>,
    handlers: Arc<HashMap<String, Arc<dyn ServiceHandler>>>,
    stats: Arc<RwLock<BrokerStats>>,
) -> Result<()> {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let policy = policy.clone();
                let handlers = handlers.clone();
                let stats = stats.clone();
                tokio::spawn(async move {
                    if let Err(_e) = handle_connection(stream, policy, handlers, stats).await {
                        // Connection closed or error - ignore
                    }
                });
            }
            Err(e) => {
                eprintln!("Accept error: {}", e);
            }
        }
    }
}

/// Handle a single connection
async fn handle_connection(
    stream: UnixStream,
    policy: Arc<PolicyEngine>,
    handlers: Arc<HashMap<String, Arc<dyn ServiceHandler>>>,
    stats: Arc<RwLock<BrokerStats>>,
) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;

        if n == 0 {
            break;
        }

        let response = match serde_json::from_str::<jsonrpc::Request>(&line) {
            Ok(request) => {
                // Update stats
                {
                    let mut s = stats.write().await;
                    s.total_calls += 1;
                    *s.calls_by_method.entry(request.method.clone()).or_insert(0) += 1;
                }

                // Route request
                let result = route_request(&request, &policy, &handlers).await;

                // Update stats based on result
                {
                    let mut s = stats.write().await;
                    if result.is_ok() {
                        s.allowed_calls += 1;
                    } else {
                        s.denied_calls += 1;
                    }
                }

                jsonrpc::Response::from_result(request.id, result)
            }
            Err(e) => jsonrpc::Response::error(
                None,
                jsonrpc::ErrorCode::InvalidRequest,
                format!("Invalid JSON-RPC request: {}", e),
            ),
        };

        let response_str = serde_json::to_string(&response)?;
        writer.write_all(response_str.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
    }

    Ok(())
}

/// Route a request to the appropriate handler
async fn route_request(
    request: &jsonrpc::Request,
    policy: &PolicyEngine,
    handlers: &HashMap<String, Arc<dyn ServiceHandler>>,
) -> Result<Value> {
    let parts: Vec<&str> = request.method.split('.').collect();

    if parts.len() != 2 {
        return Err(anyhow!("Invalid method format: {}", request.method));
    }

    let namespace = parts[0];
    let method = parts[1];
    let params = request.params.as_ref().unwrap_or(&Value::Null);

    if let Some(handler) = handlers.get(namespace) {
        handler.handle(method, params, policy).await
    } else {
        Err(anyhow!("Unknown namespace: {}", namespace))
    }
}

/// Create temporary socket path
fn create_socket_path() -> Result<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .or_else(|_| std::env::var("TMPDIR"))
        .unwrap_or_else(|_| "/tmp".to_string());

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let socket_name = format!("corral-broker-{}-{}.sock", std::process::id(), nanos);
    Ok(PathBuf::from(runtime_dir).join(socket_name))
}

/// Stop the broker and clean up the socket
impl Drop for BrokerHandle {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Permissions;

    struct EchoHandler;

    #[async_trait]
    impl ServiceHandler for EchoHandler {
        async fn handle(
            &self,
            method: &str,
            params: &Value,
            _policy: &PolicyEngine,
        ) -> Result<Value> {
            Ok(serde_json::json!({
                "method": method,
                "params": params
            }))
        }

        fn namespace(&self) -> &str {
            "echo"
        }
    }

    #[tokio::test]
    async fn test_broker_config() {
        let policy = PolicyEngine::new(Permissions::builder().build());
        let mut config = BrokerConfig::new(policy);

        config.register_handler(Arc::new(EchoHandler));

        assert!(config.handlers.contains_key("echo"));
    }
}
