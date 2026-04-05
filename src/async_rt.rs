use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::storage::{Backend, StorageEntry, StoragePath};

/// Holds the pending or completed result of a single listing request.
pub struct ListingHandle {
    /// Populated by the task on success or handled error.
    slot: Arc<Mutex<Option<Result<Vec<StorageEntry>>>>>,
    /// Kept so we can detect a panic (task finished but slot still empty).
    join: tokio::task::JoinHandle<()>,
}

impl ListingHandle {
    /// Non-blocking drain. Returns:
    /// - `None`       — task still running
    /// - `Some(Ok)`   — listing succeeded
    /// - `Some(Err)`  — listing failed *or* the task panicked
    pub fn try_recv(&self) -> Option<Result<Vec<StorageEntry>>> {
        // Happy path: task wrote a result.
        if let Some(result) = self.slot.lock().unwrap().take() {
            return Some(result);
        }
        // Task finished without writing → it panicked.
        if self.join.is_finished() {
            return Some(Err(anyhow::anyhow!(
                "listing task panicked — check S3 credentials and endpoint"
            )));
        }
        None
    }
}

/// Spawn a background listing task. `ctx.request_repaint()` is called when
/// the result is ready so the frame loop wakes up immediately.
pub fn spawn_listing(
    backend: Arc<dyn Backend>,
    path: StoragePath,
    ctx: egui::Context,
    rt: &tokio::runtime::Handle,
) -> ListingHandle {
    let slot: Arc<Mutex<Option<Result<Vec<StorageEntry>>>>> = Arc::new(Mutex::new(None));
    let slot2 = Arc::clone(&slot);

    let join = rt.spawn(async move {
        let result = backend.list(&path).await;
        *slot2.lock().unwrap() = Some(result);
        ctx.request_repaint();
    });

    ListingHandle { slot, join }
}
