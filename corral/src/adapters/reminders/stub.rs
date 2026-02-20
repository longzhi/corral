//! Stub reminders adapter for unsupported platforms

use super::{AddParams, IdParams, ListParams, Reminder, RemindersAdapter, UpdateParams};
use anyhow::{anyhow, Result};
use async_trait::async_trait;

/// Stub implementation for unsupported platforms
pub struct StubRemindersAdapter;

#[async_trait]
impl RemindersAdapter for StubRemindersAdapter {
    async fn list(&self, _params: ListParams) -> Result<Vec<Reminder>> {
        Err(anyhow!(
            "Reminders service not available on this platform"
        ))
    }

    async fn add(&self, _params: AddParams) -> Result<Reminder> {
        Err(anyhow!(
            "Reminders service not available on this platform"
        ))
    }

    async fn update(&self, _params: UpdateParams) -> Result<Reminder> {
        Err(anyhow!(
            "Reminders service not available on this platform"
        ))
    }

    async fn complete(&self, _params: IdParams) -> Result<Reminder> {
        Err(anyhow!(
            "Reminders service not available on this platform"
        ))
    }

    async fn delete(&self, _params: IdParams) -> Result<()> {
        Err(anyhow!(
            "Reminders service not available on this platform"
        ))
    }

    fn is_available(&self) -> bool {
        false
    }
}
