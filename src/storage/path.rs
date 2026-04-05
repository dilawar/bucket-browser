use std::path::PathBuf;

use strum::{Display, EnumIs};

/// A location in any supported backend.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StoragePath {
    /// Local filesystem directory (dev / testing).
    Local(PathBuf),
    /// S3-compatible location: `s3://<bucket>/<prefix>`.
    /// `prefix` has no leading slash; directories end with `/`.
    S3 { bucket: String, prefix: String },
}

impl StoragePath {
    /// Parse an address-bar string into a `StoragePath`.
    /// Strings starting with `s3://` are S3; everything else is Local.
    pub fn parse(s: &str) -> Self {
        let s = s.trim();
        if let Some(rest) = s.strip_prefix("s3://") {
            match rest.split_once('/') {
                Some((bucket, prefix)) => Self::S3 {
                    bucket: bucket.to_owned(),
                    prefix: prefix.to_owned(),
                },
                None => Self::S3 {
                    bucket: rest.to_owned(),
                    prefix: String::new(),
                },
            }
        } else {
            Self::Local(PathBuf::from(s))
        }
    }

    /// The string shown in the toolbar address bar.
    pub fn display_string(&self) -> String {
        match self {
            Self::Local(p) => p.to_string_lossy().into_owned(),
            Self::S3 { bucket, prefix } => {
                if prefix.is_empty() {
                    format!("s3://{bucket}/")
                } else {
                    format!("s3://{bucket}/{prefix}")
                }
            }
        }
    }

    /// One level up, or `None` if already at the root.
    pub fn parent(&self) -> Option<Self> {
        match self {
            Self::Local(p) => p.parent().map(|p| Self::Local(p.to_path_buf())),
            Self::S3 { bucket, prefix } => {
                if prefix.is_empty() {
                    return None;
                }
                // Strip trailing slash, then find the previous slash.
                let trimmed = prefix.trim_end_matches('/');
                let new_prefix = match trimmed.rfind('/') {
                    Some(idx) => trimmed[..=idx].to_owned(),
                    None => String::new(),
                };
                Some(Self::S3 { bucket: bucket.clone(), prefix: new_prefix })
            }
        }
    }

    /// Descend into a child directory named `name`.
    pub fn child(&self, name: &str) -> Self {
        match self {
            Self::Local(p) => Self::Local(p.join(name)),
            Self::S3 { bucket, prefix } => Self::S3 {
                bucket: bucket.clone(),
                prefix: format!("{prefix}{name}/"),
            },
        }
    }

    /// Breadcrumb segments: `(label, path_to_that_segment)` from root to here.
    pub fn breadcrumbs(&self) -> Vec<(String, Self)> {
        match self {
            Self::Local(p) => {
                let mut out = Vec::new();
                let mut acc = PathBuf::new();
                for component in p.components() {
                    acc.push(component);
                    let label = match component {
                        std::path::Component::RootDir => "/".to_owned(),
                        other => other.as_os_str().to_string_lossy().into_owned(),
                    };
                    out.push((label, Self::Local(acc.clone())));
                }
                out
            }
            Self::S3 { bucket, prefix } => {
                let mut out = vec![(
                    format!("s3://{bucket}"),
                    Self::S3 { bucket: bucket.clone(), prefix: String::new() },
                )];
                if !prefix.is_empty() {
                    let mut acc = String::new();
                    for segment in prefix.trim_end_matches('/').split('/') {
                        acc.push_str(segment);
                        acc.push('/');
                        out.push((
                            segment.to_owned(),
                            Self::S3 { bucket: bucket.clone(), prefix: acc.clone() },
                        ));
                    }
                }
                out
            }
        }
    }
}

// ── Entry types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Display, EnumIs)]
pub enum EntryKind {
    #[strum(to_string = "directory")]
    Directory,
    #[strum(to_string = "file")]
    File,
}

#[derive(Debug, Clone)]
pub struct StorageEntry {
    /// Last path segment (no trailing slash).
    pub name: String,
    /// Full path to this entry.
    pub path: StoragePath,
    pub kind: EntryKind,
    /// Size in bytes; `None` for directories or when unavailable.
    pub size: Option<u64>,
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
}

/// Format byte count as a human-readable string, e.g. `"1.2 MB"`.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut idx = 0usize;
    while value >= 1024.0 && idx + 1 < UNITS.len() {
        value /= 1024.0;
        idx += 1;
    }
    if idx == 0 { format!("{bytes} B") } else { format!("{value:.1} {}", UNITS[idx]) }
}
