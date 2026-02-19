//! macOS sandbox using DYLD_INSERT_LIBRARIES

use super::{ExecutionResult, Runtime};
use crate::broker::BrokerHandle;
use crate::manifest::Manifest;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

pub struct MacOSRuntime {
    manifest: Manifest,
    skill_path: PathBuf,
    work_dir: PathBuf,
    data_dir: PathBuf,
}

impl MacOSRuntime {
    pub fn new(manifest: Manifest, skill_path: PathBuf) -> Result<Self> {
        // Create temporary work directory
        let work_dir = std::env::temp_dir().join(format!("corral-work-{}", std::process::id()));
        std::fs::create_dir_all(&work_dir)?;

        // Create persistent data directory
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("corral")
            .join("skills")
            .join(&manifest.name);
        std::fs::create_dir_all(&data_dir)?;

        Ok(Self {
            manifest,
            skill_path,
            work_dir,
            data_dir,
        })
    }
}

#[async_trait::async_trait]
impl Runtime for MacOSRuntime {
    async fn execute(&self, broker: &BrokerHandle) -> Result<ExecutionResult> {
        let mut cmd = Command::new(&self.manifest.runtime);

        // Entry point
        let entry = self.skill_path.join(&self.manifest.entry);
        cmd.arg(&entry);

        // Set working directory
        cmd.current_dir(&self.work_dir);

        // Environment variables
        cmd.env_clear(); // Start with clean environment

        cmd.env("SKILL_DIR", &self.skill_path)
            .env("WORK_DIR", &self.work_dir)
            .env("DATA_DIR", &self.data_dir)
            .env("SANDBOX_SOCKET", &broker.socket_path);

        // Add allowed environment variables
        if let Some(env_vars) = &self.manifest.permissions.env {
            for var in env_vars {
                if let Ok(value) = std::env::var(var) {
                    cmd.env(var, value);
                }
            }
        }

        // Essential system variables
        cmd.env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");

        // TODO: DYLD_INSERT_LIBRARIES with libsandbox.dylib
        // For Phase 1, we rely on broker-based isolation only
        // let libsandbox = "/path/to/libsandbox.dylib";
        // if Path::new(libsandbox).exists() {
        //     cmd.env("DYLD_INSERT_LIBRARIES", libsandbox);
        //     cmd.env("DYLD_FORCE_FLAT_NAMESPACE", "1");
        // }

        // Process group isolation
        use nix::unistd::{setpgid, Pid};
        unsafe {
            cmd.pre_exec(|| {
                // Create new process group
                setpgid(Pid::from_raw(0), Pid::from_raw(0))?;
                Ok(())
            });
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let output = cmd.output().await.context("Failed to execute skill")?;

        // Cleanup work directory
        let _ = tokio::fs::remove_dir_all(&self.work_dir).await;

        Ok(ExecutionResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}
