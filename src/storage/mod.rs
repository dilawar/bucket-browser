mod backend;
mod local;
mod path;
#[cfg(not(target_arch = "wasm32"))]
pub mod s3;

pub use backend::Backend;
pub use local::LocalBackend;
pub use path::{human_size, EntryKind, StorageEntry, StoragePath};
#[cfg(not(target_arch = "wasm32"))]
pub use s3::{
    S3Backend, ENV_ACCESS_KEY, ENV_BUCKET, ENV_ENDPOINT, ENV_REGION, ENV_SECRET_KEY,
};
