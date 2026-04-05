use anyhow::Result;
use async_trait::async_trait;

use super::path::{StorageEntry, StoragePath};

#[async_trait]
pub trait Backend: Send + Sync + 'static {
    /// List the immediate children of `path`.
    async fn list(&self, path: &StoragePath) -> Result<Vec<StorageEntry>>;

    /// Short human-readable name shown in the status bar (e.g. `"Local"`, `"S3: my-bucket"`).
    fn name(&self) -> &str;
}
