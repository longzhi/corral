//! JSON-RPC broker for sandbox communication

pub mod handlers;
pub mod jsonrpc;
pub mod router;

use crate::policy::PolicyEngine;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;
use tracing::{error, info};

/// Broker handle for communication
pub struct BrokerHandle {
    pub socket_path: PathBuf,
    pub stats: Arc<RwLock<BrokerStats>>,
}

/// Broker call statistics
#[derive(Default, Clone)]
pub struct BrokerStats {
    pub total_calls: usize,
    pub allowed_calls: usize,
    pub denied_calls: usize,
    pub calls_by_method: std::collections::HashMap<String, usize>,
}

/// Start the broker server
pub async fn start_broker(policy: PolicyEngine) -> Result<BrokerHandle> {
    let socket_path = create_socket_path()?;

    // Remove old socket if exists
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path)?;
    info!("Broker listening on {:?}", socket_path);

    let stats = Arc::new(RwLock::new(BrokerStats::default()));
    let handle = BrokerHandle {
        socket_path: socket_path.clone(),
        stats: stats.clone(),
    };

    // Spawn broker task
    tokio::spawn(async move {
        if let Err(e) = broker_loop(listener, policy, stats).await {
            error!("Broker error: {}", e);
        }
    });

    Ok(handle)
}

/// Main broker loop
async fn broker_loop(
    listener: UnixListener,
    policy: PolicyEngine,
    stats: Arc<RwLock<BrokerStats>>,
) -> Result<()> {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let policy = policy.clone();
                let stats = stats.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, policy, stats).await {
                        error!("Connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Accept error: {}", e);
            }
        }
    }
}

/// Handle a single connection
async fn handle_connection(
    stream: UnixStream,
    policy: PolicyEngine,
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
            // Connection closed
            break;
        }

        // Parse JSON-RPC request
        let response = match serde_json::from_str::<jsonrpc::Request>(&line) {
            Ok(request) => {
                // Update stats
                {
                    let mut s = stats.write().await;
                    s.total_calls += 1;
                    *s.calls_by_method.entry(request.method.clone()).or_insert(0) += 1;
                }

                // Route request
                let result = router::route_request(&request, &policy).await;

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

        // Send response
        let response_str = serde_json::to_string(&response)?;
        writer.write_all(response_str.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
    }

    Ok(())
}

/// Create temporary socket path
fn create_socket_path() -> Result<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .or_else(|_| std::env::var("TMPDIR"))
        .unwrap_or_else(|_| "/tmp".to_string());

    let socket_name = format!("corral-broker-{}.sock", std::process::id());
    Ok(PathBuf::from(runtime_dir).join(socket_name))
}
