//! Reminders service adapter
//!
//! Provides access to the system's reminders/tasks service across platforms.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod stub;

/// Reminder data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    pub id: String,
    pub title: String,
    pub list: String,
    pub completed: bool,
    #[serde(rename = "dueDate")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    pub priority: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(rename = "creationDate")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_date: Option<String>,
}

/// Parameters for listing reminders
#[derive(Debug, Deserialize)]
pub struct ListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<bool>,
}

/// Parameters for adding a reminder
#[derive(Debug, Deserialize)]
pub struct AddParams {
    pub list: String,
    pub title: String,
    #[serde(rename = "dueDate")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
}

/// Parameters for updating a reminder
#[derive(Debug, Deserialize)]
pub struct UpdateParams {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "dueDate")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
}

/// Parameters for completing/deleting a reminder
#[derive(Debug, Deserialize)]
pub struct IdParams {
    pub id: String,
}

/// Reminders adapter trait
#[async_trait]
pub trait RemindersAdapter: Send + Sync {
    /// List reminders
    async fn list(&self, params: ListParams) -> Result<Vec<Reminder>>;

    /// Add a reminder
    async fn add(&self, params: AddParams) -> Result<Reminder>;

    /// Update a reminder
    async fn update(&self, params: UpdateParams) -> Result<Reminder>;

    /// Mark a reminder as completed
    async fn complete(&self, params: IdParams) -> Result<Reminder>;

    /// Delete a reminder
    async fn delete(&self, params: IdParams) -> Result<()>;

    /// Check if this adapter is available
    fn is_available(&self) -> bool;
}

/// Create the platform-specific reminders adapter
pub fn create_adapter() -> Box<dyn RemindersAdapter> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOSRemindersAdapter::new())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Box::new(stub::StubRemindersAdapter)
    }
}
