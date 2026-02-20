//! Integration tests for corral

#[test]
fn test_manifest_loading() {
    // Create a temporary skill directory
    let temp_dir = std::env::temp_dir().join("corral-test-skill");
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Write a test manifest
    let manifest_content = r#"
name: test-skill
version: 1.0.0
description: Test skill for integration testing
author: test-author
entry: ./run.sh
runtime: bash

permissions:
  fs:
    read:
      - $SKILL_DIR/**
    write:
      - $WORK_DIR/**
  
  network:
    allow:
      - api.example.com:443
"#;

    std::fs::write(temp_dir.join("skill.yaml"), manifest_content).unwrap();

    // Try to load it
    let manifest = corral::manifest::Manifest::load(&temp_dir);
    assert!(manifest.is_ok());

    let manifest = manifest.unwrap();
    assert_eq!(manifest.name, "test-skill");
    assert_eq!(manifest.version, "1.0.0");
    assert_eq!(manifest.runtime, "bash");

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_policy_engine() {
    use corral_core::PolicyEngine;

    let temp_dir = std::env::temp_dir().join("corral-test-policy");
    std::fs::create_dir_all(&temp_dir).unwrap();

    let manifest_content = r#"
name: policy-test
version: 1.0.0
description: Policy test
author: test
entry: ./run.sh
runtime: bash

permissions:
  fs:
    read:
      - "*.txt"
      - config/**
  network:
    allow:
      - api.example.com:443
  exec:
    - curl
  env:
    - PATH
"#;

    std::fs::write(temp_dir.join("skill.yaml"), manifest_content).unwrap();

    let manifest = corral::manifest::Manifest::load(&temp_dir).unwrap();
    let policy = PolicyEngine::new(manifest.to_permissions());

    // Test basic checks
    // Network check
    assert!(policy.check_network("api.example.com", 443));
    assert!(!policy.check_network("evil.com", 443));

    // Exec check
    assert!(policy.check_exec("curl"));
    assert!(policy.check_exec("/usr/bin/curl")); // Path extraction works
    assert!(!policy.check_exec("rm"));

    // Env check
    assert!(policy.check_env("PATH"));
    assert!(!policy.check_env("SECRET"));

    // File access with glob patterns
    assert!(policy.check_path_read("test.txt"));
    assert!(policy.check_path_read("readme.txt"));
    assert!(policy.check_path_read("config/app.json"));
    assert!(!policy.check_path_read("data/sensitive.db"));

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}
