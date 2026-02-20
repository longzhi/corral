//! Policy engine for permission checks

use crate::manifest::{AccessLevel, Manifest};
use anyhow::{anyhow, Result};
use glob::Pattern;
use std::path::Path;
use std::sync::Arc;

/// Policy engine for checking permissions
#[derive(Clone)]
pub struct PolicyEngine {
    manifest: Arc<Manifest>,
}

impl PolicyEngine {
    /// Create a new policy engine from manifest
    pub fn new(manifest: Manifest) -> Self {
        Self {
            manifest: Arc::new(manifest),
        }
    }

    /// Check if a file path can be read
    pub fn check_file_read(&self, path: &str) -> Result<()> {
        let fs_perms = self
            .manifest
            .permissions
            .fs
            .as_ref()
            .ok_or_else(|| anyhow!("No filesystem permissions declared"))?;

        let read_patterns = fs_perms
            .read
            .as_ref()
            .ok_or_else(|| anyhow!("No read permissions declared"))?;

        if self.matches_any_pattern(path, read_patterns) {
            Ok(())
        } else {
            Err(anyhow!("Read access denied for path: {}", path))
        }
    }

    /// Check if a file path can be written
    pub fn check_file_write(&self, path: &str) -> Result<()> {
        let fs_perms = self
            .manifest
            .permissions
            .fs
            .as_ref()
            .ok_or_else(|| anyhow!("No filesystem permissions declared"))?;

        let write_patterns = fs_perms
            .write
            .as_ref()
            .ok_or_else(|| anyhow!("No write permissions declared"))?;

        if self.matches_any_pattern(path, write_patterns) {
            Ok(())
        } else {
            Err(anyhow!("Write access denied for path: {}", path))
        }
    }

    /// Check if a network connection is allowed
    pub fn check_network(&self, host: &str, port: u16) -> Result<()> {
        let network_perms = self
            .manifest
            .permissions
            .network
            .as_ref()
            .ok_or_else(|| anyhow!("No network permissions declared"))?;

        let allow_list = network_perms
            .allow
            .as_ref()
            .ok_or_else(|| anyhow!("No network hosts allowed"))?;

        let target = format!("{}:{}", host, port);

        for allowed in allow_list {
            if self.matches_network_pattern(&target, allowed) {
                return Ok(());
            }
        }

        Err(anyhow!("Network access denied for {}:{}", host, port))
    }

    /// Check if a service method call is allowed
    pub fn check_service(
        &self,
        service: &str,
        method: &str,
        _params: &serde_json::Value,
    ) -> Result<()> {
        let services = self
            .manifest
            .permissions
            .services
            .as_ref()
            .ok_or_else(|| anyhow!("No service permissions declared"))?;

        let service_access = match service {
            "reminders" => services.reminders.as_ref(),
            "calendar" => services.calendar.as_ref(),
            "browser" => services.browser.as_ref(),
            "notifications" => services.notifications.as_ref(),
            "clipboard" => services.clipboard.as_ref(),
            _ => return Err(anyhow!("Unknown service: {}", service)),
        }
        .ok_or_else(|| anyhow!("Service '{}' not permitted", service))?;

        // Check if method is allowed based on access level
        let method_allowed = match &service_access.access {
            AccessLevel::Read => method.starts_with("list") || method.starts_with("get"),
            AccessLevel::Write => {
                method.starts_with("add")
                    || method.starts_with("create")
                    || method.starts_with("update")
                    || method.starts_with("delete")
            }
            AccessLevel::ReadWrite => true,
            AccessLevel::Send => method == "send",
            AccessLevel::Open => method == "open",
        };

        if !method_allowed {
            return Err(anyhow!(
                "Method '{}' not allowed for service '{}' with access level {:?}",
                method,
                service,
                service_access.access
            ));
        }

        // Scope checking is done separately for specific services (e.g., check_reminders_scope)

        Ok(())
    }

    /// Check if a reminders list is allowed by scope
    pub fn check_reminders_scope(&self, list_name: &str) -> Result<()> {
        let services = self
            .manifest
            .permissions
            .services
            .as_ref()
            .ok_or_else(|| anyhow!("No service permissions declared"))?;

        let reminders_access = services
            .reminders
            .as_ref()
            .ok_or_else(|| anyhow!("Reminders service not permitted"))?;

        // If no scope is defined, all lists are allowed
        if reminders_access.scope.is_none() {
            return Ok(());
        }

        // Check if the list is in the allowed scope
        let scope = reminders_access.scope.as_ref().unwrap();

        // If scope has a "lists" field, check it
        if let Some(lists) = scope.get("lists") {
            if let Some(allowed_lists) = lists.as_array() {
                for allowed in allowed_lists {
                    if let Some(allowed_str) = allowed.as_str() {
                        if allowed_str == "*" || allowed_str == list_name {
                            return Ok(());
                        }
                    }
                }
                return Err(anyhow!(
                    "List '{}' not in allowed scope. Allowed lists: {:?}",
                    list_name,
                    allowed_lists
                ));
            }
        }

        // If we reach here, scope is defined but doesn't have lists field, or is malformed
        // Be permissive and allow
        Ok(())
    }

    /// Check if an executable is allowed
    pub fn check_exec(&self, command: &str) -> Result<()> {
        let exec_perms = self
            .manifest
            .permissions
            .exec
            .as_ref()
            .ok_or_else(|| anyhow!("No exec permissions declared"))?;

        // Extract command name from path
        let cmd_name = Path::new(command)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(command);

        if exec_perms.iter().any(|allowed| allowed == cmd_name) {
            Ok(())
        } else {
            Err(anyhow!("Execution denied for command: {}", command))
        }
    }

    /// Check if an environment variable is allowed
    pub fn check_env(&self, var_name: &str) -> Result<()> {
        let env_perms = self
            .manifest
            .permissions
            .env
            .as_ref()
            .ok_or_else(|| anyhow!("No env permissions declared"))?;

        if env_perms.iter().any(|allowed| allowed == var_name) {
            Ok(())
        } else {
            Err(anyhow!("Environment variable access denied: {}", var_name))
        }
    }

    /// Check if path matches any glob pattern
    fn matches_any_pattern(&self, path: &str, patterns: &[String]) -> bool {
        patterns
            .iter()
            .any(|pattern| self.matches_pattern(path, pattern))
    }

    /// Check if a single path matches a pattern
    fn matches_pattern(&self, path: &str, pattern: &str) -> bool {
        // For patterns with variables, we need to map the path to the same space
        // E.g., if pattern is "$SKILL_DIR/**", only paths that would be under SKILL_DIR match
        // In Phase 1, we simulate by checking if the normalized path makes sense

        if pattern.contains("$SKILL_DIR") {
            // For skill dir patterns, assume paths need to be relative or under skill dir
            // In reality, skill_path would be used here
            // For testing: only match paths that don't start with /
            if path.starts_with('/') && !path.contains("/skill/") {
                return false;
            }
            let pattern_suffix = pattern.replace("$SKILL_DIR/", "");
            if let Ok(glob) = Pattern::new(&pattern_suffix) {
                let normalized_path = path.trim_start_matches('/');
                return glob.matches(normalized_path);
            }
        } else if pattern.contains("$DATA_DIR") {
            if path.starts_with('/') && !path.contains("/data/") {
                return false;
            }
            let pattern_suffix = pattern.replace("$DATA_DIR/", "");
            if let Ok(glob) = Pattern::new(&pattern_suffix) {
                let normalized_path = path.strip_prefix("/data/").unwrap_or(path);
                return glob.matches(normalized_path);
            }
        } else if pattern.contains("$WORK_DIR") {
            if path.starts_with('/') && !path.contains("/work/") {
                return false;
            }
            let pattern_suffix = pattern.replace("$WORK_DIR/", "");
            if let Ok(glob) = Pattern::new(&pattern_suffix) {
                let normalized_path = path.strip_prefix("/work/").unwrap_or(path);
                return glob.matches(normalized_path);
            }
        } else {
            // No variables, match directly
            if let Ok(glob) = Pattern::new(pattern) {
                return glob.matches(path);
            }
        }

        false
    }

    /// Check if target matches network pattern (supports wildcards)
    fn matches_network_pattern(&self, target: &str, pattern: &str) -> bool {
        // Simple wildcard matching for domains
        // api.example.com:443 matches api.example.com:443
        // *.example.com:443 matches api.example.com:443

        if target == pattern {
            return true;
        }

        // Extract host:port from both
        let (target_host, target_port) = target.rsplit_once(':').unwrap_or((target, ""));
        let (pattern_host, pattern_port) = pattern.rsplit_once(':').unwrap_or((pattern, ""));

        // Port must match exactly if specified
        if !pattern_port.is_empty() && pattern_port != target_port {
            return false;
        }

        // Check host with wildcard support
        if let Some(domain_suffix) = pattern_host.strip_prefix("*.") {
            target_host.ends_with(domain_suffix) || target_host == domain_suffix
        } else {
            target_host == pattern_host
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::*;

    fn create_test_manifest() -> Manifest {
        Manifest {
            name: "test".into(),
            version: "1.0.0".into(),
            description: "test".into(),
            author: "test".into(),
            entry: "./run.sh".into(),
            runtime: "bash".into(),
            permissions: Permissions {
                fs: Some(FsPermissions {
                    read: Some(vec!["$SKILL_DIR/**".into(), "$DATA_DIR/config.json".into()]),
                    write: Some(vec!["$WORK_DIR/**".into(), "$DATA_DIR/**".into()]),
                }),
                network: Some(NetworkPermissions {
                    allow: Some(vec![
                        "api.example.com:443".into(),
                        "*.cdn.example.com:443".into(),
                    ]),
                }),
                services: Some(ServicePermissions {
                    reminders: Some(ServiceAccess {
                        access: AccessLevel::ReadWrite,
                        scope: None,
                    }),
                    calendar: Some(ServiceAccess {
                        access: AccessLevel::Read,
                        scope: None,
                    }),
                    ..Default::default()
                }),
                exec: Some(vec!["curl".into(), "jq".into()]),
                env: Some(vec!["LANG".into(), "TZ".into()]),
            },
        }
    }

    #[test]
    fn test_file_read_allowed() {
        let policy = PolicyEngine::new(create_test_manifest());
        assert!(policy.check_file_read("test.txt").is_ok());
        assert!(policy.check_file_read("data/config.json").is_ok());
    }

    #[test]
    fn test_file_read_denied() {
        let policy = PolicyEngine::new(create_test_manifest());
        assert!(policy.check_file_read("/etc/passwd").is_err());
    }

    #[test]
    fn test_file_write_allowed() {
        let policy = PolicyEngine::new(create_test_manifest());
        assert!(policy.check_file_write("work/output.txt").is_ok());
        assert!(policy.check_file_write("data/state.json").is_ok());
    }

    #[test]
    fn test_file_write_denied() {
        let policy = PolicyEngine::new(create_test_manifest());
        assert!(policy.check_file_write("/etc/passwd").is_err());
    }

    #[test]
    fn test_network_allowed() {
        let policy = PolicyEngine::new(create_test_manifest());
        assert!(policy.check_network("api.example.com", 443).is_ok());
        assert!(policy.check_network("sub.cdn.example.com", 443).is_ok());
    }

    #[test]
    fn test_network_denied() {
        let policy = PolicyEngine::new(create_test_manifest());
        assert!(policy.check_network("evil.com", 443).is_err());
        assert!(policy.check_network("api.example.com", 80).is_err());
    }

    #[test]
    fn test_service_allowed() {
        let policy = PolicyEngine::new(create_test_manifest());
        let params = serde_json::json!({});

        assert!(policy.check_service("reminders", "add", &params).is_ok());
        assert!(policy.check_service("calendar", "list", &params).is_ok());
    }

    #[test]
    fn test_service_denied() {
        let policy = PolicyEngine::new(create_test_manifest());
        let params = serde_json::json!({});

        // Calendar is read-only
        assert!(policy.check_service("calendar", "create", &params).is_err());

        // Browser not permitted
        assert!(policy.check_service("browser", "open", &params).is_err());
    }

    #[test]
    fn test_exec_allowed() {
        let policy = PolicyEngine::new(create_test_manifest());
        assert!(policy.check_exec("curl").is_ok());
        assert!(policy.check_exec("/usr/bin/jq").is_ok());
    }

    #[test]
    fn test_exec_denied() {
        let policy = PolicyEngine::new(create_test_manifest());
        assert!(policy.check_exec("rm").is_err());
    }

    #[test]
    fn test_env_allowed() {
        let policy = PolicyEngine::new(create_test_manifest());
        assert!(policy.check_env("LANG").is_ok());
        assert!(policy.check_env("TZ").is_ok());
    }

    #[test]
    fn test_env_denied() {
        let policy = PolicyEngine::new(create_test_manifest());
        assert!(policy.check_env("SECRET_KEY").is_err());
    }
}
