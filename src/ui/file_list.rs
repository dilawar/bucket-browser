use std::cell::Cell;

use egui::{Button, Color32, Label, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};

use crate::storage::{EntryKind, StorageEntry, StoragePath, human_size};

#[derive(Default)]
pub struct FileListResponse {
    pub open_dir: Option<StoragePath>,
    pub download: Option<StoragePath>,
    pub upload: bool,
}

// Fixed-column widths (icon + size + modified + copy) plus table borders/gaps.
const FIXED_WIDTH: f32 = 28.0 + 80.0 + 130.0 + 28.0 + 48.0;
const LINE_HEIGHT: f32 = 18.0;
// Vertical padding added above and below the text within each row.
const ROW_V_PAD: f32 = 6.0;
const ROW_PADDING: f32 = ROW_V_PAD * 2.0;
// Estimated average character width for the default egui proportional font.
const CHAR_WIDTH: f32 = 7.5;
// Approximate rendered width of the "⬆ Upload" button.
const UPLOAD_BTN_W: f32 = 88.0;

/// Compute the row height needed so the name column doesn't clip when it wraps.
fn row_height(name: &str, name_col_width: f32) -> f32 {
    let lines = ((name.len() as f32 * CHAR_WIDTH) / name_col_width)
        .ceil()
        .max(1.0);
    LINE_HEIGHT * lines + ROW_PADDING
}

/// Return an emoji icon for a file based on its mime type (guessed from extension).
fn file_icon(name: &str) -> &'static str {
    let mime = mime_guess::from_path(name).first_or_octet_stream();
    match mime.type_().as_str() {
        "image" => "🖼",
        "audio" => "🎵",
        "video" => "🎬",
        "text" => "📝",
        _ => match mime.subtype().as_str() {
            "zip" | "gzip" | "x-tar" | "x-bzip2" | "x-xz" | "x-7z-compressed"
            | "x-rar-compressed" => "📦",
            "pdf" => "📕",
            _ => "📄",
        },
    }
}

pub fn show(
    ui: &mut Ui,
    entries: &[StorageEntry],
    filter: &mut String,
    loading: bool,
    error: Option<&str>,
    transfer_busy: bool,
) -> FileListResponse {
    // Use Cell so multiple closures (top-bar button, bg context menu, row context menus)
    // can all set this flag without conflicting borrows.
    let upload = Cell::new(false);

    // ── Background interaction (registered first so table rows take priority) ──
    // Covers the whole panel for right-click context menus on empty space.
    let panel_rect = ui.max_rect();
    let bg_resp = ui.interact(panel_rect, ui.id().with("bg_ctx"), Sense::click());
    bg_resp.context_menu(|ui| {
        if ui
            .add_enabled(!transfer_busy, Button::new("⬆ Upload"))
            .on_hover_text("Upload a file to the current location")
            .clicked()
        {
            upload.set(true);
            ui.close_menu();
        }
    });

    // ── Top bar: filter field + upload button ─────────────────────────────────
    ui.horizontal(|ui| {
        ui.label("🔍");
        // Reserve exact width for the button so it is always visible.
        let spacing = ui.spacing().item_spacing.x;
        let text_w = (ui.available_width() - UPLOAD_BTN_W - spacing).max(40.0);
        ui.add_sized(
            [text_w, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(filter).hint_text("Filter…"),
        );
        if ui
            .add_enabled(!transfer_busy, Button::new("⬆ Upload"))
            .on_hover_text("Upload a file to the current location")
            .clicked()
        {
            upload.set(true);
        }
    });
    ui.separator();

    if loading {
        ui.centered_and_justified(|ui| {
            ui.spinner();
        });
        return FileListResponse {
            upload: upload.get(),
            ..Default::default()
        };
    }

    if let Some(msg) = error {
        ui.colored_label(Color32::RED, msg);
        return FileListResponse {
            upload: upload.get(),
            ..Default::default()
        };
    }

    let filter_lc = filter.to_lowercase();
    let visible: Vec<&StorageEntry> = entries
        .iter()
        .filter(|e| filter_lc.is_empty() || e.name.to_lowercase().contains(&filter_lc))
        .collect();

    // Name column width: total panel width minus the fixed columns.
    let name_col_width = (ui.available_width() - FIXED_WIDTH).max(80.0);

    let open_dir: Cell<Option<StoragePath>> = Cell::new(None);
    let download: Cell<Option<StoragePath>> = Cell::new(None);

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .auto_shrink(false)
        .column(Column::exact(28.0)) // icon
        .column(Column::remainder().clip(false)) // name (wraps)
        .column(Column::initial(80.0).resizable(true)) // size
        .column(Column::initial(130.0).resizable(true)) // modified
        .column(Column::exact(28.0)) // copy
        .header(20.0, |mut h| {
            h.col(|_| {});
            h.col(|ui| {
                ui.label(RichText::new("Name").strong());
            });
            h.col(|ui| {
                ui.label(RichText::new("Size").strong());
            });
            h.col(|ui| {
                ui.label(RichText::new("Modified").strong());
            });
            h.col(|_| {});
        })
        .body(|body| {
            let heights = visible.iter().map(|e| row_height(&e.name, name_col_width));
            body.heterogeneous_rows(heights, |mut row| {
                let entry = visible[row.index()];

                // ── icon ─────────────────────────────────────────────────────
                row.col(|ui| {
                    ui.add_space(ROW_V_PAD);
                    let icon = match &entry.kind {
                        EntryKind::Directory => "📁",
                        EntryKind::File => file_icon(&entry.name),
                    };
                    ui.label(icon);
                });

                // ── name ─────────────────────────────────────────────────────
                row.col(|ui| {
                    ui.add_space(ROW_V_PAD);
                    let resp = ui
                        .add(
                            Label::new(match entry.kind {
                                EntryKind::Directory => RichText::new(&entry.name)
                                    .color(Color32::from_rgb(100, 180, 255)),
                                EntryKind::File => RichText::new(&entry.name),
                            })
                            .wrap()
                            .sense(Sense::click()),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(entry.path.to_string());

                    if resp.clicked() {
                        match entry.kind {
                            EntryKind::Directory => open_dir.set(Some(entry.path.clone())),
                            EntryKind::File => download.set(Some(entry.path.clone())),
                        }
                    }

                    // Right-click on any row also shows the upload option.
                    resp.context_menu(|ui| {
                        if ui
                            .add_enabled(!transfer_busy, Button::new("⬆ Upload"))
                            .on_hover_text("Upload a file to the current location")
                            .clicked()
                        {
                            upload.set(true);
                            ui.close_menu();
                        }
                    });
                });

                // ── size ─────────────────────────────────────────────────────
                row.col(|ui| {
                    ui.add_space(ROW_V_PAD);
                    if let Some(size) = entry.size {
                        ui.label(RichText::new(human_size(size)).color(Color32::GRAY).small());
                    }
                });

                // ── modified ─────────────────────────────────────────────────
                row.col(|ui| {
                    ui.add_space(ROW_V_PAD);
                    if let Some(ts) = entry.last_modified {
                        ui.label(
                            RichText::new(ts.format("%Y-%m-%d %H:%M").to_string())
                                .color(Color32::GRAY)
                                .small(),
                        );
                    }
                });

                // ── copy path ────────────────────────────────────────────────
                row.col(|ui| {
                    ui.add_space(ROW_V_PAD);
                    let path_str = entry.path.to_string();
                    if ui
                        .button("⎘")
                        .on_hover_text(format!("Copy: {path_str}"))
                        .clicked()
                    {
                        ui.ctx().copy_text(path_str);
                    }
                });
            });
        });

    FileListResponse {
        open_dir: open_dir.into_inner(),
        download: download.into_inner(),
        upload: upload.get(),
    }
}
