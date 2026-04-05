use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::storage::{Backend, StorageEntry, StoragePath};

// ── ListingHandle ─────────────────────────────────────────────────────────────

/// Holds the pending or completed result of a single listing request.
pub struct ListingHandle {
    slot: Arc<Mutex<Option<Result<Vec<StorageEntry>>>>>,
    join: tokio::task::JoinHandle<()>,
}

impl ListingHandle {
    /// Non-blocking drain. Returns `None` while running, `Some(result)` when done.
    pub fn try_recv(&self) -> Option<Result<Vec<StorageEntry>>> {
        if let Some(result) = self.slot.lock().unwrap().take() {
            return Some(result);
        }
        if self.join.is_finished() {
            return Some(Err(anyhow::anyhow!(
                "listing task panicked — check S3 credentials and endpoint"
            )));
        }
        None
    }
}

/// Spawn a background listing task.
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

// ── TransferHandle ────────────────────────────────────────────────────────────

/// Tracks an in-progress upload or download. Result is a human-readable status string.
pub struct TransferHandle {
    slot: Arc<Mutex<Option<Result<String>>>>,
    join: tokio::task::JoinHandle<()>,
}

impl TransferHandle {
    /// Returns `None` while running, `Some(result)` when done.
    pub fn try_recv(&self) -> Option<Result<String>> {
        if let Some(result) = self.slot.lock().unwrap().take() {
            return Some(result);
        }
        if self.join.is_finished() {
            return Some(Err(anyhow::anyhow!("transfer task panicked")));
        }
        None
    }

    pub fn is_running(&self) -> bool {
        !self.join.is_finished() && self.slot.lock().unwrap().is_none()
    }
}

/// Spawn a download: fetches `path` from the backend, shows a native save dialog,
/// then writes the bytes to the chosen location.
pub fn spawn_download(
    backend: Arc<dyn Backend>,
    path: StoragePath,
    ctx: egui::Context,
    rt: &tokio::runtime::Handle,
) -> TransferHandle {
    let slot: Arc<Mutex<Option<Result<String>>>> = Arc::new(Mutex::new(None));
    let slot2 = Arc::clone(&slot);

    let join = rt.spawn(async move {
        let result = do_download(backend, path).await;
        *slot2.lock().unwrap() = Some(result);
        ctx.request_repaint();
    });

    TransferHandle { slot, join }
}

async fn do_download(backend: Arc<dyn Backend>, path: StoragePath) -> Result<String> {
    let file_name = path
        .to_string()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("download")
        .to_owned();

    // Open save dialog on the blocking thread pool (rfd is sync).
    let save_path = tokio::task::spawn_blocking(move || {
        rfd::FileDialog::new().set_file_name(&file_name).save_file()
    })
    .await?;

    let Some(save_path) = save_path else {
        return Ok("Download cancelled.".to_owned());
    };

    let data = backend.get(&path).await?;
    tokio::fs::write(&save_path, &data).await?;
    Ok(format!("Saved to {}", save_path.display()))
}

/// Spawn an upload: shows a native file picker, reads the chosen file, and
/// puts it under `current_path` in the backend.
pub fn spawn_upload(
    backend: Arc<dyn Backend>,
    current_path: StoragePath,
    ctx: egui::Context,
    rt: &tokio::runtime::Handle,
) -> TransferHandle {
    let slot: Arc<Mutex<Option<Result<String>>>> = Arc::new(Mutex::new(None));
    let slot2 = Arc::clone(&slot);

    let join = rt.spawn(async move {
        let result = do_upload(backend, current_path).await;
        *slot2.lock().unwrap() = Some(result);
        ctx.request_repaint();
    });

    TransferHandle { slot, join }
}

async fn do_upload(backend: Arc<dyn Backend>, current_path: StoragePath) -> Result<String> {
    // Open file picker on the blocking thread pool.
    let local_path = tokio::task::spawn_blocking(|| rfd::FileDialog::new().pick_file()).await?;

    let Some(local_path) = local_path else {
        return Ok("Upload cancelled.".to_owned());
    };

    let file_name = local_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "upload".to_owned());

    let dest = current_path.child_file(&file_name);
    let data = bytes::Bytes::from(tokio::fs::read(&local_path).await?);
    let size = data.len();
    backend.put(&dest, data).await?;
    Ok(format!("Uploaded {file_name} ({size} bytes) → {dest}"))
}
