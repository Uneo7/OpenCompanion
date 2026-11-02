use eframe::egui::{self, pos2, Color32, Rect};

use crate::assets::photo;

pub fn show(ui: &mut egui::Ui, rect: Rect) {
    if let Some(tex) = photo::texture(ui.ctx()) {
        ui.painter().image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
}
