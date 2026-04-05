use egui::Ui;

use crate::storage::StoragePath;

pub struct SidebarResponse {
    pub navigate_to: Option<StoragePath>,
}

pub fn show(ui: &mut Ui, current_path: &StoragePath, filter: &mut String) -> SidebarResponse {
    let mut navigate_to = None;

    ui.heading("Location");
    ui.separator();

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (label, path) in current_path.breadcrumbs() {
            if ui.link(&label).clicked() {
                navigate_to = Some(path);
            }
            ui.label("›");
        }
    });

    ui.separator();
    ui.label("Filter:");
    ui.text_edit_singleline(filter);

    SidebarResponse { navigate_to }
}
