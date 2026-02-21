use corral_core::SandboxBuilder;
use tempfile::TempDir;

fn workspace_glob(dir: &std::path::Path) -> String {
    format!("{}/**", dir.display())
}

#[cfg(target_os = "macos")]
fn libsandbox_exists() -> bool {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("libsandbox/libsandbox.dylib")
        .exists()
}

#[cfg(target_os = "linux")]
fn libsandbox_exists() -> bool {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("libsandbox/libsandbox.so")
        .exists()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn libsandbox_exists() -> bool {
    false
}

#[tokio::test]
async fn allows_read_write_inside_workspace() {
    if !libsandbox_exists() {
        return;
    }

    let tmp = TempDir::new().unwrap();
    let ws_pattern = workspace_glob(tmp.path());
    std::fs::write(tmp.path().join("in.txt"), "ok").unwrap();

    let sandbox = SandboxBuilder::new()
        .work_dir(tmp.path())
        .fs_read([ws_pattern.clone()])
        .fs_write([ws_pattern])
        .exec_allow(["sh", "cat", "echo"])
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    let read = sandbox.execute("cat in.txt").await.unwrap();
    assert_eq!(read.exit_code, 0);
    assert!(read.stdout.contains("ok"));

    let write = sandbox.execute("echo hi > out.txt && cat out.txt").await.unwrap();
    assert_eq!(write.exit_code, 0);
    assert!(write.stdout.contains("hi"));
}

#[tokio::test]
async fn denies_read_outside_workspace() {
    if !libsandbox_exists() {
        return;
    }

    let tmp = TempDir::new().unwrap();
    let ws_pattern = workspace_glob(tmp.path());

    let sandbox = SandboxBuilder::new()
        .work_dir(tmp.path())
        .fs_read([ws_pattern.clone()])
        .fs_write([ws_pattern])
        .exec_allow(["sh", "cat"])
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    let read = sandbox.execute("cat /etc/hosts").await;
    assert!(read.is_err() || read.unwrap().exit_code != 0);
}

#[tokio::test]
async fn denies_network_by_default() {
    if !libsandbox_exists() {
        return;
    }

    let tmp = TempDir::new().unwrap();
    let ws_pattern = workspace_glob(tmp.path());

    let sandbox = SandboxBuilder::new()
        .work_dir(tmp.path())
        .fs_read([ws_pattern.clone()])
        .fs_write([ws_pattern])
        .exec_allow(["sh", "curl"])
        .network_deny()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    let net = sandbox
        .execute("curl -sS --max-time 3 https://example.com >/dev/null")
        .await;
    assert!(net.is_err() || net.unwrap().exit_code != 0);
}

#[tokio::test]
async fn denies_absolute_path_outside_workspace() {
    if !libsandbox_exists() {
        return;
    }

    let tmp = TempDir::new().unwrap();
    let ws_pattern = workspace_glob(tmp.path());

    let sandbox = SandboxBuilder::new()
        .work_dir(tmp.path())
        .fs_read([ws_pattern.clone()])
        .fs_write([ws_pattern])
        .exec_allow(["sh"])
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    // This should fail because /etc/passwd is not in the allowed paths
    let exec = sandbox.execute("cat /etc/passwd").await;
    assert!(exec.is_err() || exec.unwrap().exit_code != 0);
}
