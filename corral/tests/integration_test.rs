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
      - $SKILL_DIR/**
  network:
    allow:
      - api.example.com:443
"#;
    
    std::fs::write(temp_dir.join("skill.yaml"), manifest_content).unwrap();
    
    let manifest = corral::manifest::Manifest::load(&temp_dir).unwrap();
    let policy = corral::policy::PolicyEngine::new(manifest);
    
    // Test file access
    assert!(policy.check_file_read("test.txt").is_ok());
    assert!(policy.check_file_read("/etc/passwd").is_err());
    
    // Test network access
    assert!(policy.check_network("api.example.com", 443).is_ok());
    assert!(policy.check_network("evil.com", 443).is_err());
    
    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();
}
