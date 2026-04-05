use std::path::PathBuf;

use anyhow::{bail, Result};
use async_trait::async_trait;
use tracing::{debug, warn};

use super::backend::Backend;
use super::path::{EntryKind, StorageEntry, StoragePath};

pub struct LocalBackend;

#[async_trait]
impl Backend for LocalBackend {
    async fn list(&self, path: &StoragePath) -> Result<Vec<StorageEntry>> {
        let StoragePath::Local(dir) = path else {
            bail!("LocalBackend cannot handle {path:?}");
        };
        Ok(list_dir(dir, path))
    }

    fn name(&self) -> &str {
        "Local"
    }
}

fn list_dir(dir: &PathBuf, parent: &StoragePath) -> Vec<StorageEntry> {
    debug!("Listing {:?}", dir);
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        warn!("Cannot read {:?}", dir);
        return vec![];
    };

    let mut entries: Vec<StorageEntry> = read_dir
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let meta = e.metadata().ok()?;
            let kind = if meta.is_dir() { EntryKind::Directory } else { EntryKind::File };
            let size = kind.is_file().then(|| meta.len());
            let last_modified = meta.modified().ok().map(|t| t.into());
            let path = parent.child(&name);
            // For local files the child() appends "/" for dirs; strip trailing slash from name
            Some(StorageEntry { name, path, kind, size, last_modified })
        })
        .collect();

    entries.sort_by(|a, b| match (&a.kind, &b.kind) {
        (EntryKind::Directory, EntryKind::File) => std::cmp::Ordering::Less,
        (EntryKind::File, EntryKind::Directory) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    entries
}
