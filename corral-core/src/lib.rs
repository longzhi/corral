//! Corral Core - Capability-based sandboxing library
//!
//! This library provides the core sandboxing engine for Corral, enabling fine-grained
//! permission control for untrusted code execution.
//!
//! # Example
//!
//! ```no_run
//! use corral_core::{Sandbox, SandboxBuilder};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let sandbox = SandboxBuilder::new()
//!         .fs_read(["/usr/**"])
//!         .fs_write(["/tmp/work/**"])
//!         .network_deny()
//!         .exec_allow(["python3", "bash"])
//!         .timeout(Duration::from_secs(30))
//!         .build()?;
//!
//!     let result = sandbox.execute("echo hello").await?;
//!     assert_eq!(result.stdout.trim(), "hello");
//!
//!     Ok(())
//! }
//! ```

pub mod permissions;
pub mod policy;
pub mod sandbox;

#[cfg(feature = "broker")]
pub mod broker;

// Re-exports for convenience
pub use permissions::{
    FsPermissions, NetworkPermissions, Permissions, PermissionsBuilder, ServicePermission,
};
pub use policy::PolicyEngine;
pub use sandbox::{ExecuteResult, Sandbox, SandboxBuilder, SandboxConfig};

#[cfg(feature = "broker")]
pub use broker::{BrokerConfig, BrokerHandle, BrokerStats, ServiceHandler, start_broker};
