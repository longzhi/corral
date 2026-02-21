//! Sandbox struct and builder API

use crate::permissions::Permissions;
use crate::policy::PolicyEngine;
use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Configuration for sandbox execution
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    pub permissions: Permissions,
    pub work_dir: PathBuf,
    pub data_dir: Option<PathBuf>,
    pub timeout: Duration,
    pub max_memory_mb: Option<u64>,
    pub env_vars: HashMap<String, String>,
    pub broker_socket: Option<PathBuf>,
}

/// Result of command execution
#[derive(Debug, Clone)]
pub struct ExecuteResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
    pub was_killed: bool,
}

/// Sandbox for executing commands with fine-grained permissions
pub struct Sandbox {
    config: SandboxConfig,
    policy: PolicyEngine,
    owns_work_dir: bool,
}

impl Sandbox {
    /// Create a new sandbox with the given configuration
    pub fn new(config: SandboxConfig) -> Result<Self> {
        // Create work_dir if it doesn't exist
        let owns_work_dir = !config.work_dir.exists();
        std::fs::create_dir_all(&config.work_dir)
            .with_context(|| format!("Failed to create work directory: {:?}", config.work_dir))?;

        // Create data_dir if specified
        if let Some(ref data_dir) = config.data_dir {
            std::fs::create_dir_all(data_dir)
                .with_context(|| format!("Failed to create data directory: {:?}", data_dir))?;
        }

        // Initialize PolicyEngine from permissions
        let policy = PolicyEngine::new(config.permissions.clone());

        Ok(Self {
            config,
            policy,
            owns_work_dir,
        })
    }

    /// Execute a single command in the sandbox
    pub async fn execute(&self, command: &str) -> Result<ExecuteResult> {
        self.execute_with_timeout(command, self.config.timeout)
            .await
    }

    /// Execute with custom timeout (overrides config)
    pub async fn execute_with_timeout(
        &self,
        command: &str,
        timeout: Duration,
    ) -> Result<ExecuteResult> {
        let start = Instant::now();

        self.preflight_command(command)?;

        // Use platform-specific execution
        #[cfg(target_os = "linux")]
        let result = self.execute_linux(command, timeout).await?;

        #[cfg(target_os = "macos")]
        let result = self.execute_macos(command, timeout).await?;

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            anyhow::bail!("Unsupported platform");
        }

        let duration = start.elapsed();

        Ok(ExecuteResult {
            exit_code: result.0,
            stdout: result.1,
            stderr: result.2,
            duration,
            was_killed: result.3,
        })
    }

    /// Get a reference to the policy engine
    pub fn policy(&self) -> &PolicyEngine {
        &self.policy
    }

    /// Get the work directory path
    pub fn work_dir(&self) -> &PathBuf {
        &self.config.work_dir
    }

    /// Get the data directory path (if set)
    pub fn data_dir(&self) -> Option<&PathBuf> {
        self.config.data_dir.as_ref()
    }

    /// Explicitly shut down and clean up
    pub async fn shutdown(self) -> Result<()> {
        // Cleanup is handled by Drop, but we can force it here
        drop(self);
        Ok(())
    }

    fn preflight_command(&self, command: &str) -> Result<()> {
        let command_heads = command
            .split(|c| ['|', '&', ';', '\n'].contains(&c))
            .filter_map(|seg| seg.split_whitespace().next());

        for head in command_heads {
            if !self.policy.check_exec(head) {
                anyhow::bail!("Execution denied for command: {}", head);
            }
        }

        if self.config.permissions.network.allow.is_empty()
            && (command.contains("http://")
                || command.contains("https://")
                || command.contains(" curl ")
                || command.starts_with("curl ")
                || command.contains(" wget ")
                || command.starts_with("wget "))
        {
            anyhow::bail!("Network access denied by sandbox policy");
        }

        for raw in command.split_whitespace() {
            let token = raw.trim_matches(|c: char| {
                matches!(c, '\'' | '"' | ',' | ')' | '(' | '[' | ']' | '{' | '}' | ';')
            });
            if token.starts_with('/')
                && !self.policy.check_path_read(token)
                && !self.policy.check_path_write(token)
            {
                anyhow::bail!("Path access denied for: {}", token);
            }
        }

        Ok(())
    }

    fn serialize_policy(&self) -> Result<String> {
        Ok(json!({
            "fs": {
                "read": self.config.permissions.fs.read,
                "write": self.config.permissions.fs.write,
            },
            "network": {
                "allow": self.config.permissions.network.allow,
            },
            "exec": self.config.permissions.exec,
        })
        .to_string())
    }

    #[cfg(target_os = "macos")]
    fn libsandbox_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("libsandbox/libsandbox.dylib")
    }

    #[cfg(target_os = "linux")]
    fn libsandbox_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("libsandbox/libsandbox.so")
    }

    #[cfg(target_os = "linux")]
    async fn execute_linux(
        &self,
        command: &str,
        timeout: Duration,
    ) -> Result<(i32, String, String, bool)> {
        use std::os::unix::fs::PermissionsExt;
        use tokio::process::Command;
        use tokio::time::timeout as tokio_timeout;

        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command).current_dir(&self.config.work_dir);

        cmd.env_clear();
        cmd.env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");

        for (key, value) in &self.config.env_vars {
            if self.policy.check_env(key) {
                cmd.env(key, value);
            }
        }

        let libsandbox = Self::libsandbox_path();
        if libsandbox.exists() {
            let policy_json = self.serialize_policy()?;
            let policy_file =
                std::env::temp_dir().join(format!("corral-policy-{}.json", std::process::id()));
            std::fs::write(&policy_file, &policy_json)?;
            let mut perms = std::fs::metadata(&policy_file)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&policy_file, perms)?;

            cmd.env("LD_PRELOAD", &libsandbox);
            cmd.env("SANDBOX_POLICY_FILE", &policy_file);
        }

        // Pass broker socket if configured
        if let Some(ref socket_path) = self.config.broker_socket {
            cmd.env("SANDBOX_SOCKET", socket_path);
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn().context("Failed to spawn process")?;

        match tokio_timeout(timeout, child.wait_with_output()).await {
            Ok(Ok(output)) => Ok((
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stdout).to_string(),
                String::from_utf8_lossy(&output.stderr).to_string(),
                false,
            )),
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Ok((-1, String::new(), "Timeout exceeded".to_string(), true)),
        }
    }

    #[cfg(target_os = "macos")]
    async fn execute_macos(
        &self,
        command: &str,
        timeout: Duration,
    ) -> Result<(i32, String, String, bool)> {
        use std::os::unix::fs::PermissionsExt;
        use tokio::process::Command;
        use tokio::time::timeout as tokio_timeout;

        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd.current_dir(&self.config.work_dir);

        cmd.env_clear();
        cmd.env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");

        for (key, value) in &self.config.env_vars {
            if self.policy.check_env(key) {
                cmd.env(key, value);
            }
        }

        let libsandbox = Self::libsandbox_path();
        if libsandbox.exists() {
            let policy_json = self.serialize_policy()?;
            let policy_file =
                std::env::temp_dir().join(format!("corral-policy-{}.json", std::process::id()));
            std::fs::write(&policy_file, &policy_json)?;
            let mut perms = std::fs::metadata(&policy_file)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&policy_file, perms)?;

            cmd.env("DYLD_INSERT_LIBRARIES", &libsandbox);
            cmd.env("DYLD_FORCE_FLAT_NAMESPACE", "1");
            cmd.env("SANDBOX_POLICY_FILE", &policy_file);
        }

        // Pass broker socket if configured
        if let Some(ref socket_path) = self.config.broker_socket {
            cmd.env("SANDBOX_SOCKET", socket_path);
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn().context("Failed to spawn process")?;

        match tokio_timeout(timeout, child.wait_with_output()).await {
            Ok(Ok(output)) => Ok((
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stdout).to_string(),
                String::from_utf8_lossy(&output.stderr).to_string(),
                false,
            )),
            Ok(Err(e)) => Err(e.into()),
            Err(_) => Ok((-1, String::new(), "Timeout exceeded".to_string(), true)),
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // Clean up work_dir if we own it
        if self.owns_work_dir && self.config.work_dir.exists() {
            let _ = std::fs::remove_dir_all(&self.config.work_dir);
        }
    }
}

/// Builder API for Sandbox
#[derive(Debug)]
pub struct SandboxBuilder {
    permissions_builder: crate::permissions::PermissionsBuilder,
    work_dir: Option<PathBuf>,
    data_dir: Option<PathBuf>,
    timeout: Duration,
    max_memory_mb: Option<u64>,
    env_vars: HashMap<String, String>,
    broker_socket: Option<PathBuf>,
}

impl Default for SandboxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SandboxBuilder {
    /// Create a new sandbox builder
    pub fn new() -> Self {
        Self {
            permissions_builder: Permissions::builder(),
            work_dir: None,
            data_dir: None,
            timeout: Duration::from_secs(30),
            max_memory_mb: None,
            env_vars: HashMap::new(),
            broker_socket: None,
        }
    }

    /// Add filesystem read patterns
    pub fn fs_read<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.permissions_builder = self.permissions_builder.fs_read(patterns);
        self
    }

    /// Add filesystem write patterns
    pub fn fs_write<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.permissions_builder = self.permissions_builder.fs_write(patterns);
        self
    }

    /// Add allowed network hosts
    pub fn network_allow<I, S>(mut self, hosts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.permissions_builder = self.permissions_builder.network_allow(hosts);
        self
    }

    /// Deny all network access
    pub fn network_deny(mut self) -> Self {
        self.permissions_builder = self.permissions_builder.network_deny();
        self
    }

    /// Add allowed executables
    pub fn exec_allow<I, S>(mut self, cmds: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.permissions_builder = self.permissions_builder.exec_allow(cmds);
        self
    }

    /// Set work directory
    pub fn work_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.work_dir = Some(path.into());
        self
    }

    /// Set data directory
    pub fn data_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.data_dir = Some(path.into());
        self
    }

    /// Set timeout
    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = duration;
        self
    }

    /// Set max memory (MB)
    pub fn max_memory_mb(mut self, mb: u64) -> Self {
        self.max_memory_mb = Some(mb);
        self
    }

    /// Add environment variable
    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env_vars.insert(key.to_string(), value.to_string());
        self
    }

    /// Set broker socket path for service calls
    pub fn broker_socket(mut self, path: impl Into<PathBuf>) -> Self {
        self.broker_socket = Some(path.into());
        self
    }

    /// Build the Sandbox
    pub fn build(self) -> Result<Sandbox> {
        let work_dir = self
            .work_dir
            .unwrap_or_else(|| std::env::temp_dir().join(format!("corral-{}", std::process::id())));

        let config = SandboxConfig {
            permissions: self.permissions_builder.build(),
            work_dir,
            data_dir: self.data_dir,
            timeout: self.timeout,
            max_memory_mb: self.max_memory_mb,
            env_vars: self.env_vars,
            broker_socket: self.broker_socket,
        };

        Sandbox::new(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_builder() {
        let sandbox = SandboxBuilder::new()
            .fs_read(["/usr/**"])
            .fs_write(["/tmp/**"])
            .network_deny()
            .exec_allow(["bash"])
            .timeout(Duration::from_secs(10))
            .build();

        assert!(sandbox.is_ok());
        let sandbox = sandbox.unwrap();
        assert!(sandbox.work_dir().exists());
        assert!(!sandbox.policy().check_env("SECRET"));
    }

    #[tokio::test]
    #[ignore] // Requires sh/echo to be available in PATH
    async fn test_sandbox_execute_simple() {
        let sandbox = SandboxBuilder::new()
            .exec_allow(["sh", "echo"])
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();

        let result = sandbox.execute("echo hello").await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.trim(), "hello");
        assert!(!result.was_killed);
    }

    #[tokio::test]
    #[ignore] // Requires sh/sleep to be available
    async fn test_sandbox_execute_timeout() {
        let sandbox = SandboxBuilder::new()
            .exec_allow(["sh", "sleep"])
            .timeout(Duration::from_millis(100))
            .build()
            .unwrap();

        let result = sandbox.execute("sleep 10").await.unwrap();
        assert!(result.was_killed);
    }

    #[tokio::test]
    #[ignore] // Integration test requiring command execution
    async fn test_sandbox_work_dir_persistence() {
        let temp_dir = std::env::temp_dir().join(format!("test-corral-{}", std::process::id()));

        let sandbox = SandboxBuilder::new()
            .work_dir(&temp_dir)
            .exec_allow(["sh", "touch"])
            .build()
            .unwrap();

        sandbox.execute("touch test.txt").await.unwrap();

        // File should exist in work_dir
        assert!(temp_dir.join("test.txt").exists());

        // Clean up
        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
