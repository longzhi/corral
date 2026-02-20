//! macOS Reminders adapter
//!
//! Uses the Swift helper binary to interact with EventKit.

use super::{AddParams, IdParams, ListParams, Reminder, RemindersAdapter, UpdateParams};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Request to the Swift helper
#[derive(Debug, Serialize)]
struct HelperRequest {
    action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    list: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "dueDate")]
    due_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<i32>,
}

/// Response from the Swift helper
#[derive(Debug, Deserialize)]
struct HelperResponse {
    reminders: Option<Vec<Reminder>>,
    error: Option<String>,
    #[allow(dead_code)]
    success: Option<bool>,
}

/// macOS implementation of RemindersAdapter
pub struct MacOSRemindersAdapter {
    helper_path: PathBuf,
}

impl MacOSRemindersAdapter {
    /// Create a new macOS reminders adapter
    pub fn new() -> Self {
        Self {
            helper_path: Self::find_helper_binary(),
        }
    }

    /// Find the reminders-helper binary
    fn find_helper_binary() -> PathBuf {
        // Strategy:
        // 1. Check REMINDERS_HELPER_PATH env var
        // 2. Check in the same directory as the current executable
        // 3. Check in ../helpers/reminders-helper-macos/
        // 4. Check in PATH

        if let Ok(path) = env::var("REMINDERS_HELPER_PATH") {
            let p = PathBuf::from(path);
            if p.exists() {
                return p;
            }
        }

        // Check next to our binary
        if let Ok(exe) = env::current_exe() {
            if let Some(parent) = exe.parent() {
                let helper = parent.join("reminders-helper");
                if helper.exists() {
                    return helper;
                }

                // Check in helpers/ subdirectory
                let helper = parent.join("helpers").join("reminders-helper");
                if helper.exists() {
                    return helper;
                }

                // Check in ../helpers/reminders-helper-macos/
                let helper = parent
                    .parent()
                    .map(|p| p.join("helpers/reminders-helper-macos/reminders-helper"));
                if let Some(h) = helper {
                    if h.exists() {
                        return h;
                    }
                }
            }
        }

        // Fallback: assume it's in PATH
        PathBuf::from("reminders-helper")
    }

    /// Call the Swift helper with a request
    async fn call_helper(&self, request: HelperRequest) -> Result<HelperResponse> {
        // Serialize request
        let request_json =
            serde_json::to_string(&request).context("Failed to serialize request")?;

        // Spawn helper process
        let mut child = Command::new(&self.helper_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!(
                "Failed to spawn reminders-helper at {:?}. \
                Make sure it's built and available. \
                Run: cd helpers/reminders-helper-macos && make",
                self.helper_path
            ))?;

        // Write request to stdin
        {
            let mut stdin = child.stdin.take().context("Failed to open stdin")?;
            stdin
                .write_all(request_json.as_bytes())
                .await
                .context("Failed to write to stdin")?;
            stdin
                .write_all(b"\n")
                .await
                .context("Failed to write newline")?;
            stdin.flush().await.context("Failed to flush stdin")?;
        }

        // Read response from stdout
        let stdout = child.stdout.take().context("Failed to open stdout")?;
        let mut reader = BufReader::new(stdout);
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .await
            .context("Failed to read response")?;

        // Wait for process to complete
        let status = child.wait().await.context("Failed to wait for helper")?;

        if !status.success() {
            // Try to read stderr for more context
            let stderr = child.stderr;
            let error_msg = if let Some(stderr) = stderr {
                let mut err_reader = BufReader::new(stderr);
                let mut err_line = String::new();
                let _ = err_reader.read_line(&mut err_line).await;
                err_line.trim().to_string()
            } else {
                "Unknown error".to_string()
            };

            return Err(anyhow!(
                "Helper process exited with status {}: {}",
                status.code().unwrap_or(-1),
                error_msg
            ));
        }

        // Parse response
        let response: HelperResponse =
            serde_json::from_str(&response_line).context("Failed to parse response from helper")?;

        // Check for errors in response
        if let Some(error) = response.error {
            return Err(anyhow!("Helper returned error: {}", error));
        }

        Ok(response)
    }
}

#[async_trait]
impl RemindersAdapter for MacOSRemindersAdapter {
    async fn list(&self, params: ListParams) -> Result<Vec<Reminder>> {
        let request = HelperRequest {
            action: "list".to_string(),
            list: params.list,
            completed: params.completed,
            id: None,
            title: None,
            due_date: None,
            notes: None,
            priority: None,
        };

        let response = self.call_helper(request).await?;
        Ok(response.reminders.unwrap_or_default())
    }

    async fn add(&self, params: AddParams) -> Result<Reminder> {
        let request = HelperRequest {
            action: "add".to_string(),
            list: Some(params.list),
            completed: None,
            id: None,
            title: Some(params.title),
            due_date: params.due_date,
            notes: params.notes,
            priority: params.priority,
        };

        let response = self.call_helper(request).await?;
        response
            .reminders
            .and_then(|mut v| v.pop())
            .ok_or_else(|| anyhow!("No reminder returned from helper"))
    }

    async fn update(&self, params: UpdateParams) -> Result<Reminder> {
        let request = HelperRequest {
            action: "update".to_string(),
            list: None,
            completed: None,
            id: Some(params.id),
            title: params.title,
            due_date: params.due_date,
            notes: params.notes,
            priority: params.priority,
        };

        let response = self.call_helper(request).await?;
        response
            .reminders
            .and_then(|mut v| v.pop())
            .ok_or_else(|| anyhow!("No reminder returned from helper"))
    }

    async fn complete(&self, params: IdParams) -> Result<Reminder> {
        let request = HelperRequest {
            action: "complete".to_string(),
            list: None,
            completed: None,
            id: Some(params.id),
            title: None,
            due_date: None,
            notes: None,
            priority: None,
        };

        let response = self.call_helper(request).await?;
        response
            .reminders
            .and_then(|mut v| v.pop())
            .ok_or_else(|| anyhow!("No reminder returned from helper"))
    }

    async fn delete(&self, params: IdParams) -> Result<()> {
        let request = HelperRequest {
            action: "delete".to_string(),
            list: None,
            completed: None,
            id: Some(params.id),
            title: None,
            due_date: None,
            notes: None,
            priority: None,
        };

        self.call_helper(request).await?;
        Ok(())
    }

    fn is_available(&self) -> bool {
        // Check if the helper binary exists
        self.helper_path.exists()
    }
}
