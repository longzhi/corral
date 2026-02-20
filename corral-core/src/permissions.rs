//! Standalone permissions definition and builder API

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Standalone permissions definition - no Manifest dependency
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Permissions {
    #[serde(default)]
    pub fs: FsPermissions,
    #[serde(default)]
    pub network: NetworkPermissions,
    #[serde(default)]
    pub exec: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub services: HashMap<String, ServicePermission>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FsPermissions {
    pub read: Vec<String>,  // glob patterns
    pub write: Vec<String>, // glob patterns
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct NetworkPermissions {
    pub allow: Vec<String>, // host:port patterns
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServicePermission {
    pub access: String, // "read", "write", "readwrite", "send", "open"
    pub scope: HashMap<String, serde_json::Value>,
}

impl Permissions {
    /// Create a new builder
    pub fn builder() -> PermissionsBuilder {
        PermissionsBuilder::new()
    }

    /// Intersect two permission sets (returns the most restrictive combination)
    pub fn intersect(&self, other: &Permissions) -> Permissions {
        Permissions {
            fs: FsPermissions {
                read: self
                    .fs
                    .read
                    .iter()
                    .filter(|p| other.fs.read.contains(p))
                    .cloned()
                    .collect(),
                write: self
                    .fs
                    .write
                    .iter()
                    .filter(|p| other.fs.write.contains(p))
                    .cloned()
                    .collect(),
            },
            network: NetworkPermissions {
                allow: self
                    .network
                    .allow
                    .iter()
                    .filter(|h| other.network.allow.contains(h))
                    .cloned()
                    .collect(),
            },
            exec: self
                .exec
                .iter()
                .filter(|e| other.exec.contains(e))
                .cloned()
                .collect(),
            env: self
                .env
                .iter()
                .filter(|e| other.env.contains(e))
                .cloned()
                .collect(),
            services: self
                .services
                .iter()
                .filter_map(|(k, v)| {
                    if other.services.contains_key(k) {
                        Some((k.clone(), v.clone()))
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }
}

/// Builder API for Permissions
#[derive(Debug, Default)]
pub struct PermissionsBuilder {
    fs_read: Vec<String>,
    fs_write: Vec<String>,
    network_allow: Vec<String>,
    network_deny: bool,
    exec: Vec<String>,
    env: Vec<String>,
    services: HashMap<String, ServicePermission>,
}

impl PermissionsBuilder {
    /// Create a new permissions builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Add filesystem read patterns
    pub fn fs_read<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.fs_read.extend(patterns.into_iter().map(Into::into));
        self
    }

    /// Add filesystem write patterns
    pub fn fs_write<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.fs_write.extend(patterns.into_iter().map(Into::into));
        self
    }

    /// Add allowed network hosts
    pub fn network_allow<I, S>(mut self, hosts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.network_allow.extend(hosts.into_iter().map(Into::into));
        self
    }

    /// Deny all network access
    pub fn network_deny(mut self) -> Self {
        self.network_deny = true;
        self.network_allow.clear();
        self
    }

    /// Add allowed executables
    pub fn exec_allow<I, S>(mut self, cmds: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.exec.extend(cmds.into_iter().map(Into::into));
        self
    }

    /// Add allowed environment variables
    pub fn env_allow<I, S>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.env.extend(vars.into_iter().map(Into::into));
        self
    }

    /// Add a service permission
    pub fn service(mut self, name: &str, perm: ServicePermission) -> Self {
        self.services.insert(name.to_string(), perm);
        self
    }

    /// Build the Permissions object
    pub fn build(self) -> Permissions {
        Permissions {
            fs: FsPermissions {
                read: self.fs_read,
                write: self.fs_write,
            },
            network: NetworkPermissions {
                allow: if self.network_deny {
                    Vec::new()
                } else {
                    self.network_allow
                },
            },
            exec: self.exec,
            env: self.env,
            services: self.services,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let perms = Permissions::builder()
            .fs_read(["/usr/**", "/etc/**"])
            .fs_write(["/tmp/**"])
            .network_allow(["api.example.com:443"])
            .exec_allow(["curl", "bash"])
            .env_allow(["PATH", "HOME"])
            .build();

        assert_eq!(perms.fs.read.len(), 2);
        assert_eq!(perms.fs.write.len(), 1);
        assert_eq!(perms.network.allow.len(), 1);
        assert_eq!(perms.exec.len(), 2);
        assert_eq!(perms.env.len(), 2);
    }

    #[test]
    fn test_network_deny() {
        let perms = Permissions::builder()
            .network_allow(["api.example.com:443"])
            .network_deny()
            .build();

        assert!(perms.network.allow.is_empty());
    }

    #[test]
    fn test_intersect() {
        let perms1 = Permissions::builder()
            .fs_read(["/usr/**", "/etc/**"])
            .fs_write(["/tmp/**", "/var/**"])
            .exec_allow(["curl", "bash", "python"])
            .build();

        let perms2 = Permissions::builder()
            .fs_read(["/usr/**", "/home/**"])
            .fs_write(["/tmp/**"])
            .exec_allow(["curl", "jq"])
            .build();

        let intersection = perms1.intersect(&perms2);

        assert_eq!(intersection.fs.read, vec!["/usr/**"]);
        assert_eq!(intersection.fs.write, vec!["/tmp/**"]);
        assert_eq!(intersection.exec, vec!["curl"]);
    }

    #[test]
    fn test_service_permission() {
        let mut scope = HashMap::new();
        scope.insert("lists".to_string(), serde_json::json!(["Shopping"]));

        let service_perm = ServicePermission {
            access: "readwrite".to_string(),
            scope,
        };

        let perms = Permissions::builder()
            .service("reminders", service_perm)
            .build();

        assert!(perms.services.contains_key("reminders"));
        assert_eq!(perms.services.get("reminders").unwrap().access, "readwrite");
    }
}
