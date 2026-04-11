/// End-to-end tests against a real S3-compatible bucket.
///
/// Credentials are loaded from the `.env` file via `dotenvy` (or from
/// environment variables already set in the shell / CI runner).
/// Tests are skipped silently when credentials are unavailable so that
/// a plain `cargo test` without a `.env` file does not fail.
use std::time::Duration;

use bytes::Bytes;
use s3_explorer::storage::{Backend, EntryKind, S3Backend, StoragePath};

/// Load `.env` (if present) then return a connected [`S3Backend`],
/// or return early (skip) if credentials are still absent.
macro_rules! backend_or_skip {
    () => {{
        dotenvy::dotenv().ok();
        match S3Backend::from_env() {
            Ok(b) => b,
            Err(_) => {
                eprintln!("S3 credentials not set — skipping e2e test");
                return;
            }
        }
    }};
}

/// A unique key prefix so our test objects don't collide with real data and
/// are easy to spot / clean up if something goes wrong.
fn test_prefix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    format!("_s3explorer_test_{ts}/")
}

fn bucket() -> String {
    dotenvy::dotenv().ok();
    std::env::var("S3_BUCKET").unwrap_or_default()
}

// ── Happy-path tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_from_env_builds_successfully() {
    backend_or_skip!();
    // Reaching here means from_env() returned Ok — the client was constructed.
}

#[tokio::test]
async fn e2e_list_bucket_root() {
    let backend = backend_or_skip!();
    let root = StoragePath::s3(&bucket(), "");

    let entries = backend.list(&root).await.expect("list should succeed");
    // We can't know what's in the bucket, but the call must succeed and return
    // a well-formed vec (may be empty for a brand-new bucket).
    for entry in &entries {
        assert!(!entry.name.is_empty(), "every entry must have a name");
    }
}

#[tokio::test]
async fn e2e_put_get_roundtrip() {
    let backend = backend_or_skip!();
    let prefix = test_prefix();
    let key = format!("{prefix}roundtrip.txt");
    let path = StoragePath::s3(&bucket(), &key);
    let payload = Bytes::from("s3-explorer e2e test payload");

    backend.put(&path, payload.clone()).await.expect("put should succeed");
    let got = backend.get(&path).await.expect("get should succeed");
    assert_eq!(got, payload);

    // Cleanup
    backend.delete(&path).await.ok();
}

#[tokio::test]
async fn e2e_uploaded_file_appears_in_listing() {
    let backend = backend_or_skip!();
    let prefix = test_prefix();
    let key = format!("{prefix}listed.txt");
    let path = StoragePath::s3(&bucket(), &key);

    backend.put(&path, Bytes::from("list me")).await.expect("put should succeed");

    let dir = StoragePath::s3(&bucket(), &prefix);
    let entries = backend.list(&dir).await.expect("list should succeed");

    let found = entries.iter().any(|e| e.name == "listed.txt" && e.kind == EntryKind::File);
    assert!(found, "uploaded file must appear in listing; got: {entries:?}");

    // Cleanup
    backend.delete(&path).await.ok();
}

#[tokio::test]
async fn e2e_delete_removes_file() {
    let backend = backend_or_skip!();
    let prefix = test_prefix();
    let key = format!("{prefix}delete_me.txt");
    let path = StoragePath::s3(&bucket(), &key);

    backend.put(&path, Bytes::from("delete me")).await.expect("put should succeed");
    backend.delete(&path).await.expect("delete should succeed");

    // After deletion the file must not appear in listing.
    let dir = StoragePath::s3(&bucket(), &prefix);
    let entries = backend.list(&dir).await.expect("list should succeed");
    assert!(
        entries.iter().all(|e| e.name != "delete_me.txt"),
        "deleted file must not appear in listing"
    );
}

#[tokio::test]
async fn e2e_presign_url_is_valid_https_url() {
    let backend = backend_or_skip!();
    let prefix = test_prefix();
    let key = format!("{prefix}presign.txt");
    let path = StoragePath::s3(&bucket(), &key);

    backend.put(&path, Bytes::from("presign me")).await.expect("put should succeed");

    let url = backend
        .presign_url(&path, Duration::from_secs(300))
        .await
        .expect("presign should succeed");

    assert!(url.starts_with("https://"), "presigned URL must be HTTPS; got: {url}");

    // Cleanup
    backend.delete(&path).await.ok();
}

#[tokio::test]
async fn e2e_rename_file() {
    let backend = backend_or_skip!();
    let prefix = test_prefix();
    let src_key = format!("{prefix}before.txt");
    let dst_key = format!("{prefix}after.txt");
    let src = StoragePath::s3(&bucket(), &src_key);
    let dst = StoragePath::s3(&bucket(), &dst_key);

    backend.put(&src, Bytes::from("rename me")).await.expect("put should succeed");
    backend.rename(&src, &dst).await.expect("rename should succeed");

    let got = backend.get(&dst).await.expect("get renamed file should succeed");
    assert_eq!(got, Bytes::from("rename me"));

    // Original must be gone.
    assert!(backend.get(&src).await.is_err(), "original key must not exist after rename");

    // Cleanup
    backend.delete(&dst).await.ok();
}
