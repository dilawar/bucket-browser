use std::time::Duration;

use bytes::Bytes;
use s3_explorer::storage::{
    Backend, EntryKind, LocalBackend, S3Backend, S3Config, StoragePath,
};

// ── S3Backend construction & URL helpers ─────────────────────────────────────

#[test]
fn s3_backend_builds_with_credentials() {
    S3Backend::with_credentials(S3Config {
        bucket: "my-bucket",
        endpoint: None,
        access_key: "AKIAIOSFODNN7EXAMPLE",
        secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        region: "us-east-1",
    })
    .expect("should build without error");
}

#[test]
fn s3_backend_builds_with_custom_endpoint() {
    S3Backend::with_credentials(S3Config {
        bucket: "my-bucket",
        endpoint: Some("https://s3.us-west-002.backblazeb2.com"),
        access_key: "keyid",
        secret_key: "secretkey",
        region: "us-west-002",
    })
    .expect("should build with custom endpoint");
}

#[test]
fn s3_public_url_aws_style() {
    let backend = S3Backend::with_credentials(S3Config {
        bucket: "my-bucket",
        endpoint: None,
        access_key: "key",
        secret_key: "secret",
        region: "eu-west-1",
    })
    .unwrap();
    let path = StoragePath::s3("my-bucket", "folder/file.txt");
    let url = backend.public_url(&path).expect("should return a URL");
    assert_eq!(url, "https://my-bucket.s3.eu-west-1.amazonaws.com/folder/file.txt");
}

#[test]
fn s3_public_url_custom_endpoint() {
    let backend = S3Backend::with_credentials(S3Config {
        bucket: "my-bucket",
        endpoint: Some("https://s3.us-west-002.backblazeb2.com"),
        access_key: "key",
        secret_key: "secret",
        region: "us-west-002",
    })
    .unwrap();
    let path = StoragePath::s3("my-bucket", "folder/file.txt");
    let url = backend.public_url(&path).expect("should return a URL");
    assert_eq!(url, "https://s3.us-west-002.backblazeb2.com/my-bucket/folder/file.txt");
}

#[test]
fn s3_public_url_none_for_local_path() {
    let backend = S3Backend::with_credentials(S3Config {
        bucket: "my-bucket",
        endpoint: None,
        access_key: "key",
        secret_key: "secret",
        region: "us-east-1",
    })
    .unwrap();
    let path = StoragePath::Local("/some/file".into());
    assert!(backend.public_url(&path).is_none());
}

// ── LocalBackend ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn local_list_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let path = StoragePath::Local(dir.path().to_path_buf());
    let entries = LocalBackend.list(&path).await.unwrap();
    assert!(entries.is_empty());
}

#[tokio::test]
async fn local_put_get_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = StoragePath::Local(dir.path().join("hello.txt"));

    LocalBackend.put(&file_path, Bytes::from("hello world")).await.unwrap();
    let got = LocalBackend.get(&file_path).await.unwrap();
    assert_eq!(got, Bytes::from("hello world"));
}

#[tokio::test]
async fn local_list_contains_uploaded_file() {
    let dir = tempfile::tempdir().unwrap();
    let dir_path = StoragePath::Local(dir.path().to_path_buf());
    let file_path = StoragePath::Local(dir.path().join("data.bin"));

    LocalBackend.put(&file_path, Bytes::from(vec![1u8, 2, 3])).await.unwrap();

    let entries = LocalBackend.list(&dir_path).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "data.bin");
    assert_eq!(entries[0].kind, EntryKind::File);
    assert_eq!(entries[0].size, Some(3));
}

#[tokio::test]
async fn local_delete_file() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = StoragePath::Local(dir.path().join("to_delete.txt"));

    LocalBackend.put(&file_path, Bytes::from("bye")).await.unwrap();
    LocalBackend.delete(&file_path).await.unwrap();

    let dir_path = StoragePath::Local(dir.path().to_path_buf());
    let entries = LocalBackend.list(&dir_path).await.unwrap();
    assert!(entries.is_empty());
}

#[tokio::test]
async fn local_rename_file() {
    let dir = tempfile::tempdir().unwrap();
    let src = StoragePath::Local(dir.path().join("original.txt"));
    let dst = StoragePath::Local(dir.path().join("renamed.txt"));

    LocalBackend.put(&src, Bytes::from("content")).await.unwrap();
    LocalBackend.rename(&src, &dst).await.unwrap();

    assert!(LocalBackend.get(&dst).await.is_ok());
    assert!(LocalBackend.get(&src).await.is_err());
}

#[tokio::test]
async fn local_create_dir_and_list() {
    let dir = tempfile::tempdir().unwrap();
    let subdir = StoragePath::Local(dir.path().join("subdir"));

    LocalBackend.create_dir(&subdir).await.unwrap();

    let dir_path = StoragePath::Local(dir.path().to_path_buf());
    let entries = LocalBackend.list(&dir_path).await.unwrap();
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"subdir"));
}

#[tokio::test]
async fn local_list_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let subdir = dir.path().join("sub");
    std::fs::create_dir(&subdir).unwrap();
    std::fs::write(subdir.join("a.txt"), b"a").unwrap();
    std::fs::write(dir.path().join("b.txt"), b"b").unwrap();

    let root = StoragePath::Local(dir.path().to_path_buf());
    let files = LocalBackend.list_recursive(&root).await.unwrap();
    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|e| e.kind == EntryKind::File));
}

#[tokio::test]
async fn local_presign_url_unsupported() {
    let dir = tempfile::tempdir().unwrap();
    let path = StoragePath::Local(dir.path().join("file.txt"));
    let result = LocalBackend.presign_url(&path, Duration::from_secs(60)).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn local_public_url_is_file_scheme() {
    let path = StoragePath::Local("/tmp/some/file.txt".into());
    let url = LocalBackend.public_url(&path).expect("should return file:// URL");
    assert!(url.starts_with("file://"));
}
