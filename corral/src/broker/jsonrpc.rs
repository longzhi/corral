//! JSON-RPC 2.0 protocol implementation

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC request
#[derive(Debug, Deserialize)]
pub struct Request {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// JSON-RPC response
#[derive(Debug, Serialize)]
pub struct Response {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}

/// JSON-RPC error
#[derive(Debug, Serialize)]
pub struct Error {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC error codes
#[derive(Debug)]
#[allow(dead_code)]
pub enum ErrorCode {
    // Standard JSON-RPC errors
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,

    // Corral-specific errors
    PermissionDenied = -32001,
    ScopeViolation = -32002,
    RateLimited = -32003,
    Timeout = -32004,
    ServiceUnavailable = -32005,
    NetworkDenied = -32006,
    PathDenied = -32007,
}

impl Response {
    /// Create success response
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create error response
    pub fn error(id: Option<Value>, code: ErrorCode, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(Error {
                code: code as i32,
                message,
                data: None,
            }),
        }
    }

    /// Create response from Result
    pub fn from_result(id: Option<Value>, result: anyhow::Result<Value>) -> Self {
        match result {
            Ok(value) => Self::success(id, value),
            Err(e) => {
                // Determine error code from error message
                let error_msg = e.to_string();
                let code = if error_msg.contains("denied") || error_msg.contains("not permitted") {
                    ErrorCode::PermissionDenied
                } else if error_msg.contains("not found") {
                    ErrorCode::MethodNotFound
                } else if error_msg.contains("invalid") {
                    ErrorCode::InvalidParams
                } else if error_msg.contains("unavailable") {
                    ErrorCode::ServiceUnavailable
                } else {
                    ErrorCode::InternalError
                };

                Self::error(id, code, error_msg)
            }
        }
    }
}
