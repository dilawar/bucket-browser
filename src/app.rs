use std::sync::Arc;

use egui::{CentralPanel, Color32, RichText, SidePanel, TopBottomPanel};
use tracing::info;

use crate::async_rt::{self, ListingHandle, TransferHandle};
use crate::storage::{Backend, StorageEntry, StoragePath};
use crate::ui::{config, file_list, sidebar, toolbar};

// ── App mode ──────────────────────────────────────────────────────────────────

enum Mode {
    Configure {
        fields: config::ConfigFields,
        error: Option<String>,
    },
    Browse,
}

// ── App struct ────────────────────────────────────────────────────────────────

pub struct S3Explorer {
    mode: Mode,
    backend: Option<Arc<dyn Backend>>,
    current_path: StoragePath,
    entries: Vec<StorageEntry>,
    listing: Option<ListingHandle>,
    loading: bool,
    error: Option<String>,
    path_input: String,
    history: Vec<StoragePath>,
    history_pos: usize,
    filter: String,
    needs_initial_load: bool,
    transfer: Option<TransferHandle>,
    transfer_msg: Option<String>,
    rt: tokio::runtime::Handle,
}

impl S3Explorer {
    /// Start immediately in browse mode (credentials already resolved).
    pub fn new(backend: Arc<dyn Backend>, start: StoragePath, rt: tokio::runtime::Handle) -> Self {
        info!("Opening {:?} with backend '{}'", start, backend.name());
        let path_input = start.to_string();
        Self {
            mode: Mode::Browse,
            backend: Some(backend),
            current_path: start.clone(),
            entries: vec![],
            listing: None,
            loading: false,
            error: None,
            path_input,
            history: vec![start],
            history_pos: 0,
            filter: String::new(),
            needs_initial_load: true,
            transfer: None,
            transfer_msg: None,
            rt,
        }
    }

    /// Start in configure mode; fields are pre-filled from env vars and saved credentials.
    pub fn needs_config(rt: tokio::runtime::Handle) -> Self {
        Self {
            mode: Mode::Configure {
                fields: config::ConfigFields::load(),
                error: None,
            },
            backend: None,
            current_path: StoragePath::default(),
            entries: vec![],
            listing: None,
            loading: false,
            error: None,
            path_input: String::new(),
            history: vec![],
            history_pos: 0,
            filter: String::new(),
            needs_initial_load: false,
            transfer: None,
            transfer_msg: None,
            rt,
        }
    }

    // ── listing ───────────────────────────────────────────────────────────────

    fn request_listing(&mut self, path: StoragePath, ctx: &egui::Context) {
        let Some(backend) = &self.backend else { return };
        self.loading = true;
        self.error = None;
        self.filter.clear();
        self.path_input = path.to_string();
        self.current_path = path.clone();
        self.listing = Some(async_rt::spawn_listing(
            Arc::clone(backend),
            path,
            ctx.clone(),
            &self.rt,
        ));
    }

    fn poll_listing(&mut self) {
        if let Some(handle) = &self.listing
            && let Some(result) = handle.try_recv() {
                self.loading = false;
                self.listing = None;
                match result {
                    Ok(entries) => self.entries = entries,
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
    }

    // ── transfers ─────────────────────────────────────────────────────────────

    fn start_download(&mut self, path: StoragePath, ctx: &egui::Context) {
        let Some(backend) = &self.backend else { return };
        self.transfer_msg = None;
        self.transfer = Some(async_rt::spawn_download(
            Arc::clone(backend),
            path,
            ctx.clone(),
            &self.rt,
        ));
    }

    fn start_upload(&mut self, ctx: &egui::Context) {
        let Some(backend) = &self.backend else { return };
        self.transfer_msg = None;
        self.transfer = Some(async_rt::spawn_upload(
            Arc::clone(backend),
            self.current_path.clone(),
            ctx.clone(),
            &self.rt,
        ));
    }

    fn poll_transfer(&mut self, ctx: &egui::Context) {
        if let Some(handle) = &self.transfer
            && let Some(result) = handle.try_recv() {
                self.transfer = None;
                match result {
                    Ok(msg) => {
                        info!("{msg}");
                        self.transfer_msg = Some(msg);
                        // Refresh the listing so newly uploaded files appear.
                        let path = self.current_path.clone();
                        self.request_listing(path, ctx);
                    }
                    Err(e) => {
                        self.transfer_msg = Some(format!("Error: {e}"));
                    }
                }
            }
    }

    fn transfer_busy(&self) -> bool {
        self.transfer.as_ref().is_some_and(|h| h.is_running())
    }

    // ── navigation ────────────────────────────────────────────────────────────

    fn navigate_to(&mut self, path: StoragePath, ctx: &egui::Context) {
        self.history.truncate(self.history_pos + 1);
        self.history.push(path.clone());
        self.history_pos = self.history.len() - 1;
        self.request_listing(path, ctx);
    }

    fn go_back(&mut self, ctx: &egui::Context) {
        if self.history_pos > 0 {
            self.history_pos -= 1;
            let path = self.history[self.history_pos].clone();
            self.request_listing(path, ctx);
        }
    }

    fn go_forward(&mut self, ctx: &egui::Context) {
        if self.history_pos + 1 < self.history.len() {
            self.history_pos += 1;
            let path = self.history[self.history_pos].clone();
            self.request_listing(path, ctx);
        }
    }

    fn go_up(&mut self, ctx: &egui::Context) {
        if let Some(parent) = self.current_path.parent() {
            self.navigate_to(parent, ctx);
        }
    }

    // ── connect ───────────────────────────────────────────────────────────────

    fn try_connect(&mut self, ctx: &egui::Context) {
        use crate::credentials::{CredentialStore, SavedCredentials};
        use crate::storage::S3Backend;

        let Mode::Configure { fields, error } = &mut self.mode else {
            return;
        };

        let endpoint = fields.resolved_endpoint();
        let region = if fields.region.is_empty() {
            "us-east-1".to_owned()
        } else {
            fields.region.clone()
        };

        match S3Backend::with_credentials(
            &fields.bucket,
            endpoint.as_deref(),
            &fields.access_key,
            &fields.secret_key,
            &region,
        ) {
            Ok(backend) => {
                if fields.remember {
                    let creds = SavedCredentials {
                        bucket: fields.bucket.clone(),
                        endpoint: endpoint.clone().unwrap_or_default(),
                        access_key: fields.access_key.clone(),
                        secret_key: fields.secret_key.clone(),
                        region: fields.region.clone(),
                    };
                    if let Err(e) = CredentialStore::open().and_then(|s| s.save(&creds)) {
                        tracing::warn!("Failed to save credentials: {e}");
                    }
                }

                let start = StoragePath::s3_root(&fields.bucket);
                info!("Connected to S3 bucket '{}'", fields.bucket);
                self.backend = Some(Arc::new(backend));
                self.current_path = start.clone();
                self.path_input = start.to_string();
                self.history = vec![start];
                self.history_pos = 0;
                self.mode = Mode::Browse;
                let path = self.current_path.clone();
                self.request_listing(path, ctx);
            }
            Err(e) => {
                *error = Some(e.to_string());
            }
        }
    }
}

// ── eframe::App ───────────────────────────────────────────────────────────────

impl eframe::App for S3Explorer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        match self.mode {
            Mode::Configure { .. } => self.draw_config(ctx),
            Mode::Browse => self.draw_browser(ctx),
        }
    }
}

impl S3Explorer {
    fn draw_config(&mut self, ctx: &egui::Context) {
        CentralPanel::default().show(ctx, |ui| {
            let Mode::Configure { fields, error } = &mut self.mode else {
                return;
            };
            let resp = config::show(ui, fields, error.as_deref());
            if resp.connect {
                self.try_connect(ctx);
            }
        });
    }

    fn draw_browser(&mut self, ctx: &egui::Context) {
        if self.needs_initial_load {
            self.needs_initial_load = false;
            let path = self.current_path.clone();
            self.request_listing(path, ctx);
        }

        self.poll_listing();
        self.poll_transfer(ctx);

        let can_back = self.history_pos > 0;
        let can_forward = self.history_pos + 1 < self.history.len();
        let can_up = self.current_path.parent().is_some();
        let busy = self.transfer_busy();

        TopBottomPanel::top("toolbar").show(ctx, |ui| {
            let resp = toolbar::show(ui, &mut self.path_input, can_back, can_forward, can_up);
            if resp.go_back {
                self.go_back(ctx);
            }
            if resp.go_forward {
                self.go_forward(ctx);
            }
            if resp.go_up {
                self.go_up(ctx);
            }
            if resp.refresh {
                let p = self.current_path.clone();
                self.request_listing(p, ctx);
            }
            if let Some(p) = resp.navigate_to {
                self.navigate_to(p, ctx);
            }
        });

        TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let backend_name = self.backend.as_ref().map_or("—", |b| b.name());
                let n = self.entries.len();
                ui.label(
                    RichText::new(format!(
                        "{backend_name}  ·  {n} item{}",
                        if n == 1 { "" } else { "s" }
                    ))
                    .small()
                    .color(Color32::GRAY),
                );

                if busy {
                    ui.separator();
                    ui.spinner();
                    ui.label(RichText::new("Transferring…").small().color(Color32::GRAY));
                } else if let Some(msg) = &self.transfer_msg {
                    ui.separator();
                    let color = if msg.starts_with("Error") {
                        Color32::RED
                    } else {
                        Color32::from_rgb(80, 200, 120)
                    };
                    ui.label(RichText::new(msg).small().color(color));
                }
            });
        });

        let max_sidebar = ctx.screen_rect().width() / 2.0;
        SidePanel::left("sidebar")
            .resizable(true)
            .default_width(220.0)
            .width_range(80.0..=max_sidebar)
            .show(ctx, |ui| {
                let resp = sidebar::show(ui, &self.current_path);
                if let Some(path) = resp.navigate_to {
                    self.navigate_to(path, ctx);
                }
            });

        CentralPanel::default().show(ctx, |ui| {
            let resp = file_list::show(
                ui,
                &self.entries,
                &mut self.filter,
                self.loading,
                self.error.as_deref(),
                busy,
            );
            if let Some(dir) = resp.open_dir {
                self.navigate_to(dir, ctx);
            }
            if let Some(file) = resp.download
                && !busy {
                    self.start_download(file, ctx);
                }
            if resp.upload && !busy {
                self.start_upload(ctx);
            }
        });
    }
}
