use std::sync::Arc;

use anyhow::Result;
use s3_explorer::app::S3Explorer;
use s3_explorer::storage::{S3Backend, StoragePath};

const APP_TITLE: &str = "S3 Compatible Bucket Browser";

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("s3_explorer=debug".parse()?),
        )
        .init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let rt_handle = rt.handle().clone();

    let app = resolve_startup(rt_handle);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_inner_size([1100.0, 700.0]),
        ..Default::default()
    };

    // Keep the runtime alive for the duration of the process.
    let _rt = rt;

    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            s3_explorer::ui::font::setup_fonts(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}

/// Determine the startup mode:
/// 1. All required env vars set → open browser directly.
/// 2. CLI arg is a local path → open browser with LocalBackend.
/// 3. Otherwise → show the credentials config form.
fn resolve_startup(rt: tokio::runtime::Handle) -> S3Explorer {
    use s3_explorer::storage::LocalBackend;

    // Priority 1: full S3 config from env vars.
    if let Ok(backend) = S3Backend::from_env() {
        let start = StoragePath::s3_root(backend.bucket_name());
        return S3Explorer::new(Arc::new(backend), start, rt);
    }

    // Priority 2: explicit local path as CLI argument.
    if let Some(arg) = std::env::args().nth(1) {
        let path = StoragePath::parse(&arg);
        if let StoragePath::Local(ref pb) = path
            && pb.exists()
        {
            return S3Explorer::new(Arc::new(LocalBackend), path, rt);
        }
    }

    // Priority 3: show config form (pre-filled from env/saved creds).
    S3Explorer::needs_config(rt)
}
