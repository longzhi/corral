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
        // Check for network access patterns when network is denied
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

        // Strip heredoc content before checking paths.
        // Heredocs embed arbitrary text that should not be scanned for path tokens.
        let command_without_heredoc = Self::strip_heredoc_content(command);

        // Check for absolute paths that aren't in the allowed read/write list
        for raw in command_without_heredoc.split_whitespace() {
            let token = raw.trim_matches(|c: char| {
                matches!(c, '\'' | '"' | ',' | ')' | '(' | '[' | ']' | '{' | '}' | ';')
            });
            // Skip bare "/" — it's an arithmetic operator or separator, not a path
            if token == "/" {
                continue;
            }
            if token.starts_with('/')
                && !self.policy.check_path_read(token)
                && !self.policy.check_path_write(token)
            {
                anyhow::bail!("Path access denied for: {}", token);
            }
        }

        Ok(())
    }

    /// Strip heredoc body content from a command string.
    ///
    /// Detects patterns like `<<EOF`, `<<'EOF'`, `<<"EOF"`, `<<-EOF` and removes
    /// everything from the heredoc start through the closing tag (or end of string).
    /// Only the command portion before the heredoc body is returned for scanning.
    fn strip_heredoc_content(command: &str) -> String {
        let mut result = String::new();
        let mut remaining = command;

        loop {
            // Find the next `<<` operator
            let Some(heredoc_pos) = remaining.find("<<") else {
                result.push_str(remaining);
                break;
            };

            // Keep everything before the `<<`
            result.push_str(&remaining[..heredoc_pos]);

            let after_arrows = &remaining[heredoc_pos + 2..];

            // Skip optional `-` (for <<-EOF)
            let after_dash = after_arrows.strip_prefix('-').unwrap_or(after_arrows);

            // Skip optional whitespace
            let after_ws = after_dash.trim_start();

            // Extract the tag: strip optional quotes, then read alphanumeric/underscore
            let (tag, after_tag_and_quote) = Self::extract_heredoc_tag(after_ws);

            if tag.is_empty() {
                // Not a heredoc (e.g., `<<` used for bit shift or something else)
                result.push_str("<<");
                remaining = after_arrows;
                continue;
            }

            // Drop the `<<[-][\'\"?]TAG[\'\"?]` from output since it's part of the heredoc syntax
            // We already pushed everything before `<<` into result.

            // Now find the closing tag on its own line (after a newline)
            let end_pattern = format!("\n{}\n", tag);
            let end_pattern_eof = format!("\n{}", tag);

            // Calculate how far we consumed after the original `<<`
            let consumed_for_tag = remaining.len() - after_tag_and_quote.len();
            let heredoc_body_start = &remaining[consumed_for_tag..];

            if let Some(pos) = heredoc_body_start.find(&end_pattern) {
                remaining = &heredoc_body_start[pos + end_pattern.len()..];
            } else if heredoc_body_start.ends_with(&end_pattern_eof) {
                remaining = "";
            } else {
                // No closing tag found — treat the rest as heredoc content
                remaining = "";
            }
        }

        result
    }

    /// Extract a heredoc tag from the text after `<<` (and optional `-`/whitespace).
    /// Returns (tag, remaining_after_tag).
    fn extract_heredoc_tag(s: &str) -> (&str, &str) {
        let (quote_char, tag_start) = if let Some(rest) = s.strip_prefix('\'') {
            (Some('\''), rest)
        } else if let Some(rest) = s.strip_prefix('"') {
            (Some('"'), rest)
        } else {
            (None, s)
        };

        // Read tag characters (alphanumeric + underscore)
        let tag_end = tag_start
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(tag_start.len());

        let tag = &tag_start[..tag_end];

        if tag.is_empty() {
            return ("", s);
        }

        let after_tag = &tag_start[tag_end..];

        // Skip closing quote if present
        let after_quote = match quote_char {
            Some(q) => after_tag.strip_prefix(q).unwrap_or(after_tag),
            None => after_tag,
        };

        (tag, after_quote)
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

    #[test]
    fn test_preflight_ignores_slash_in_heredoc_body() {
        let sandbox = SandboxBuilder::new()
            .fs_read(["/tmp/**"])
            .fs_write(["/tmp/**"])
            .build()
            .unwrap();
        let command = "cat > /tmp/note.md <<'EOF'\nWeather: 11C / -1C\nSome text with / in it\nEOF";

        let result = sandbox.preflight_command(command);
        assert!(result.is_ok(), "heredoc content should not trigger path check: {:?}", result.err());
    }

    #[test]
    fn test_preflight_ignores_absolute_path_in_heredoc_body() {
        let sandbox = SandboxBuilder::new()
            .fs_read(["/tmp/**"])
            .fs_write(["/tmp/**"])
            .build()
            .unwrap();
        let command = "cat > /tmp/note.md <<EOF\nDo not treat /etc/passwd as command arg\nEOF";

        let result = sandbox.preflight_command(command);
        assert!(result.is_ok(), "heredoc content should not trigger path check: {:?}", result.err());
    }

    #[test]
    fn test_preflight_still_checks_real_absolute_path_argument() {
        let sandbox = SandboxBuilder::new().build().unwrap();
        let result = sandbox.preflight_command("cat /etc/passwd");

        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("Path access denied for: /etc/passwd")
        );
    }

    #[test]
    fn test_preflight_skips_standalone_slash_token() {
        let sandbox = SandboxBuilder::new().build().unwrap();
        let result = sandbox.preflight_command("echo 11 / -1");

        assert!(result.is_ok());
    }
}
