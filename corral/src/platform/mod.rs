//! Platform-specific sandbox implementations

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

use crate::broker::BrokerHandle;
use crate::manifest::Manifest;
use anyhow::Result;
use std::path::Path;

/// Result of skill execution
pub struct ExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Platform runtime interface
#[async_trait::async_trait]
pub trait Runtime {
    async fn execute(&self, broker: &BrokerHandle) -> Result<ExecutionResult>;
}

/// Create platform-specific runtime
pub fn create_runtime(
    manifest: &Manifest,
    skill_path: &Path,
) -> Result<Box<dyn Runtime + Send + Sync>> {
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(linux::LinuxRuntime::new(
            manifest.clone(),
            skill_path.to_path_buf(),
        )?))
    }

    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacOSRuntime::new(
            manifest.clone(),
            skill_path.to_path_buf(),
        )?))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        anyhow::bail!("Unsupported platform")
    }
}
