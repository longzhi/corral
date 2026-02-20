//! Policy engine for permission checks

use crate::permissions::Permissions;
use anyhow::{anyhow, Result};
use glob::Pattern;
use std::path::Path;
use std::sync::Arc;

/// Policy engine for checking permissions
#[derive(Clone)]
pub struct PolicyEngine {
    permissions: Arc<Permissions>,
}

impl PolicyEngine {
    /// Create a new policy engine from permissions
    pub fn new(permissions: Permissions) -> Self {
        Self {
            permissions: Arc::new(permissions),
        }
    }

    /// Check if a file path can be read
    pub fn check_path_read(&self, path: &str) -> bool {
        self.matches_any_pattern(path, &self.permissions.fs.read)
    }

    /// Check if a file path can be written
    pub fn check_path_write(&self, path: &str) -> bool {
        self.matches_any_pattern(path, &self.permissions.fs.write)
    }

    /// Check if a network connection is allowed
    pub fn check_network(&self, host: &str, port: u16) -> bool {
        let target = format!("{}:{}", host, port);

        for allowed in &self.permissions.network.allow {
            if self.matches_network_pattern(&target, allowed) {
                return true;
            }
        }

        false
    }

    /// Check if an executable is allowed
    pub fn check_exec(&self, command: &str) -> bool {
        // Extract command name from path
        let cmd_name = Path::new(command)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(command);

        self.permissions
            .exec
            .iter()
            .any(|allowed| allowed == cmd_name)
    }

    /// Check if an environment variable is allowed
    pub fn check_env(&self, var_name: &str) -> bool {
        self.permissions
            .env
            .iter()
            .any(|allowed| allowed == var_name)
    }

    /// Check if a service method call is allowed
    pub fn check_service(&self, service: &str, action: &str) -> bool {
        if let Some(service_perm) = self.permissions.services.get(service) {
            // Check if action is allowed based on access level
            match service_perm.access.as_str() {
                "read" => action.starts_with("list") || action.starts_with("get"),
                "write" => {
                    action.starts_with("add")
                        || action.starts_with("create")
                        || action.starts_with("update")
                        || action.starts_with("delete")
                }
                "readwrite" => true,
                "send" => action == "send",
                "open" => action == "open",
                _ => false,
            }
        } else {
            false
        }
    }

    /// Check if a reminders list is allowed by scope
    pub fn check_reminders_scope(&self, list_name: &str) -> bool {
        if let Some(reminders_perm) = self.permissions.services.get("reminders") {
            // If no scope is defined, all lists are allowed
            if reminders_perm.scope.is_empty() {
                return true;
            }

            // Check if the list is in the allowed scope
            if let Some(lists) = reminders_perm.scope.get("lists") {
                if let Some(allowed_lists) = lists.as_array() {
                    for allowed in allowed_lists {
                        if let Some(allowed_str) = allowed.as_str() {
                            if allowed_str == "*" || allowed_str == list_name {
                                return true;
                            }
                        }
                    }
                    return false;
                }
            }

            // If scope is defined but doesn't have lists field, allow
            true
        } else {
            false
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

        if pattern.contains("$SKILL_DIR") {
            // For skill dir patterns, assume paths need to be relative or under skill dir
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

    /// Get a reference to the underlying permissions
    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }
}

// Additional Result-based API methods for backward compatibility with existing broker code
impl PolicyEngine {
    /// Check file read (Result-based API for backward compatibility)
    pub fn check_file_read(&self, path: &str) -> Result<()> {
        if self.check_path_read(path) {
            Ok(())
        } else {
            Err(anyhow!("Read access denied for path: {}", path))
        }
    }

    /// Check file write (Result-based API for backward compatibility)
    pub fn check_file_write(&self, path: &str) -> Result<()> {
        if self.check_path_write(path) {
            Ok(())
        } else {
            Err(anyhow!("Write access denied for path: {}", path))
        }
    }

    /// Check network access (Result-based, for broker compatibility)
    pub fn check_network_result(&self, host: &str, port: u16) -> Result<()> {
        if self.check_network(host, port) {
            Ok(())
        } else {
            Err(anyhow!("Network access denied for {}:{}", host, port))
        }
    }

    /// Check exec (Result-based, for broker compatibility)
    pub fn check_exec_result(&self, command: &str) -> Result<()> {
        if self.check_exec(command) {
            Ok(())
        } else {
            Err(anyhow!("Execution denied for command: {}", command))
        }
    }

    /// Check env (Result-based, for broker compatibility)
    pub fn check_env_result(&self, var_name: &str) -> Result<()> {
        if self.check_env(var_name) {
            Ok(())
        } else {
            Err(anyhow!("Environment variable access denied: {}", var_name))
        }
    }

    /// Check service (Result-based, for broker compatibility)
    pub fn check_service_result(
        &self,
        service: &str,
        method: &str,
        _params: &serde_json::Value,
    ) -> Result<()> {
        if self.check_service(service, method) {
            Ok(())
        } else {
            Err(anyhow!(
                "Service method '{}' not allowed for service '{}'",
                method,
                service
            ))
        }
    }

    /// Check reminders scope (Result-based, for broker compatibility)
    pub fn check_reminders_scope_result(&self, list_name: &str) -> Result<()> {
        if self.check_reminders_scope(list_name) {
            Ok(())
        } else {
            Err(anyhow!(
                "Reminders list '{}' not in allowed scope",
                list_name
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::Permissions;
    use std::collections::HashMap;

    fn create_test_permissions() -> Permissions {
        let mut services = HashMap::new();

        let mut reminders_scope = HashMap::new();
        reminders_scope.insert("lists".to_string(), serde_json::json!(["Shopping"]));

        services.insert(
            "reminders".to_string(),
            crate::permissions::ServicePermission {
                access: "readwrite".to_string(),
                scope: reminders_scope,
            },
        );

        services.insert(
            "calendar".to_string(),
            crate::permissions::ServicePermission {
                access: "read".to_string(),
                scope: HashMap::new(),
            },
        );

        let mut perms = Permissions::builder()
            .fs_read(["$SKILL_DIR/**", "$DATA_DIR/config.json"])
            .fs_write(["$WORK_DIR/**", "$DATA_DIR/**"])
            .network_allow(["api.example.com:443", "*.cdn.example.com:443"])
            .exec_allow(["curl", "jq"])
            .env_allow(["LANG", "TZ"])
            .build();

        perms.services = services;
        perms
    }

    #[test]
    fn test_path_read_allowed() {
        let policy = PolicyEngine::new(create_test_permissions());
        assert!(policy.check_path_read("test.txt"));
        assert!(policy.check_path_read("data/config.json"));
    }

    #[test]
    fn test_path_read_denied() {
        let policy = PolicyEngine::new(create_test_permissions());
        assert!(!policy.check_path_read("/etc/passwd"));
    }

    #[test]
    fn test_path_write_allowed() {
        let policy = PolicyEngine::new(create_test_permissions());
        assert!(policy.check_path_write("work/output.txt"));
        assert!(policy.check_path_write("data/state.json"));
    }

    #[test]
    fn test_path_write_denied() {
        let policy = PolicyEngine::new(create_test_permissions());
        assert!(!policy.check_path_write("/etc/passwd"));
    }

    #[test]
    fn test_network_allowed() {
        let policy = PolicyEngine::new(create_test_permissions());
        assert!(policy.check_network("api.example.com", 443));
        assert!(policy.check_network("sub.cdn.example.com", 443));
    }

    #[test]
    fn test_network_denied() {
        let policy = PolicyEngine::new(create_test_permissions());
        assert!(!policy.check_network("evil.com", 443));
        assert!(!policy.check_network("api.example.com", 80));
    }

    #[test]
    fn test_service_allowed() {
        let policy = PolicyEngine::new(create_test_permissions());

        assert!(policy.check_service("reminders", "add"));
        assert!(policy.check_service("reminders", "list"));
        assert!(policy.check_service("calendar", "list"));
    }

    #[test]
    fn test_service_denied() {
        let policy = PolicyEngine::new(create_test_permissions());

        // Calendar is read-only
        assert!(!policy.check_service("calendar", "create"));

        // Browser not permitted
        assert!(!policy.check_service("browser", "open"));
    }

    #[test]
    fn test_exec_allowed() {
        let policy = PolicyEngine::new(create_test_permissions());
        assert!(policy.check_exec("curl"));
        assert!(policy.check_exec("/usr/bin/jq"));
    }

    #[test]
    fn test_exec_denied() {
        let policy = PolicyEngine::new(create_test_permissions());
        assert!(!policy.check_exec("rm"));
    }

    #[test]
    fn test_env_allowed() {
        let policy = PolicyEngine::new(create_test_permissions());
        assert!(policy.check_env("LANG"));
        assert!(policy.check_env("TZ"));
    }

    #[test]
    fn test_env_denied() {
        let policy = PolicyEngine::new(create_test_permissions());
        assert!(!policy.check_env("SECRET_KEY"));
    }

    #[test]
    fn test_reminders_scope_allowed() {
        let policy = PolicyEngine::new(create_test_permissions());
        assert!(policy.check_reminders_scope("Shopping"));
    }

    #[test]
    fn test_reminders_scope_denied() {
        let policy = PolicyEngine::new(create_test_permissions());
        assert!(!policy.check_reminders_scope("Work"));
    }
}
