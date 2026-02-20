//! Watchdog for resource monitoring and enforcement

use crate::manifest::Manifest;
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Watchdog for monitoring resource usage
pub struct Watchdog {
    #[allow(dead_code)]
    manifest: Arc<Manifest>,
    stopped: Arc<AtomicBool>,
}

impl Watchdog {
    /// Create a new watchdog
    pub fn new(manifest: Manifest) -> Self {
        let stopped = Arc::new(AtomicBool::new(false));

        // TODO: Watchdog implementation pending
        // Future features:
        // - Timeout enforcement
        // - Memory monitoring
        // - CPU usage tracking
        // - Rate limiting

        Self {
            manifest: Arc::new(manifest),
            stopped: stopped.clone(),
        }
    }

    /// Stop the watchdog
    pub fn stop(&self) -> Result<()> {
        self.stopped.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Permissions;

    #[test]
    fn test_watchdog_create() {
        let manifest = Manifest {
            name: "test".into(),
            version: "1.0.0".into(),
            description: "test".into(),
            author: "test".into(),
            entry: "./run.sh".into(),
            runtime: "bash".into(),
            permissions: Permissions::default(),
        };

        let watchdog = Watchdog::new(manifest);
        assert!(watchdog.stop().is_ok());
    }
}
