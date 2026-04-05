use egui::{Button, Key, TextEdit, Ui};

use crate::storage::StoragePath;

pub struct ToolbarResponse {
    pub navigate_to: Option<StoragePath>,
    pub go_back: bool,
    pub go_forward: bool,
    pub go_up: bool,
    pub refresh: bool,
}

pub fn show(
    ui: &mut Ui,
    path_input: &mut String,
    can_back: bool,
    can_forward: bool,
    can_up: bool,
) -> ToolbarResponse {
    let mut resp = ToolbarResponse {
        navigate_to: None,
        go_back: false,
        go_forward: false,
        go_up: false,
        refresh: false,
    };

    ui.horizontal(|ui| {
        resp.go_back = ui.add_enabled(can_back, Button::new("◀")).clicked();
        resp.go_forward = ui.add_enabled(can_forward, Button::new("▶")).clicked();
        resp.go_up = ui.add_enabled(can_up, Button::new("▲")).clicked();
        resp.refresh = ui.button("⟳").clicked();

        ui.separator();

        let path_response = ui.add(
            TextEdit::singleline(path_input)
                .desired_width(f32::INFINITY)
                .hint_text("Path or s3://bucket/prefix …"),
        );

        if path_response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            resp.navigate_to = Some(StoragePath::parse(path_input));
        }
    });

    resp
}
