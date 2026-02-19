//! Skill manifest parser and validator

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Skill manifest from skill.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub entry: String,
    pub runtime: String,

    #[serde(default)]
    pub permissions: Permissions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Permissions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fs: Option<FsPermissions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkPermissions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<ServicePermissions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsPermissions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub write: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPermissions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServicePermissions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reminders: Option<ServiceAccess>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar: Option<ServiceAccess>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser: Option<ServiceAccess>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<ServiceAccess>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipboard: Option<ServiceAccess>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAccess {
    pub access: AccessLevel,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessLevel {
    Read,
    Write,
    ReadWrite,
    Send,
    Open,
}

impl Manifest {
    /// Load manifest from skill directory
    pub fn load(skill_path: &Path) -> Result<Self> {
        let manifest_path = skill_path.join("skill.yaml");

        let content = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read manifest at {:?}", manifest_path))?;

        let manifest: Manifest =
            serde_yaml::from_str(&content).with_context(|| "Failed to parse skill.yaml")?;

        manifest.validate()?;

        Ok(manifest)
    }

    /// Validate manifest fields
    fn validate(&self) -> Result<()> {
        anyhow::ensure!(!self.name.is_empty(), "Skill name cannot be empty");
        anyhow::ensure!(!self.version.is_empty(), "Skill version cannot be empty");
        anyhow::ensure!(!self.entry.is_empty(), "Entry point cannot be empty");
        anyhow::ensure!(!self.runtime.is_empty(), "Runtime cannot be empty");

        // Validate runtime
        let valid_runtimes = ["bash", "sh", "python", "python3", "node"];
        anyhow::ensure!(
            valid_runtimes.contains(&self.runtime.as_str()),
            "Invalid runtime '{}'. Must be one of: {}",
            self.runtime,
            valid_runtimes.join(", ")
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_manifest() {
        let yaml = r#"
name: test-skill
version: 1.0.0
description: Test skill
author: test
entry: ./run.sh
runtime: bash
"#;

        let manifest: Manifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.name, "test-skill");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.runtime, "bash");
    }

    #[test]
    fn test_parse_full_manifest() {
        let yaml = r#"
name: smart-shopping
version: 1.0.0
description: Manage shopping lists
author: community/alice
entry: ./run.sh
runtime: bash

permissions:
  fs:
    read:
      - $SKILL_DIR/**
      - $DATA_DIR/config.json
    write:
      - $WORK_DIR/**
      - $DATA_DIR/**
  
  network:
    allow:
      - api.example.com:443
      - cdn.example.com:443
  
  services:
    reminders:
      access: readwrite
      scope:
        lists: ["Shopping"]
    calendar:
      access: read
      scope:
        calendars: ["*"]
    browser:
      access: open
      scope:
        domains: ["example.com"]
    notifications:
      access: send
  
  exec:
    - curl
    - jq
  
  env:
    - LANG
    - TZ
"#;

        let manifest: Manifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.name, "smart-shopping");

        let fs = manifest.permissions.fs.as_ref().unwrap();
        assert_eq!(fs.read.as_ref().unwrap().len(), 2);
        assert_eq!(fs.write.as_ref().unwrap().len(), 2);

        let network = manifest.permissions.network.as_ref().unwrap();
        assert_eq!(network.allow.as_ref().unwrap().len(), 2);

        let services = manifest.permissions.services.as_ref().unwrap();
        assert!(services.reminders.is_some());
        assert!(services.calendar.is_some());

        assert_eq!(manifest.permissions.exec.as_ref().unwrap().len(), 2);
        assert_eq!(manifest.permissions.env.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_invalid_runtime() {
        let yaml = r#"
name: test-skill
version: 1.0.0
description: Test
author: test
entry: ./run.sh
runtime: invalid
"#;

        let manifest: Manifest = serde_yaml::from_str(yaml).unwrap();
        assert!(manifest.validate().is_err());
    }
}
