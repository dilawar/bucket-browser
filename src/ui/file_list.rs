use egui::{Align, Color32, Layout, RichText, ScrollArea, Ui};

use crate::storage::{human_size, EntryKind, StorageEntry, StoragePath};

pub struct FileListResponse {
    pub open_dir: Option<StoragePath>,
}

pub fn show(
    ui: &mut Ui,
    entries: &[StorageEntry],
    filter: &str,
    loading: bool,
    error: Option<&str>,
) -> FileListResponse {
    if loading {
        ui.centered_and_justified(|ui| {
            ui.spinner();
        });
        return FileListResponse { open_dir: None };
    }

    if let Some(msg) = error {
        ui.colored_label(Color32::RED, msg);
        return FileListResponse { open_dir: None };
    }

    let filter = filter.to_lowercase();
    let visible: Vec<&StorageEntry> = entries
        .iter()
        .filter(|e| filter.is_empty() || e.name.to_lowercase().contains(&filter))
        .collect();

    let mut open_dir = None;
    let row_height = 22.0;

    ScrollArea::vertical().auto_shrink(false).show_rows(
        ui,
        row_height,
        visible.len(),
        |ui, range| {
            for entry in visible[range].iter() {
                ui.horizontal(|ui| {
                    let icon = if entry.kind == EntryKind::Directory { "📁" } else { "📄" };
                    ui.label(icon);

                    let name_widget = if entry.kind == EntryKind::Directory {
                        ui.link(RichText::new(&entry.name).color(Color32::from_rgb(100, 180, 255)))
                    } else {
                        ui.label(&entry.name)
                    };

                    if name_widget.clicked() && entry.kind == EntryKind::Directory {
                        open_dir = Some(entry.path.clone());
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(size) = entry.size {
                            ui.label(
                                RichText::new(human_size(size)).color(Color32::GRAY).small(),
                            );
                        }
                        if let Some(ts) = entry.last_modified {
                            ui.label(
                                RichText::new(ts.format("%Y-%m-%d %H:%M").to_string())
                                    .color(Color32::GRAY)
                                    .small(),
                            );
                        }
                    });
                });
            }
        },
    );

    FileListResponse { open_dir }
}
