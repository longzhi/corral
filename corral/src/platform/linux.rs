//! Linux sandbox using bubblewrap (bwrap) or LD_PRELOAD

use super::{ExecutionResult, Runtime};
use crate::broker::BrokerHandle;
use crate::manifest::Manifest;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

/// Linux isolation mode
#[derive(Debug, Clone, Copy)]
pub enum LinuxIsolationMode {
    /// Kernel-level isolation using bubblewrap (stronger, requires bwrap installed)
    Bwrap,
    /// Userspace interposition using LD_PRELOAD libsandbox.so (weaker, but no dependencies)
    Preload,
}

pub struct LinuxRuntime {
    manifest: Manifest,
    skill_path: PathBuf,
    work_dir: PathBuf,
    data_dir: PathBuf,
    mode: LinuxIsolationMode,
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

        // Auto-detect isolation mode: prefer bwrap if available, fallback to preload
        let mode = if which::which("bwrap").is_ok() {
            LinuxIsolationMode::Bwrap
        } else {
            LinuxIsolationMode::Preload
        };

        Ok(Self {
            manifest,
            skill_path,
            work_dir,
            data_dir,
            mode,
        })
    }

    /// Serialize manifest permissions to JSON policy for libsandbox
    fn serialize_policy(&self) -> Result<String> {
        use serde_json::json;

        let mut read_paths = Vec::new();
        let mut write_paths = Vec::new();
        let mut network_allow = Vec::new();
        let mut exec_paths = Vec::new();

        // File system permissions
        if let Some(fs) = &self.manifest.permissions.fs {
            if let Some(read) = &fs.read {
                read_paths.extend(read.iter().cloned());
            }
            if let Some(write) = &fs.write {
                write_paths.extend(write.iter().cloned());
            }
        }

        // Network permissions
        if let Some(network) = &self.manifest.permissions.network {
            if let Some(allow) = &network.allow {
                network_allow.extend(allow.iter().cloned());
            }
        }

        // Exec permissions
        if let Some(exec) = &self.manifest.permissions.exec {
            exec_paths.extend(exec.iter().cloned());
        }

        let policy = json!({
            "fs": {
                "read": read_paths,
                "write": write_paths
            },
            "network": {
                "allow": network_allow
            },
            "exec": exec_paths
        });

        Ok(policy.to_string())
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

    /// Build LD_PRELOAD command with libsandbox.so
    fn build_preload_command(&self, broker: &BrokerHandle) -> Result<Command> {
        let mut cmd = Command::new(&self.manifest.runtime);

        // Entry point
        let entry = self.skill_path.join(&self.manifest.entry);
        cmd.arg(&entry);

        // Set working directory
        cmd.current_dir(&self.work_dir);

        // Environment variables
        cmd.env_clear();

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

        // libsandbox preload
        let libsandbox = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("libsandbox/libsandbox.so");

        if libsandbox.exists() {
            let policy_json = self.serialize_policy()?;
            cmd.env("LD_PRELOAD", &libsandbox);
            cmd.env("SANDBOX_POLICY", policy_json);
        }

        // Process group isolation
        use nix::unistd::{setpgid, Pid};
        unsafe {
            cmd.pre_exec(|| {
                setpgid(Pid::from_raw(0), Pid::from_raw(0))?;
                Ok(())
            });
        }

        Ok(cmd)
    }
}

#[async_trait::async_trait]
impl Runtime for LinuxRuntime {
    async fn execute(&self, broker: &BrokerHandle) -> Result<ExecutionResult> {
        let mut cmd = match self.mode {
            LinuxIsolationMode::Bwrap => self.build_bwrap_command(broker),
            LinuxIsolationMode::Preload => self.build_preload_command(broker)?,
        };

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
