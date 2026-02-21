//! macOS sandbox using DYLD_INSERT_LIBRARIES

use super::{ExecutionResult, Runtime};
#[cfg(feature = "broker")]
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

    /// Serialize manifest permissions to JSON policy for libsandbox
    fn serialize_policy(manifest: &Manifest) -> Result<String> {
        use serde_json::json;

        let mut read_paths = Vec::new();
        let mut write_paths = Vec::new();
        let mut network_allow = Vec::new();
        let mut exec_paths = Vec::new();

        // File system permissions
        if let Some(fs) = &manifest.permissions.fs {
            if let Some(read) = &fs.read {
                read_paths.extend(read.iter().cloned());
            }
            if let Some(write) = &fs.write {
                write_paths.extend(write.iter().cloned());
            }
        }

        // Network permissions
        if let Some(network) = &manifest.permissions.network {
            if let Some(allow) = &network.allow {
                network_allow.extend(allow.iter().cloned());
            }
        }

        // Exec permissions
        if let Some(exec) = &manifest.permissions.exec {
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
}

impl MacOSRuntime {
    /// Common execute logic with optional broker socket
    async fn execute_internal(&self, broker_socket: Option<PathBuf>) -> Result<ExecutionResult> {
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
            .env("DATA_DIR", &self.data_dir);

        // Add broker socket if provided
        if let Some(socket) = broker_socket {
            cmd.env("SANDBOX_SOCKET", socket);
        }

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

        // libsandbox interposition (Phase 2)
        let libsandbox = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("libsandbox/libsandbox.dylib");

        if libsandbox.exists() {
            // Serialize policy to JSON for libsandbox
            let policy_json = Self::serialize_policy(&self.manifest)?;

            // Write policy to temporary file with restricted permissions (mode 0600)
            let policy_file =
                std::env::temp_dir().join(format!("corral-policy-{}.json", std::process::id()));

            use std::os::unix::fs::PermissionsExt;
            std::fs::write(&policy_file, &policy_json)?;
            let mut perms = std::fs::metadata(&policy_file)?.permissions();
            perms.set_mode(0o600); // Owner read/write only
            std::fs::set_permissions(&policy_file, perms)?;

            cmd.env("DYLD_INSERT_LIBRARIES", &libsandbox);
            cmd.env("DYLD_FORCE_FLAT_NAMESPACE", "1");
            cmd.env("SANDBOX_POLICY_FILE", &policy_file);
        }

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

#[async_trait::async_trait]
impl Runtime for MacOSRuntime {
    #[cfg(feature = "broker")]
    async fn execute(&self, broker: &BrokerHandle) -> Result<ExecutionResult> {
        self.execute_internal(Some(broker.socket_path.clone()))
            .await
    }

    #[cfg(not(feature = "broker"))]
    async fn execute_no_broker(&self) -> Result<ExecutionResult> {
        self.execute_internal(None).await
    }
}
