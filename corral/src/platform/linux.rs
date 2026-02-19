//! Linux sandbox using bubblewrap (bwrap)

use super::{ExecutionResult, Runtime};
use crate::broker::BrokerHandle;
use crate::manifest::Manifest;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

pub struct LinuxRuntime {
    manifest: Manifest,
    skill_path: PathBuf,
    work_dir: PathBuf,
    data_dir: PathBuf,
}

impl LinuxRuntime {
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

    /// Build bwrap command
    fn build_bwrap_command(&self, broker: &BrokerHandle) -> Command {
        let mut cmd = Command::new("bwrap");

        // Basic isolation
        cmd.arg("--unshare-all")
            .arg("--share-net") // Share network namespace (we control via broker)
            .arg("--die-with-parent");

        // Mount basic filesystems (read-only)
        cmd.arg("--ro-bind")
            .arg("/usr")
            .arg("/usr")
            .arg("--ro-bind")
            .arg("/lib")
            .arg("/lib")
            .arg("--ro-bind")
            .arg("/lib64")
            .arg("/lib64")
            .arg("--ro-bind")
            .arg("/bin")
            .arg("/bin")
            .arg("--ro-bind")
            .arg("/sbin")
            .arg("/sbin");

        // Create essential directories
        cmd.arg("--tmpfs")
            .arg("/tmp")
            .arg("--proc")
            .arg("/proc")
            .arg("--dev")
            .arg("/dev");

        // Mount skill directory (read-only)
        cmd.arg("--ro-bind").arg(&self.skill_path).arg("/skill");

        // Mount work directory (read-write)
        cmd.arg("--bind").arg(&self.work_dir).arg("/work");

        // Mount data directory (read-write)
        cmd.arg("--bind").arg(&self.data_dir).arg("/data");

        // Set working directory
        cmd.arg("--chdir").arg("/work");

        // Environment variables
        cmd.arg("--unsetenv")
            .arg("HOME")
            .arg("--setenv")
            .arg("SKILL_DIR")
            .arg("/skill")
            .arg("--setenv")
            .arg("WORK_DIR")
            .arg("/work")
            .arg("--setenv")
            .arg("DATA_DIR")
            .arg("/data")
            .arg("--setenv")
            .arg("SANDBOX_SOCKET")
            .arg(&broker.socket_path);

        // Add allowed environment variables
        if let Some(env_vars) = &self.manifest.permissions.env {
            for var in env_vars {
                if let Ok(value) = std::env::var(var) {
                    cmd.arg("--setenv").arg(var).arg(value);
                }
            }
        }

        // Disable network if not permitted
        if self.manifest.permissions.network.is_none() {
            cmd.arg("--unshare-net");
        }

        // Execute skill entry point
        let runtime = &self.manifest.runtime;
        let entry = format!("/skill/{}", self.manifest.entry);

        cmd.arg(runtime).arg(&entry);

        cmd
    }
}

#[async_trait::async_trait]
impl Runtime for LinuxRuntime {
    async fn execute(&self, broker: &BrokerHandle) -> Result<ExecutionResult> {
        let mut cmd = self.build_bwrap_command(broker);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let output = cmd.output().await.context("Failed to execute bwrap")?;

        // Cleanup work directory
        let _ = tokio::fs::remove_dir_all(&self.work_dir).await;

        Ok(ExecutionResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}
