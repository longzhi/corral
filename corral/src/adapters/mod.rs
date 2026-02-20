//! Service adapters for system services
//!
//! Adapters provide a platform-agnostic interface to system services
//! (reminders, calendar, browser, etc.). Each adapter has platform-specific
//! implementations that are selected at runtime.

pub mod reminders;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Base trait for service adapters
#[async_trait]
pub trait ServiceAdapter: Send + Sync {
    /// Execute a method call on this service
    async fn execute(&self, method: &str, params: &Value) -> Result<Value>;
    
    /// Check if this adapter is available on the current platform
    fn is_available(&self) -> bool;
    
    /// Get the service name
    fn service_name(&self) -> &str;
}
